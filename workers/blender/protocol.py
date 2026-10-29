"""Bounded, stdlib-only request validation for the trusted Blender worker."""

import json
import math
import re

MAX_REQUEST_BYTES = 1024 * 1024
SCHEMA_VERSION = "blender-worker-v1"
ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}\Z")
VARIANT_ID = re.compile(r"variant-[0-9a-f]{64}\Z")
DIGEST = re.compile(r"[0-9a-f]{64}\Z")


class ProtocolError(ValueError):
    def __init__(self, message, field="$", code="INVALID_SCHEMA"):
        super().__init__(message)
        self.field = field
        self.code = code

    def envelope(self):
        return {"code": self.code, "message": str(self), "field": self.field, "retriable": False}


def _fail(message, field, code="INVALID_SCHEMA"):
    raise ProtocolError(message, field, code)


def _object(value, fields, path):
    if type(value) is not dict or set(value) != set(fields):
        _fail("expected exact fields: " + ", ".join(fields), path)


def _number(value, path, positive=False):
    if type(value) not in (int, float) or not math.isfinite(value) or abs(value) > 10000:
        _fail("expected finite bounded number", path)
    if positive and value <= 0:
        _fail("expected positive number", path)
    return value


def _point(value, path):
    if type(value) is not list or len(value) != 2:
        _fail("expected two coordinates", path)
    return [_number(v, path) for v in value]


def _ring(value, count, path):
    if type(value) is not list or len(value) != count:
        _fail("invalid point count", path, "INVALID_GEOMETRY")
    points = [_point(v, path) for v in value]
    if points[0] != points[-1]:
        _fail("ring must be closed", path, "INVALID_GEOMETRY")
    return points


def _id(value, regex, path):
    if type(value) is not str or not regex.fullmatch(value):
        _fail("invalid identifier", path)


def _fixed(value, expected, path):
    if type(expected) is float:
        if type(value) not in (int, float) or not math.isfinite(value) or abs(value - expected) > 1e-6:
            _fail("unexpected fixed setting", path)
    elif type(value) is not type(expected) or value != expected:
        _fail("unexpected fixed setting", path)


def _numbers(value, count, path):
    if type(value) is not list or len(value) != count:
        _fail("invalid numeric vector", path)
    for index, item in enumerate(value):
        if type(item) not in (int, float) or not math.isfinite(item) or abs(item) > 1000000:
            _fail("nonfinite or out-of-range vector element", f"{path}.{index}")
    return value


def _matrix(value, path):
    if type(value) is not list or len(value) != 4:
        _fail("expected 4x4 matrix", path)
    for index, row in enumerate(value):
        _numbers(row, 4, f"{path}.{index}")
    if any(abs(value[3][index] - expected) > 1e-6 for index, expected in enumerate((0, 0, 0, 1))):
        _fail("nonaffine matrix", path)


def _positive(value, path, zero=False):
    if type(value) not in (int, float) or not math.isfinite(value) or (value < 0 if zero else value <= 0):
        _fail("invalid positive finite value", path)


