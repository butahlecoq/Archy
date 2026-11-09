"""Fixed Blender subprocess entry point; argv contains only trusted file paths."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import protocol


def _digest(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _camera(scene, bounds, view):
    import bpy
    from mathutils import Vector
    low = Vector(bounds[:3]); high = Vector(bounds[3:])
    center = (low + high) / 2
    span = high - low
    if view == "top":
        location = center + Vector((0, 0, max(span) * 3 + 10))
        up = "Y"
    elif view == "front":
        location = center + Vector((0, -max(span) * 3 - 10, 0))
        up = "Y"
    else:
        location = center + Vector((max(span) * 2 + 10, -max(span) * 2 - 10, max(span) * 2 + 10))
        up = "Y"
    camera_data = bpy.data.cameras.new("Camera_" + view)
    camera_data.type = "ORTHO"
    camera = bpy.data.objects.new("Camera_" + view, camera_data)
    scene.collection.objects.link(camera)
    camera.location = location
    camera.rotation_euler = (center - location).to_track_quat("-Z", up).to_euler()
    bpy.context.view_layer.update()
    corners = [Vector((x, y, z)) for x in (low.x, high.x) for y in (low.y, high.y) for z in (low.z, high.z)]
    inverse = camera.matrix_world.inverted()
    projected = [inverse @ corner for corner in corners]
    camera_data.ortho_scale = 1.1 * max(max(p.x for p in projected) - min(p.x for p in projected),
                                         max(p.y for p in projected) - min(p.y for p in projected))
    camera_data.clip_start = 0.01
    camera_data.clip_end = max(1000, (location - center).length + max(span) * 2)
    scene.camera = camera
    return camera


def _preview_edges(scene, ring, height, pixel_m, camera):
    """Draw mass edges at screen width for an underresolved view, outside the GLB."""
    import bpy
    from mathutils import Vector
    curve = bpy.data.curves.new("PreviewMassingEdges", "CURVE")
    curve.dimensions = "3D"
    curve.bevel_resolution = 0
    curve.resolution_u = 1
    curve.bevel_depth = pixel_m * 0.75
    offset = camera.matrix_world.to_quaternion() @ Vector((0, 0, pixel_m))
    for z in (0, height):
        spline = curve.splines.new("POLY")
        spline.points.add(len(ring) - 1)
        for point, (x, y) in zip(spline.points, ring):
            location = Vector((x, y, z)) + offset
            point.co = (location.x, location.y, location.z, 1.0)
        spline.use_cyclic_u = True
    for x, y in ring:
        spline = curve.splines.new("POLY")
        spline.points.add(1)
        for point, z in zip(spline.points, (0, height)):
            location = Vector((x, y, z)) + offset
            point.co = (location.x, location.y, location.z, 1.0)
    material = bpy.data.materials.new("PreviewMassingEdgesInk")
    material.diffuse_color = (0.8, 0.9, 1.0, 1.0)
    material.use_nodes = True
    nodes = material.node_tree.nodes
    nodes.clear()
    emission = nodes.new("ShaderNodeEmission")
    emission.inputs["Color"].default_value = (0.8, 0.9, 1.0, 1.0)
    emission.inputs["Strength"].default_value = 1.0
    output = nodes.new("ShaderNodeOutputMaterial")
    material.node_tree.links.new(emission.outputs[0], output.inputs["Surface"])
    curve.materials.append(material)
    obj = bpy.data.objects.new("PreviewMassingEdges", curve)
    scene.collection.objects.link(obj)
    return obj


def _run(request, directory):
    import bpy
    from mathutils import Vector
    from scene import build
    objects, geometry = build(request)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.render.threads_mode = "FIXED"
    scene.render.threads = 1
    scene.cycles.device = "CPU"
    scene.cycles.samples = 16
    scene.cycles.use_adaptive_sampling = False
    scene.cycles.use_denoising = False
    scene.cycles.seed = 42
    scene.render.resolution_x = scene.render.resolution_y = 512
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.render.image_settings.compression = 15
    scene.render.film_transparent = False
    scene.display_settings.display_device = "sRGB"
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "Medium High Contrast"
    scene.view_settings.exposure = 0.0
    scene.view_settings.gamma = 1.0
    scene.world = bpy.data.worlds.new("NeutralWorld")
    scene.world.use_nodes = True
    background = scene.world.node_tree.nodes.get("Background")
    background.inputs["Color"].default_value = (0.12, 0.15, 0.2, 1.0)
    background.inputs["Strength"].default_value = 0.6
    light_data = bpy.data.lights.new("KeyLight", "AREA")
    light_data.energy = 3000
    light_data.color = (1.0, 1.0, 1.0)
    light_data.shape = "DISK"
    light_data.size = 10
    light = bpy.data.objects.new("KeyLight", light_data)
    scene.collection.objects.link(light)
    bounds = geometry["bounds_m"]
    light.location = ((bounds[0] + bounds[3]) / 2 + 5, (bounds[1] + bounds[4]) / 2 - 5, bounds[5] + 12)
    for obj in bpy.context.selected_objects:
        obj.select_set(False)
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.export_scene.gltf(filepath=str(directory / "model.glb"), export_format="GLB",
                              use_selection=True, export_yup=True, export_animations=False,
                              export_cameras=False, export_lights=False,
                              export_draco_mesh_compression_enable=False)
    cameras = {}
    outlined_views = []
    for view in ("top", "front", "iso"):
        camera = _camera(scene, bounds, view)
        outline = None
        pixel_m = camera.data.ortho_scale / 512
        if min(bounds[3] - bounds[0], bounds[4] - bounds[1]) < pixel_m * 2:
            outline = _preview_edges(scene, request["params"]["footprint_xy_m"][:-1], bounds[5], pixel_m, camera)
            outlined_views.append(view)
        bpy.context.view_layer.update()
        scene.render.filepath = str(directory / ("view-" + view + ".png"))
        bpy.ops.render.render(write_still=True)
        if outline is not None:
            curve = outline.data
            ink = curve.materials[0]
            bpy.data.objects.remove(outline, do_unlink=True)
            bpy.data.curves.remove(curve)
            bpy.data.materials.remove(ink)
        cameras[view] = {"name": camera.name, "type": camera.data.type,
                         "matrix_world": [list(row) for row in camera.matrix_world],
                         "ortho_scale": camera.data.ortho_scale,
                         "clip_start": camera.data.clip_start, "clip_end": camera.data.clip_end}
    scene.render.filepath = "//view-iso.png"
    bpy.ops.wm.save_as_mainfile(filepath=str(directory / "scene.blend"), compress=False)
    material = objects[0].data.materials[0]
    shader = material.node_tree.nodes.get("Principled BSDF")
    background = scene.world.node_tree.nodes.get("Background")
    return geometry, {"preset_id": request["preset_id"],
                      "engine": scene.render.engine, "device": scene.cycles.device,
                      "threads": scene.render.threads, "threads_mode": scene.render.threads_mode,
                      "samples": scene.cycles.samples,
                      "seed": scene.cycles.seed,
                      "adaptive_sampling": scene.cycles.use_adaptive_sampling,
                      "denoising": scene.cycles.use_denoising,
                      "resolution": [scene.render.resolution_x, scene.render.resolution_y],
                      "resolution_percentage": scene.render.resolution_percentage,
                      "format": scene.render.image_settings.file_format + "_" + scene.render.image_settings.color_mode + scene.render.image_settings.color_depth,
                      "image_compression": scene.render.image_settings.compression,
                      "film_transparent": scene.render.film_transparent,
                      "display_device": scene.display_settings.display_device,
                      "view_transform": scene.view_settings.view_transform,
                      "look": scene.view_settings.look,
                      "exposure": scene.view_settings.exposure, "gamma": scene.view_settings.gamma,
                      "axis_conversion": "Blender Z-up to glTF Y-up", "frame": scene.frame_current,
                      "unit": {"system": scene.unit_settings.system,
                               "scale_length": scene.unit_settings.scale_length},
                      "world": {"use_nodes": scene.world.use_nodes,
                                "color_rgba": list(background.inputs["Color"].default_value),
                                "strength": background.inputs["Strength"].default_value},
                      "material": {"use_nodes": material.use_nodes,
                                   "diffuse_rgba": list(material.diffuse_color),
                                   "base_color_rgba": list(shader.inputs["Base Color"].default_value),
                                   "roughness": shader.inputs["Roughness"].default_value},
                      "cameras": cameras,
                      "objects": [{"name": obj.name, "location": list(obj.location),
                                   "rotation_euler": list(obj.rotation_euler), "scale": list(obj.scale),
                                   "matrix_world": [list(row) for row in obj.matrix_world]} for obj in objects],
                      "light": {"type": light_data.type, "shape": light_data.shape,
                                "color_rgb": list(light_data.color),
                                "location": list(light.location), "rotation_euler": list(light.rotation_euler),
                                "matrix_world": [list(row) for row in light.matrix_world],
                                "energy": light_data.energy, "size": light_data.size},
                      "glb_export": {"format": "GLB", "selected_only": True, "export_yup": True,
                                     "animations": False, "cameras": False, "lights": False, "compression": False},
                      "view_bounds_m": bounds, "saved_render_path": scene.render.filepath}, outlined_views


def main():
    import bpy
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", required=True)
    parser.add_argument("--response", required=True)
    args = parser.parse_args(argv)
    response = Path(args.response).absolute()
    input_path = Path(args.request).absolute()
    if any(part.is_symlink() for path in (response, input_path) for part in (path, *path.parents)):
        raise ValueError("symlink in request or response path")
    directory = response.parent
    names = ("scene.blend", "model.glb", "view-top.png", "view-front.png", "view-iso.png")
    if response.exists() or any((directory / name).exists() or (directory / name).is_symlink() for name in names):
        raise FileExistsError("worker output collision")
    try:
        if input_path.stat().st_size > protocol.MAX_REQUEST_BYTES:
            raise protocol.ProtocolError("request exceeds 1 MiB", "$")
        request = protocol.parse_request(input_path.read_bytes())
        geometry, preset, outlined_views = _run(request, directory)
        media = {"scene.blend": "application/x-blender", "model.glb": "model/gltf-binary"}
        artifacts = [{"path": name, "sha256": _digest(directory / name),
                      "size_bytes": (directory / name).stat().st_size,
                      "media_type": media.get(name, "image/png"), "producer": "blender-worker-v1"}
                     for name in names]
        payload = {"schema_version": protocol.SCHEMA_VERSION, "request_id": request["request_id"],
                   "project_id": request["project_id"], "variant_id": request["variant_id"],
                   "run_id": request["run_id"], "request_digest": request["input_digest"],
                   "worker_version": "blender-worker-v1", "blender_version": bpy.app.version_string,
                   "build_hash": bpy.app.build_hash.decode() if isinstance(bpy.app.build_hash, bytes) else str(bpy.app.build_hash),
                   "python_version": sys.version, "status": "ok",
                   "warnings": [view + " view uses screen-width massing edges; GLB dimensions remain exact" for view in outlined_views],
                   "preset": preset, "geometry": geometry, "artifacts": artifacts}
    except protocol.ProtocolError as error:
        payload = {"schema_version": protocol.SCHEMA_VERSION, "status": "error", "error": error.envelope(), "artifacts": []}
    except Exception as error:
        payload = {"schema_version": protocol.SCHEMA_VERSION, "status": "error",
                   "error": {"code": "INTERNAL_ERROR", "field": "$", "message": str(error), "retriable": False},
                   "artifacts": []}
    try:
        protocol.validate_response(payload)
    except protocol.ProtocolError as error:
        payload = {"schema_version": protocol.SCHEMA_VERSION, "status": "error",
                   "error": {"code": "INTERNAL_ERROR", "field": error.field,
                             "message": "worker response failed validation: " + str(error),
                             "retriable": False}, "artifacts": []}
        protocol.validate_response(payload)
    with response.open("x", encoding="utf-8") as file:
        json.dump(payload, file, sort_keys=True, allow_nan=False)
    if payload["status"] != "ok":
        raise SystemExit(20)


if __name__ == "__main__":
    main()
