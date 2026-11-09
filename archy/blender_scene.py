"""Build a Blender scene from Archy's evaluated massing JSON report.

Run with ``blender --background --python blender_scene.py -- report.json scene.blend``.
The JSON is produced by ``archy brief.yaml`` so Blender does not need PyYAML.
"""

import argparse
import hashlib
from itertools import permutations
import json
import math
from pathlib import Path
import re
import sys

# Blender's script path is archy/blender_scene.py in both source and installed layouts.
PACKAGE_PARENT = Path(__file__).resolve().parent.parent
if str(PACKAGE_PARENT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_PARENT))
from archy.geometry import Footprint


MAX_FLOAT32_COORDINATE_TOLERANCE_M = 1e-4


def _coordinate_close(actual: float, expected: float, tolerance: float, *, float32: bool) -> bool:
    allowed = tolerance
    if float32:
        scaled = max(abs(actual), abs(expected)) * 1e-7
        allowed = min(max(tolerance, scaled), MAX_FLOAT32_COORDINATE_TOLERANCE_M)
    return math.isclose(actual, expected, rel_tol=0.0, abs_tol=allowed)


def _same_points(
    actual: list[tuple[float, float]],
    expected: list[tuple[float, float]],
    tolerance: float,
    *,
    float32: bool,
) -> bool:
    return len(actual) == len(expected) and any(
        all(
            all(_coordinate_close(a, e, tolerance, float32=float32) for a, e in zip(actual_point, expected_point))
            for actual_point, expected_point in zip(actual, expected_order)
        )
        for expected_order in permutations(expected)
    )


def build_scene(report: dict) -> None:
    """Replace the current scene with one metre-scaled cube per alternative."""
    import bpy
    from mathutils import Vector
    if report.get("contract_version") != 1 or report.get("units") != {"length": "m", "area": "m2"}:
        raise ValueError("expected an Archy contract version 1 report in metres")
    results = report.get("results")
    if not isinstance(results, list) or not results:
        raise ValueError("report must contain at least one result")

    identifiers = set()
    for result in results:
        alt_id = result.get("id") if isinstance(result, dict) else None
        if not isinstance(alt_id, str) or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", alt_id) is None:
            raise ValueError("alternative IDs must contain only letters, digits, underscores, and hyphens")
        if alt_id in identifiers:
            raise ValueError(f"duplicate alternative ID: {alt_id}")
        identifiers.add(alt_id)
        try:
            geometry = Footprint.from_mapping(result["input"]["footprint"])
            values = (
                result["input"]["footprint"]["width_m"],
                result["input"]["footprint"]["depth_m"],
                result["input"]["footprint"]["origin_x_m"],
                result["input"]["footprint"]["origin_y_m"],
                result["measurements"]["building_height_m"],
            )
        except (KeyError, TypeError) as error:
            raise ValueError(f"alternative {alt_id} has an incomplete geometry record") from error
        if any(isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) for value in values):
            raise ValueError(f"alternative {alt_id} has non-finite or non-numeric geometry")
        if min(values[0], values[1], values[4]) <= 0 or min(values[2], values[3]) < 0:
            raise ValueError(f"alternative {alt_id} has invalid dimensions or origins")

    report_digest = hashlib.sha256(json.dumps(report, sort_keys=True, separators=(",", ":")).encode()).hexdigest()

    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    scene["archy_report_sha256"] = report_digest
    scene["archy_blender_version"] = bpy.app.version_string
    scene["archy_blender_build_hash"] = bpy.app.build_hash.decode("ascii", errors="replace")

    for result in results:
        alt_id = result["id"]
        footprint = result["input"]["footprint"]
        height = result["measurements"]["building_height_m"]
        geometry = Footprint.from_mapping(footprint)
        center = geometry.center()
        width = geometry.width_m
        depth = geometry.depth_m
        bpy.ops.mesh.primitive_cube_add(size=1.0, location=(center.x_m, center.y_m, height / 2))
        obj = bpy.context.object
        obj.name = f"Archy_Mass_{alt_id}"
        obj.dimensions = (width, depth, height)
        obj.rotation_euler[2] = math.radians(geometry.rotation_degrees)
        obj["archy_alternative_id"] = alt_id
        obj["floor_count"] = result["input"]["floor_count"]
        obj["floor_to_floor_height_m"] = result["input"]["floor_to_floor_height_m"]
        for key, value in footprint.items():
            obj[key] = value
        for key, value in result["measurements"].items():
            obj[key] = value
        bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)