def _validate_preset(preset, storeys):
    fields = ("preset_id", "engine", "device", "threads", "threads_mode", "samples", "seed",
              "adaptive_sampling", "denoising", "resolution", "resolution_percentage",
              "format", "image_compression", "film_transparent", "display_device",
              "view_transform", "look", "exposure", "gamma", "axis_conversion",
              "frame", "unit", "world", "material", "cameras", "objects", "light",
              "glb_export", "view_bounds_m", "saved_render_path")
    _object(preset, fields, "preset")
    fixed = {"preset_id": "smoke-cpu-v1", "engine": "CYCLES", "device": "CPU",
             "threads": 1, "threads_mode": "FIXED", "samples": 16, "seed": 42, "adaptive_sampling": False,
             "denoising": False, "resolution": [512, 512], "resolution_percentage": 100,
             "format": "PNG_RGBA8", "image_compression": 15, "film_transparent": False,
             "display_device": "sRGB", "view_transform": "Standard", "look": "Medium High Contrast",
             "exposure": 0.0, "gamma": 1.0,
             "axis_conversion": "Blender Z-up to glTF Y-up", "frame": 1,
             "saved_render_path": "//view-iso.png"}
    for key, expected in fixed.items():
        _fixed(preset[key], expected, "preset." + key)
    _object(preset["unit"], ("system", "scale_length"), "preset.unit")
    _fixed(preset["unit"]["system"], "METRIC", "preset.unit.system")
    _fixed(preset["unit"]["scale_length"], 1.0, "preset.unit.scale_length")
    world = preset["world"]
    _object(world, ("use_nodes", "color_rgba", "strength"), "preset.world")
    _fixed(world["use_nodes"], True, "preset.world.use_nodes")
    _numbers(world["color_rgba"], 4, "preset.world.color_rgba")
    for index, expected in enumerate((0.12, 0.15, 0.2, 1.0)):
        _fixed(world["color_rgba"][index], expected, "preset.world.color_rgba")
    _fixed(world["strength"], 0.6, "preset.world.strength")
    material = preset["material"]
    _object(material, ("use_nodes", "diffuse_rgba", "base_color_rgba", "roughness"), "preset.material")
    _fixed(material["use_nodes"], True, "preset.material.use_nodes")
    for key in ("diffuse_rgba", "base_color_rgba"):
        _numbers(material[key], 4, "preset.material." + key)
        for index, expected in enumerate((0.3, 0.55, 0.8, 1.0)):
            _fixed(material[key][index], expected, "preset.material." + key)
    _fixed(material["roughness"], 0.8, "preset.material.roughness")
    glb = preset["glb_export"]
    _object(glb, ("format", "selected_only", "export_yup", "animations", "cameras", "lights", "compression"), "preset.glb_export")
    for key, expected in {"format": "GLB", "selected_only": True, "export_yup": True,
                          "animations": False, "cameras": False, "lights": False,
                          "compression": False}.items():
        _fixed(glb[key], expected, "preset.glb_export." + key)
    bounds = _numbers(preset["view_bounds_m"], 6, "preset.view_bounds_m")
    if any(bounds[index] >= bounds[index + 3] for index in range(3)):
        _fail("invalid view bounds", "preset.view_bounds_m")
    cameras = preset["cameras"]
    _object(cameras, ("top", "front", "iso"), "preset.cameras")
    for view, camera in cameras.items():
        path = "preset.cameras." + view
        _object(camera, ("name", "type", "matrix_world", "ortho_scale", "clip_start", "clip_end"), path)
        _fixed(camera["name"], "Camera_" + view, path + ".name")
        _fixed(camera["type"], "ORTHO", path + ".type")
        _matrix(camera["matrix_world"], path + ".matrix_world")
        _positive(camera["ortho_scale"], path + ".ortho_scale")
        _fixed(camera["clip_start"], 0.01, path + ".clip_start")
        _positive(camera["clip_end"], path + ".clip_end")
        if camera["clip_end"] <= camera["clip_start"]:
            _fail("invalid clip interval", path)
    objects = preset["objects"]
    if type(objects) is not list or len(objects) != storeys:
        _fail("storey object count mismatch", "preset.objects")
    for index, obj in enumerate(objects):
        path = f"preset.objects.{index}"
        _object(obj, ("name", "location", "rotation_euler", "scale", "matrix_world"), path)
        _fixed(obj["name"], f"Storey_{index:03d}", path + ".name")
        _numbers(obj["location"], 3, path + ".location")
        _numbers(obj["rotation_euler"], 3, path + ".rotation_euler")
        _numbers(obj["scale"], 3, path + ".scale")
        for j in range(3):
            _fixed(obj["rotation_euler"][j], 0.0, path + ".rotation_euler")
            _fixed(obj["scale"][j], 1.0, path + ".scale")
        _matrix(obj["matrix_world"], path + ".matrix_world")
    light = preset["light"]
    _object(light, ("type", "shape", "color_rgb", "location", "rotation_euler",
                    "matrix_world", "energy", "size"), "preset.light")
    for key, expected in (("type", "AREA"), ("shape", "DISK"), ("energy", 3000.0), ("size", 10.0)):
        _fixed(light[key], expected, "preset.light." + key)
    for key in ("color_rgb", "location", "rotation_euler"):
        _numbers(light[key], 3, "preset.light." + key)
    for value in light["color_rgb"]:
        _fixed(value, 1.0, "preset.light.color_rgb")
    for value in light["rotation_euler"]:
        _fixed(value, 0.0, "preset.light.rotation_euler")
    _matrix(light["matrix_world"], "preset.light.matrix_world")


