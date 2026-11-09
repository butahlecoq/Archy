"""Import-free request checks; Task02's canonical Variant remains authoritative."""

import hashlib
import json
import math
import re
import struct

MAX_REQUEST_BYTES = 1024 * 1024
SCHEMA_VERSION = "ifc-worker-v1"
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


def fail(message, field, code="INVALID_SCHEMA"):
    raise ProtocolError(message, field, code)


def exact(value, fields, field):
    if type(value) is not dict or set(value) != set(fields):
        fail("expected exact fields: " + ", ".join(fields), field)


def number(value, field, positive=False):
    if type(value) not in (int, float) or not math.isfinite(value) or abs(value) > 10000:
        fail("expected finite bounded number", field)
    if positive and value <= 1e-6:
        fail("dimension must exceed 1e-6 m", field, "INVALID_GEOMETRY")
    return float(value)


def point(value, field):
    if type(value) is not list or len(value) != 2:
        fail("expected XY point", field)
    return tuple(number(v, field) for v in value)


def relative(value, field):
    if type(value) is not str or not value or len(value) > 512 or "\\" in value or ":" in value:
        fail("unsafe logical path", field, "PERMISSION_DENIED")
    if any(not ID.fullmatch(p) or p in (".", "..") for p in value.split("/")):
        fail("unsafe logical path", field, "PERMISSION_DENIED")


def pairs_unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            fail("duplicate JSON key", key)
        result[key] = value
    return result


def parse_request(raw):
    if type(raw) is not bytes or len(raw) > MAX_REQUEST_BYTES:
        fail("request exceeds 1 MiB or is not bytes", "$")
    try:
        item = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs_unique,
                          parse_constant=lambda value: fail("nonfinite JSON", "$"))
    except ProtocolError:
        raise
    except (UnicodeError, json.JSONDecodeError, ValueError, RecursionError) as exc:
        fail("invalid UTF-8 JSON: " + str(exc), "$")
    return validate_request(item)


def canonical_id(v):
    h = hashlib.sha256(b"archy-variant-v1")
    for value in (v["schema_version"], v["project_id"], v["generation_algorithm"]):
        encoded = value.encode("utf-8")
        h.update(struct.pack(">Q", len(encoded)) + encoded)
    h.update(struct.pack(">QI", v["seed"], v["storeys"]))
    h.update(struct.pack(">d", v["floor_to_floor_m"]))
    h.update(bytes([0 if v["shape"] == "rectangle" else 1]))
    p = v["parameters"]
    for value in (*p["origin_xy_m"], p["width_m"], p["depth_m"]):
        h.update(struct.pack(">d", value))
    if v["shape"] == "l_shape":
        for value in (p["notch_width_m"], p["notch_depth_m"]):
            h.update(struct.pack(">d", value))
    return "variant-" + h.hexdigest()