def inspect_scene(report: dict, tolerance: float = 1e-6) -> dict:
    """Compare saved Blender mesh world bounds to the report inputs."""
    import bpy
    from mathutils import Vector
    report_digest = hashlib.sha256(json.dumps(report, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    if bpy.context.scene.get("archy_report_sha256") != report_digest:
        raise ValueError("saved scene provenance does not match evaluation report")
    bpy.context.view_layer.update()
    checks = []
    for result in report["results"]:
        alt_id = result["id"]
        obj = bpy.data.objects.get(f"Archy_Mass_{alt_id}")
        if obj is None or obj.type != "MESH":
            raise ValueError(f"missing generated mesh for {alt_id}")
        expected_properties = {
            "archy_alternative_id": alt_id,
            **result["input"]["footprint"],
            "floor_count": result["input"]["floor_count"],
            "floor_to_floor_height_m": result["input"]["floor_to_floor_height_m"],
            **result["measurements"],
        }
        for key, expected_value in expected_properties.items():
            actual_value = obj.get(key)
            if isinstance(expected_value, (int, float)) and not isinstance(expected_value, bool):
                if not isinstance(actual_value, (int, float)) or not math.isclose(actual_value, expected_value, rel_tol=0.0, abs_tol=tolerance):
                    raise ValueError(f"{alt_id} metadata {key}: expected {expected_value}, got {actual_value}")
            elif actual_value != expected_value:
                raise ValueError(f"{alt_id} metadata {key}: expected {expected_value}, got {actual_value}")
        bounds = [obj.matrix_world @ Vector(corner) for corner in obj.bound_box]
        actual = {
            "min_x_m": min(point.x for point in bounds),
            "max_x_m": max(point.x for point in bounds),
            "min_y_m": min(point.y for point in bounds),
            "max_y_m": max(point.y for point in bounds),
            "min_z_m": min(point.z for point in bounds),
            "max_z_m": max(point.z for point in bounds),
        }
        footprint = result["input"]["footprint"]
        geometry = Footprint.from_mapping(footprint)
        bounds_expected = geometry.bounds()
        expected = {
            "min_x_m": bounds_expected.min_x_m,
            "max_x_m": bounds_expected.max_x_m,
            "min_y_m": bounds_expected.min_y_m,
            "max_y_m": bounds_expected.max_y_m,
            "min_z_m": 0.0,
            "max_z_m": result["measurements"]["building_height_m"],
        }
        for key, expected_value in expected.items():
            if not _coordinate_close(
                actual[key], expected_value, tolerance,
                float32="rotation_degrees" in footprint,
            ):
                raise ValueError(f"{alt_id} {key}: expected {expected_value}, got {actual[key]}")
        minimum_z = min(point.z for point in bounds)
        actual_bottom = list(dict.fromkeys(
            (point.x, point.y) for point in bounds if point.z == minimum_z
        ))
        expected_corners = [(point.x_m, point.y_m) for point in geometry.corners()]
        if not _same_points(
            actual_bottom, expected_corners, tolerance,
            float32="rotation_degrees" in footprint,
        ):
            raise ValueError(f"{alt_id} transformed mesh corners do not match evaluation footprint")
        checks.append({
            "id": alt_id,
            "bounds": actual,
            "corners": [{"x_m": x, "y_m": y} for x, y in actual_bottom],
            "matches_input": True,
        })
    return {
        "contract_version": report["contract_version"],
        "runtime": {
            "blender_version": bpy.app.version_string,
            "blender_build_hash": bpy.app.build_hash.decode("ascii", errors="replace"),
        },
        "checks": checks,
    }


def render_alternatives(report: dict, output_dir: Path) -> list[Path]:
    import bpy
    from mathutils import Vector
    output_dir.mkdir(parents=True, exist_ok=True)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 1200
    scene.render.resolution_y = 900
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.film_transparent = False
    scene.render.resolution_percentage = 100
    scene.render.filepath = str(output_dir / "archy-preview.png")
    scene.render.image_settings.color_mode = "RGBA"
    scene.world.color = (0.055, 0.065, 0.08)
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "Medium High Contrast"
    scene.view_settings.exposure = 0.0
    scene.view_settings.gamma = 1.0

    material = bpy.data.materials.get("Archy_Massing_Material") or bpy.data.materials.new("Archy_Massing_Material")
    material.diffuse_color = (0.22, 0.48, 0.72, 1.0)
    material.use_nodes = True
    material.node_tree.nodes.get("Principled BSDF").inputs["Base Color"].default_value = (0.12, 0.38, 0.68, 1)
    material.node_tree.nodes.get("Principled BSDF").inputs["Roughness"].default_value = 0.68

    bpy.ops.object.select_all(action="DESELECT")
    for obj in bpy.context.scene.objects:
        if obj.type in {"CAMERA", "LIGHT"}:
            bpy.data.objects.remove(obj, do_unlink=True)
    created = []
    site = report["site"]
    setbacks = site["setbacks_m"]
    site_width = site["width_m"]
    site_depth = site["depth_m"]
    buildable = (
        setbacks["west"],
        setbacks["south"],
        site_width - setbacks["east"],
        site_depth - setbacks["north"],
    )

    def add_guide(name: str, points: list[tuple[float, float]], color: tuple[float, float, float, float], thickness: float, tracked_objects: list):
        curve = bpy.data.curves.new(name, "CURVE")
        curve.dimensions = "3D"
        curve.bevel_depth = thickness
        curve.bevel_resolution = 2
        spline = curve.splines.new("POLY")
        spline.points.add(len(points) - 1)
        for point, (x, y) in zip(spline.points, points):
            point.co = (x, y, 0.035, 1.0)
        spline.use_cyclic_u = True
        obj = bpy.data.objects.new(name, curve)
        scene.collection.objects.link(obj)
        tracked_objects.append(obj)
        obj.data.materials.append(guide_material(color))
        return obj

    def guide_material(color: tuple[float, float, float, float]):
        material = bpy.data.materials.get("Archy_Context_" + "_".join(str(round(channel * 255)) for channel in color[:3]))
        if material is None:
            material = bpy.data.materials.new("Archy_Context_" + "_".join(str(round(channel * 255)) for channel in color[:3]))
            material.diffuse_color = color
            material.use_nodes = True
            material.node_tree.nodes.get("Principled BSDF").inputs["Base Color"].default_value = color
            material.node_tree.nodes.get("Principled BSDF").inputs["Emission Color"].default_value = color
            material.node_tree.nodes.get("Principled BSDF").inputs["Emission Strength"].default_value = 0.25
        return material

    for result in report["results"]:
        mass = bpy.data.objects.get(f"Archy_Mass_{result['id']}")
        if mass is None:
            raise ValueError(f"missing generated mesh for {result['id']}")
        mass.data.materials.clear()
        mass.data.materials.append(material)
        other_masses = [obj for obj in bpy.data.objects if obj.name.startswith("Archy_Mass_") and obj != mass]
        previous_visibility = {other.name: other.hide_render for other in other_masses}
        previous_camera = scene.camera
        context_objects = []
        try:
            for other in other_masses:
                other.hide_render = True
            geometry = Footprint.from_mapping(result["input"]["footprint"])
            world_bounds = geometry.bounds()
            center = mass.matrix_world.translation.copy()
            dimensions = mass.dimensions.copy()
            extent = max(dimensions)
            min_x = min(0.0, world_bounds.min_x_m)
            max_x = max(site_width, world_bounds.max_x_m)
            min_y = min(0.0, world_bounds.min_y_m)
            max_y = max(site_depth, world_bounds.max_y_m)
            view_center = Vector(((min_x + max_x) / 2, (min_y + max_y) / 2, dimensions.z / 2))
            view_extent = max(max_x - min_x, max_y - min_y, dimensions.z)
            camera_data = bpy.data.cameras.new(f"Archy_Camera_{result['id']}")
            camera = bpy.data.objects.new(camera_data.name, camera_data)
            scene.collection.objects.link(camera)
            context_objects.append(camera)
            camera.location = view_center + Vector((view_extent * 1.2, -view_extent * 1.55, view_extent * 1.35))
            direction = view_center - camera.location
            camera.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
            camera_data.type = "ORTHO"
            camera_data.ortho_scale = view_extent * 1.75
            scene.camera = camera
            light_data = bpy.data.lights.new(f"Archy_Key_{result['id']}", "AREA")
            light = bpy.data.objects.new(light_data.name, light_data)
            scene.collection.objects.link(light)
            context_objects.append(light)
            light.location = center + Vector((0, -extent, extent * 2.0))
            light_data.energy = 6500
            light_data.shape = "DISK"
            light_data.size = extent * 1.2
            light.rotation_euler = (center - light.location).to_track_quat("-Z", "Y").to_euler()
            bpy.ops.mesh.primitive_plane_add(size=1.0, location=(site_width / 2, site_depth / 2, -0.05))
            ground = bpy.context.object
            context_objects.append(ground)
            ground.name = f"Archy_Site_{result['id']}"
            ground.dimensions = (site_width, site_depth, 0.02)
            bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
            ground_material = bpy.data.materials.get("Archy_Site_Material") or bpy.data.materials.new("Archy_Site_Material")
            ground_material.diffuse_color = (0.34, 0.36, 0.38, 1.0)
            ground_material.use_nodes = True
            ground_material.node_tree.nodes.get("Principled BSDF").inputs["Base Color"].default_value = (0.22, 0.25, 0.28, 1)
            ground.data.materials.append(ground_material)
            add_guide(f"Archy_Site_Boundary_{result['id']}", [(0, 0), (site_width, 0), (site_width, site_depth), (0, site_depth)], (0.96, 0.78, 0.28, 1), 0.045, context_objects)
            add_guide(f"Archy_Buildable_Area_{result['id']}", [(buildable[0], buildable[1]), (buildable[2], buildable[1]), (buildable[2], buildable[3]), (buildable[0], buildable[3])], (0.35, 0.88, 0.62, 1), 0.035, context_objects)
            compass_x = site_width * 0.07
            compass_y = site_depth * 0.07
            compass_length = min(site_width, site_depth) * 0.09
            for label, start, end, direction in (
                ("N", (compass_x, compass_y), (compass_x, compass_y + compass_length), "north"),
                ("E", (compass_x, compass_y), (compass_x + compass_length, compass_y), "east"),
            ):
                add_guide(f"Archy_Compass_{direction}_{result['id']}", [start, end], (0.9, 0.92, 0.96, 1), 0.045, context_objects)
                text_data = bpy.data.curves.new(f"Archy_Compass_Label_{label}_{result['id']}", "FONT")
                text_data.body = label
                text_data.size = compass_length * 0.22
                text_obj = bpy.data.objects.new(text_data.name, text_data)
                scene.collection.objects.link(text_obj)
                context_objects.append(text_obj)
                text_obj.location = (end[0] + compass_length * 0.05, end[1] + compass_length * 0.05, 0.04)
                text_obj.data.materials.append(guide_material((0.9, 0.92, 0.96, 1)))
            path = output_dir / f"{result['id']}.png"
            scene.render.filepath = str(path)
            scene.render.image_settings.color_mode = "RGBA"
            scene.render.resolution_percentage = 100
            scene.render.filepath = str(path)
            scene.render.engine = "BLENDER_EEVEE"
            scene.eevee.taa_render_samples = 64
            bpy.ops.render.render(write_still=True)
            created.append(path)
        finally:
            scene.camera = previous_camera
            for context_obj in reversed(context_objects):
                if context_obj.name in bpy.data.objects:
                    bpy.data.objects.remove(context_obj, do_unlink=True)
            for other in other_masses:
                other.hide_render = previous_visibility[other.name]
    return created


def main(argv=None) -> int:
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else (argv or [])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("scene", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--inspect", action="store_true", help="inspect an existing scene and compare bounds")
    mode.add_argument("--render-dir", type=Path, help="render one PNG preview per alternative")
    parser.add_argument("--inspection-json", type=Path, help="write inspection results as JSON")
    args = parser.parse_args(argv)
    with args.report.open(encoding="utf-8") as stream:
        report = json.load(stream)
    import bpy
    if args.inspect:
        bpy.ops.wm.open_mainfile(filepath=str(args.scene))
        inspection = inspect_scene(report)
        encoded = json.dumps(inspection, indent=2, sort_keys=True)
        if args.inspection_json:
            args.inspection_json.write_text(encoded + "\n", encoding="utf-8")
        print(encoded)
    elif args.render_dir:
        bpy.ops.wm.open_mainfile(filepath=str(args.scene))
        paths = render_alternatives(report, args.render_dir)
        print(json.dumps({"renders": [str(path) for path in paths]}, indent=2))
    else:
        build_scene(report)
        bpy.ops.wm.save_as_mainfile(filepath=str(args.scene))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