def _relative(value):
    if type(value) is not str or len(value) > 512 or not value or "\\" in value or ":" in value:
        _fail("invalid logical output directory", "output_dir")
    parts = value.split("/")
    if any(not ID.fullmatch(p) or p in (".", "..") for p in parts):
        _fail("invalid logical output directory", "output_dir")


def _duplicate_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            _fail("duplicate JSON key", key)
        result[key] = value
    return result


def parse_request(raw):
    if type(raw) is not bytes or len(raw) > MAX_REQUEST_BYTES:
        _fail("request exceeds 1 MiB or is not bytes", "$")
    try:
        request = json.loads(raw.decode("utf-8"), object_pairs_hook=_duplicate_pairs,
                             parse_constant=lambda value: _fail("nonfinite JSON", "$"))
    except (UnicodeError, json.JSONDecodeError) as exc:
        _fail("invalid UTF-8 JSON: " + str(exc), "$")
    return validate_request(request)


def validate_request(request):
    _object(request, ("schema_version", "operation", "request_id", "project_id",
                      "variant_id", "run_id", "params", "site", "input_digest",
                      "preset_id", "output_dir"), "$")
    for field, expected in (("schema_version", SCHEMA_VERSION), ("operation", "build_massing"),
                            ("preset_id", "smoke-cpu-v1")):
        if request[field] != expected or type(request[field]) is not str:
            _fail("unsupported " + field, field)
    for field in ("request_id", "project_id", "run_id"):
        _id(request[field], ID, field)
    _id(request["variant_id"], VARIANT_ID, "variant_id")
    _id(request["input_digest"], DIGEST, "input_digest")
    _relative(request["output_dir"])
    site = request["site"]
    _object(site, ("polygon_xy_m", "north_angle_deg"), "site")
    _ring(site["polygon_xy_m"], 5, "site.polygon_xy_m")
    _number(site["north_angle_deg"], "site.north_angle_deg")
    variant = request["params"]
    _object(variant, ("schema_version", "project_id", "id", "seed",
                      "generation_algorithm", "shape", "parameters", "footprint_xy_m",
                      "storeys", "floor_to_floor_m"), "params")
    if variant["schema_version"] != "0.1.0" or variant["project_id"] != request["project_id"] or variant["id"] != request["variant_id"]:
        _fail("variant association or schema mismatch", "params")
    if type(variant["generation_algorithm"]) is not str or not ID.fullmatch(variant["generation_algorithm"]):
        _fail("invalid generation algorithm", "params.generation_algorithm")
    if type(variant["seed"]) is not int or not 0 <= variant["seed"] <= 2**64 - 1:
        _fail("invalid seed", "params.seed")
    if type(variant["storeys"]) is not int or not 1 <= variant["storeys"] <= 100:
        _fail("unsupported backend storey capacity", "params.storeys", "BACKEND_UNAVAILABLE")
    height = _number(variant["floor_to_floor_m"], "params.floor_to_floor_m", True)
    if height * variant["storeys"] > 10000:
        _fail("unsupported backend height capacity", "params.storeys", "BACKEND_UNAVAILABLE")
    shape = variant["shape"]
    if shape not in ("rectangle", "l_shape"):
        _fail("unsupported shape", "params.shape")
    p = variant["parameters"]
    fields = ["shape", "origin_xy_m", "width_m", "depth_m"]
    if shape == "l_shape":
        fields += ["notch_width_m", "notch_depth_m"]
    _object(p, fields, "params.parameters")
    if p["shape"] != shape:
        _fail("shape mismatch", "params.parameters.shape")
    x, y = _point(p["origin_xy_m"], "params.parameters.origin_xy_m")
    w = _number(p["width_m"], "params.parameters.width_m", True)
    d = _number(p["depth_m"], "params.parameters.depth_m", True)
    if min(w, d) <= 1e-6:
        _fail("degenerate dimension", "params.parameters", "INVALID_GEOMETRY")
    if shape == "rectangle":
        expected = [[x, y], [x+w, y], [x+w, y+d], [x, y+d], [x, y]]
    else:
        nw = _number(p["notch_width_m"], "params.parameters.notch_width_m", True)
        nd = _number(p["notch_depth_m"], "params.parameters.notch_depth_m", True)
        if min(nw, nd, w-nw, d-nd) <= 1e-6:
            _fail("malformed L notch", "params.parameters", "INVALID_GEOMETRY")
        expected = [[x, y], [x+w, y], [x+w, y+d-nd], [x+w-nw, y+d-nd],
                    [x+w-nw, y+d], [x, y+d], [x, y]]
    actual = _ring(variant["footprint_xy_m"], len(expected), "params.footprint_xy_m")
    for got, want in zip(actual, expected):
        if any(abs(a-b) > 1e-9 for a, b in zip(got, want)):
            _fail("footprint differs from parameters", "params.footprint_xy_m", "INVALID_GEOMETRY")
    for point in expected:
        _point(point, "params.footprint_xy_m")
    return request


