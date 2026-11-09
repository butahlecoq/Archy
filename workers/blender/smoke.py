"""Read-only fresh-process check for a generated, trusted .blend file."""

import bpy

scene = bpy.context.scene
if not scene.get("archy_variant_id") or not scene.get("archy_input_digest"):
    raise RuntimeError("missing source provenance")
objects = [obj for obj in scene.objects if obj.type == "MESH" and obj.name.startswith("Storey_")]
if not objects or len(objects) != len({obj.name for obj in objects}):
    raise RuntimeError("invalid storey inventory")
if any(len(obj.data.vertices) < 8 or len(obj.data.polygons) < 8 for obj in objects):
    raise RuntimeError("empty storey mesh")
if not scene.render.filepath.startswith("//"):
    raise RuntimeError("saved scene contains an absolute render output path")
print("ARCHY_REOPEN_OK", len(objects), scene["archy_variant_id"], scene.get("archy_north_angle_deg"))