def validate_request(item):
    exact(item, ("schema_version", "operation", "request_id", "project_id", "variant_id", "run_id",
                 "variant", "site", "source_digest", "source_artifacts", "output_dir"), "$")
    if item["schema_version"] != SCHEMA_VERSION or item["operation"] != "export_schematic_ifc":
        fail("unsupported protocol or operation", "operation")
    for key in ("request_id", "project_id", "run_id"):
        if type(item[key]) is not str or not ID.fullmatch(item[key]):
            fail("invalid identifier", key)
    if type(item["variant_id"]) is not str or not VARIANT_ID.fullmatch(item["variant_id"]):
        fail("invalid variant identifier", "variant_id")
    if type(item["source_digest"]) is not str or not DIGEST.fullmatch(item["source_digest"]):
        fail("invalid source digest", "source_digest")
    relative(item["output_dir"], "output_dir")
    refs = item["source_artifacts"]
    if type(refs) is not list or len(refs) > 16:
        fail("unsupported source reference count", "source_artifacts")
    for ref in refs:
        exact(ref, ("path", "sha256"), "source_artifacts")
        relative(ref["path"], "source_artifacts.path")
        if type(ref["sha256"]) is not str or not DIGEST.fullmatch(ref["sha256"]):
            fail("invalid source artifact digest", "source_artifacts.sha256")
    site = item["site"]
    exact(site, ("polygon_xy_m", "north_angle_deg"), "site")
    ring = site["polygon_xy_m"]
    if type(ring) is not list or len(ring) != 5:
        fail("unsupported site polygon", "site.polygon_xy_m", "INVALID_GEOMETRY")
    sites = [point(p, "site.polygon_xy_m") for p in ring]
    if sites[0] != sites[-1] or sites[0][0] >= sites[1][0] or sites[1][1] >= sites[2][1] or sites[0][1] >= sites[2][1] or sites[0][0] >= sites[2][0] or sites[0][1] != sites[1][1] or sites[1][0] != sites[2][0] or sites[2][1] != sites[3][1] or sites[3][0] != sites[0][0]:
        fail("site must be a canonical rectangle", "site.polygon_xy_m", "INVALID_GEOMETRY")
    number(site["north_angle_deg"], "site.north_angle_deg")
    v = item["variant"]
    exact(v, ("schema_version", "project_id", "id", "seed", "generation_algorithm", "shape",
              "parameters", "footprint_xy_m", "storeys", "floor_to_floor_m"), "variant")
    if v["schema_version"] != "0.1.0" or v["project_id"] != item["project_id"] or v["id"] != item["variant_id"]:
        fail("variant association or schema mismatch", "variant")
    if type(v["generation_algorithm"]) is not str or not ID.fullmatch(v["generation_algorithm"]):
        fail("invalid algorithm", "variant.generation_algorithm")
    if type(v["seed"]) is not int or not 0 <= v["seed"] <= 2**64 - 1:
        fail("invalid seed", "variant.seed")
    if type(v["storeys"]) is not int or not 1 <= v["storeys"] <= 100:
        fail("unsupported storey capacity", "variant.storeys", "INVALID_GEOMETRY")
    height = number(v["floor_to_floor_m"], "variant.floor_to_floor_m", True)
    if height * v["storeys"] > 10000:
        fail("unsupported height capacity", "variant.storeys", "INVALID_GEOMETRY")
    shape = v["shape"]
    if type(shape) is not str or shape not in ("rectangle", "l_shape"):
        fail("unsupported shape", "variant.shape")
    p = v["parameters"]
    fields = ["shape", "origin_xy_m", "width_m", "depth_m"]
    if shape == "l_shape":
        fields += ["notch_width_m", "notch_depth_m"]
    exact(p, fields, "variant.parameters")
    if p["shape"] != shape:
        fail("shape mismatch", "variant.parameters.shape")
    origin = point(p["origin_xy_m"], "variant.parameters.origin_xy_m")
    w = number(p["width_m"], "variant.parameters.width_m", True)
    d = number(p["depth_m"], "variant.parameters.depth_m", True)
    x, y = origin
    right, top = x + w, y + d
    if shape == "rectangle":
        expected = [(x, y), (right, y), (right, top), (x, top), (x, y)]
    else:
        nw = number(p["notch_width_m"], "variant.parameters.notch_width_m", True)
        nd = number(p["notch_depth_m"], "variant.parameters.notch_depth_m", True)
        if w - nw <= 1e-6 or d - nd <= 1e-6:
            fail("malformed L notch", "variant.parameters", "INVALID_GEOMETRY")
        expected = [(x, y), (right, y), (right, top - nd), (right - nw, top - nd),
                    (right - nw, top), (x, top), (x, y)]
    if any(abs(c) > 10000 or not math.isfinite(c) for pt in expected for c in pt):
        fail("unsupported coordinate capacity", "variant.parameters", "INVALID_GEOMETRY")
    if any(sum(abs(a - b) for a, b in zip(expected[i], expected[i + 1])) <= 1e-6 for i in range(len(expected) - 1)):
        fail("collapsed edge", "variant.parameters", "INVALID_GEOMETRY")
    actual = v["footprint_xy_m"]
    if type(actual) is not list or len(actual) != len(expected):
        fail("invalid footprint vertex count", "variant.footprint_xy_m", "INVALID_GEOMETRY")
    for stored, derived in zip(actual, expected):
        got = point(stored, "variant.footprint_xy_m")
        if any(struct.pack(">d", a) != struct.pack(">d", b) for a, b in zip(got, derived)):
            fail("footprint differs from canonical parameters", "variant.footprint_xy_m", "INVALID_GEOMETRY")
    values = (*origin, w, d, height) + ((nw, nd) if shape == "l_shape" else ())
    for value in values:
        if value == 0 and math.copysign(1, value) < 0:
            fail("negative zero is noncanonical", "variant.parameters", "INVALID_GEOMETRY")
    if canonical_id(v) != v["id"]:
        fail("variant ID differs from Task02 canonical identity", "variant.id", "INVALID_GEOMETRY")
    return item