def validate_response(response):
    if type(response) is not dict or response.get("schema_version") != SCHEMA_VERSION:
        _fail("invalid response schema", "$")
    if response.get("status") == "error":
        _object(response, ("schema_version", "status", "error", "artifacts"), "$")
        error = response["error"]
        _object(error, ("code", "message", "field", "retriable"), "error")
        if type(response["artifacts"]) is not list or response["artifacts"]:
            _fail("error cannot accept artifacts", "artifacts")
        if error["code"] not in ("INVALID_SCHEMA", "INVALID_GEOMETRY", "BACKEND_UNAVAILABLE", "INTERNAL_ERROR"):
            _fail("unsupported error code", "error.code")
        if any(type(error[key]) is not str for key in ("message", "field")) or type(error["retriable"]) is not bool:
            _fail("invalid error envelope", "error")
        return response
    _object(response, ("schema_version", "request_id", "project_id", "variant_id", "run_id",
                       "request_digest", "worker_version", "blender_version", "build_hash",
                       "python_version", "status", "warnings", "preset", "geometry", "artifacts"), "$")
    if response["status"] != "ok" or response["worker_version"] != SCHEMA_VERSION:
        _fail("invalid response status or worker version", "status")
    for field in ("request_id", "project_id", "run_id"):
        _id(response[field], ID, field)
    _id(response["variant_id"], VARIANT_ID, "variant_id")
    _id(response["request_digest"], DIGEST, "request_digest")
    for field in ("blender_version", "build_hash", "python_version"):
        if type(response[field]) is not str or not response[field]:
            _fail("missing runtime version", field)
    if type(response["warnings"]) is not list or any(type(value) is not str for value in response["warnings"]):
        _fail("invalid warnings", "warnings")
    geometry = response["geometry"]
    _object(geometry, ("storeys", "footprint_area_m2", "volume_m3", "bounds_m",
                       "world_coordinate_max_error_m", "cap_area_error_m2"), "geometry")
    if type(geometry["storeys"]) is not int or not 1 <= geometry["storeys"] <= 100:
        _fail("invalid storey count", "geometry.storeys")
    for field in ("footprint_area_m2", "volume_m3"):
        _positive(geometry[field], "geometry." + field)
    for field in ("world_coordinate_max_error_m", "cap_area_error_m2"):
        _positive(geometry[field], "geometry." + field, zero=True)
    if geometry["world_coordinate_max_error_m"] > 1e-6:
        _fail("world coordinate parity exceeded", "geometry.world_coordinate_max_error_m")
    bounds = _numbers(geometry["bounds_m"], 6, "geometry.bounds_m")
    if any(bounds[index] >= bounds[index + 3] for index in range(3)):
        _fail("invalid geometry bounds", "geometry.bounds_m")
    _validate_preset(response["preset"], geometry["storeys"])
    media = {"scene.blend": "application/x-blender", "model.glb": "model/gltf-binary",
             "view-top.png": "image/png", "view-front.png": "image/png", "view-iso.png": "image/png"}
    artifacts = response["artifacts"]
    if type(artifacts) is not list or len(artifacts) != len(media):
        _fail("incomplete artifact inventory", "artifacts")
    paths = []
    for index, artifact in enumerate(artifacts):
        path = f"artifacts.{index}"
        _object(artifact, ("path", "sha256", "size_bytes", "media_type", "producer"), path)
        name = artifact["path"]
        if type(name) is not str or name not in media or artifact["media_type"] != media[name]:
            _fail("invalid artifact path or media type", path)
        paths.append(name)
        _id(artifact["sha256"], DIGEST, path + ".sha256")
        if type(artifact["size_bytes"]) is not int or artifact["size_bytes"] <= 0:
            _fail("invalid artifact size", path + ".size_bytes")
        if artifact["producer"] != SCHEMA_VERSION:
            _fail("invalid producer", path + ".producer")
    if set(paths) != set(media):
        _fail("duplicate or missing artifact", "artifacts")
    return response
