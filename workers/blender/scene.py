"""Trusted data-block construction for Task02 rectangle and concave L massing."""

import bpy
import math
from mathutils import Vector
from mathutils.geometry import tessellate_polygon
from protocol import ProtocolError


def build(request):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    scene.frame_set(1)
    variant = request["params"]
    ring = variant["footprint_xy_m"][:-1]
    n = len(ring)
    origin = Vector((*variant["parameters"]["origin_xy_m"], 0.0))
    local = [Vector((point[0] - origin.x, point[1] - origin.y, 0.0)) for point in ring]
    triangles = tessellate_polygon([local])
    if len(triangles) != len(local) - 2:
        raise ValueError("cap tessellation did not cover footprint")
    cap = [tuple(triangle) for triangle in triangles]
    area = sum((ring[i][0] * ring[(i + 1) % len(ring)][1] - ring[(i + 1) % len(ring)][0] * ring[i][1]) for i in range(len(ring))) / 2
    if area <= 0:
        raise ValueError("nonpositive footprint area")
    cap_area = sum(abs((local[b].x-local[a].x)*(local[c].y-local[a].y)
                       - (local[c].x-local[a].x)*(local[b].y-local[a].y)) / 2
                   for a, b, c in cap)
    perimeter = sum(math.hypot(ring[(i + 1) % n][0] - ring[i][0],
                               ring[(i + 1) % n][1] - ring[i][1]) for i in range(n))
    if abs(cap_area - area) > perimeter * 1e-6:
        raise ValueError("cap coverage mismatch")
    collection = bpy.data.collections.new("Massing")
    scene.collection.children.link(collection)
    material = bpy.data.materials.new("MassingNeutral")
    material.diffuse_color = (0.3, 0.55, 0.8, 1.0)
    material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Base Color"].default_value = material.diffuse_color
    shader.inputs["Roughness"].default_value = 0.8
    objects = []
    max_world_error = 0.0
    for storey in range(variant["storeys"]):
        z = storey * variant["floor_to_floor_m"]
        vertices = [(v.x, v.y, 0.0) for v in local] + [(v.x, v.y, variant["floor_to_floor_m"]) for v in local]
        faces = [tuple(reversed(triangle)) for triangle in cap]
        faces += [tuple(index + n for index in triangle) for triangle in cap]
        faces += [(i, (i + 1) % n, (i + 1) % n + n, i + n) for i in range(n)]
        mesh = bpy.data.meshes.new(f"Storey_{storey:03d}_mesh")
        mesh.from_pydata(vertices, [], faces)
        mesh.update()
        if mesh.validate(verbose=False, clean_customdata=False):
            raise ValueError("mesh validation repaired geometry")
        counts = {}
        for polygon in mesh.polygons:
            for edge in polygon.edge_keys:
                key = tuple(sorted(edge))
                counts[key] = counts.get(key, 0) + 1
        if any(count != 2 for count in counts.values()):
            raise ValueError("open or nonmanifold prism")
        volume = 0.0
        for polygon in mesh.polygons:
            vertices = [mesh.vertices[index].co for index in polygon.vertices]
            for j in range(1, len(vertices) - 1):
                volume += vertices[0].dot(vertices[j].cross(vertices[j + 1])) / 6
        if volume <= 0 or abs(volume - area * variant["floor_to_floor_m"]) > max(1e-6, area * variant["floor_to_floor_m"] * 1e-6):
            raise ValueError("prism signed volume mismatch")
        obj = bpy.data.objects.new(f"Storey_{storey:03d}", mesh)
        obj.location = (origin.x, origin.y, z)
        mesh.materials.append(material)
        collection.objects.link(obj)
        bpy.context.view_layer.update()
        for index, canonical in enumerate(ring):
            for layer in (0, 1):
                world = obj.matrix_world @ mesh.vertices[index + layer * n].co
                expected = (canonical[0], canonical[1], (storey + layer) * variant["floor_to_floor_m"])
                error = max(abs(world[axis] - expected[axis]) for axis in range(3))
                max_world_error = max(max_world_error, error)
                if error > 1e-6:
                    raise ProtocolError(
                        f"Blender float32 world coordinate error {error:.9g} m exceeds 1e-6 m parity budget; use representable coordinates or a nearer origin for this backend",
                        "params.footprint_xy_m", "BACKEND_UNAVAILABLE")
        objects.append(obj)
    scene["archy_variant_id"] = variant["id"]
    scene["archy_north_angle_deg"] = request["site"]["north_angle_deg"]
    scene["archy_input_digest"] = request["input_digest"]
    return objects, {"storeys": variant["storeys"], "footprint_area_m2": area,
                     "volume_m3": area * variant["storeys"] * variant["floor_to_floor_m"],
                     "world_coordinate_max_error_m": max_world_error,
                     "cap_area_error_m2": abs(cap_area - area),
                     "bounds_m": [min(p[0] for p in ring), min(p[1] for p in ring), 0.0,
                                  max(p[0] for p in ring), max(p[1] for p in ring),
                                  variant["storeys"] * variant["floor_to_floor_m"]]}
