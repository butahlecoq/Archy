import json
import copy
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "workers" / "blender"))
import protocol


def request():
    variant = json.loads((ROOT / "tests/fixtures/generation/courtyard.seed42.json").read_text())["variants"][0]
    return {
        "schema_version": "blender-worker-v1", "operation": "build_massing",
        "request_id": "request-1", "project_id": variant["project_id"],
        "variant_id": variant["id"], "run_id": "run-1",
        "params": variant, "site": {
            "polygon_xy_m": [[0, 0], [20, 0], [20, 35], [0, 35], [0, 0]],
            "north_angle_deg": 0.0,
        },
        "input_digest": "a" * 64, "preset_id": "smoke-cpu-v1",
        "output_dir": "runs/run-1/blender",
    }


def success_response():
    item = request()
    names = ("scene.blend", "model.glb", "view-top.png", "view-front.png", "view-iso.png")
    matrix = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
              [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
    cameras = {view: {"name": "Camera_" + view, "type": "ORTHO",
                      "matrix_world": copy.deepcopy(matrix), "ortho_scale": 40.0,
                      "clip_start": 0.01, "clip_end": 1000.0} for view in ("top", "front", "iso")}
    return {"schema_version": protocol.SCHEMA_VERSION, "request_id": item["request_id"],
            "project_id": item["project_id"], "variant_id": item["variant_id"],
            "run_id": item["run_id"], "request_digest": item["input_digest"],
            "worker_version": "blender-worker-v1", "blender_version": "4.5.15 LTS",
            "build_hash": "5afeabba6097", "python_version": "3.11.15", "status": "ok",
            "warnings": [],
            "preset": {"preset_id": "smoke-cpu-v1", "engine": "CYCLES", "device": "CPU",
                       "threads": 1, "threads_mode": "FIXED", "samples": 16, "seed": 42, "adaptive_sampling": False,
                       "denoising": False, "resolution": [512, 512], "resolution_percentage": 100,
                       "format": "PNG_RGBA8", "image_compression": 15, "film_transparent": False,
                       "display_device": "sRGB", "view_transform": "Standard",
                       "look": "Medium High Contrast", "exposure": 0.0, "gamma": 1.0,
                       "axis_conversion": "Blender Z-up to glTF Y-up", "frame": 1,
                       "unit": {"system": "METRIC", "scale_length": 1.0},
                       "world": {"use_nodes": True, "color_rgba": [0.12, 0.15, 0.2, 1.0], "strength": 0.6},
                       "material": {"use_nodes": True, "diffuse_rgba": [0.3, 0.55, 0.8, 1.0],
                                    "base_color_rgba": [0.3, 0.55, 0.8, 1.0], "roughness": 0.8},
                       "cameras": cameras,
                       "objects": [{"name": f"Storey_{i:03d}", "location": [3.0, 3.0, i * 3.2],
                                    "rotation_euler": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0],
                                    "matrix_world": copy.deepcopy(matrix)} for i in range(3)],
                       "light": {"type": "AREA", "shape": "DISK", "color_rgb": [1.0, 1.0, 1.0],
                                 "location": [5.0, -5.0, 22.0], "rotation_euler": [0.0, 0.0, 0.0],
                                 "matrix_world": copy.deepcopy(matrix), "energy": 3000.0, "size": 10.0},
                       "glb_export": {"format": "GLB", "selected_only": True, "export_yup": True,
                                      "animations": False, "cameras": False, "lights": False,
                                      "compression": False},
                       "view_bounds_m": [0.0, 0.0, 0.0, 20.0, 35.0, 9.6],
                       "saved_render_path": "//view-iso.png"},
            "geometry": {"storeys": 3, "footprint_area_m2": 200.0, "volume_m3": 1920.0,
                         "bounds_m": [3.0, 3.0, 0.0, 13.0, 23.0, 9.6],
                         "world_coordinate_max_error_m": 0.0, "cap_area_error_m2": 0.0},
            "artifacts": [{"path": name, "sha256": "a" * 64, "size_bytes": 1,
                           "media_type": "image/png" if name.endswith("png") else ("model/gltf-binary" if name.endswith("glb") else "application/x-blender"),
                           "producer": "blender-worker-v1"} for name in names]}


class ProtocolTests(unittest.TestCase):
    def test_response_inventory_rejects_traversal_and_missing_files(self):
        response = success_response()
        self.assertIs(protocol.validate_response(response), response)
        response["artifacts"][0]["path"] = "../scene.blend"
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_response(response)
        response = success_response()
        response["artifacts"] = response["artifacts"][1:]
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_response(response)

    def test_response_rejects_nested_unknown_nonfinite_and_changed_fixed_settings(self):
        cases = []
        for nested, key, value in (("preset", "engine", "BLENDER_EEVEE_NEXT"),
                                   ("preset", "device", "GPU"),
                                   ("preset", "samples", 32),
                                   ("preset", "extra", "injected"),
                                   ("geometry", "volume_m3", float("nan")),
                                   ("geometry", "bounds_m", [0, 0, 0, float("inf"), 1, 1]),
                                   ("geometry", "world_coordinate_max_error_m", 1.1e-6)):
            response = success_response(); response[nested][key] = value; cases.append(response)
        for response in cases:
            with self.subTest(response=response), self.assertRaises(protocol.ProtocolError):
                protocol.validate_response(response)
        for path, key, value in (("world", "strength", float("nan")),
                                 ("material", "use_nodes", False),
                                 ("material", "roughness", 0.2),
                                 ("glb_export", "compression", True)):
            response = success_response(); response["preset"][path][key] = value
            with self.subTest(path=path, key=key), self.assertRaises(protocol.ProtocolError):
                protocol.validate_response(response)

    def test_valid_task02_variant(self):
        self.assertEqual(protocol.validate_request(request())["params"]["storeys"], 3)

    def test_duplicate_json_key(self):
        data = json.dumps(request()).replace('"operation":', '"operation": "build_massing", "operation":', 1)
        with self.assertRaises(protocol.ProtocolError):
            protocol.parse_request(data.encode())

    def test_unknown_and_bad_values(self):
        cases = []
        for field, value in (("operation", "exec"), ("preset_id", "custom"),
                             ("output_dir", "../escape"), ("request_id", "x;import os")):
            item = request(); item[field] = value; cases.append(item)
        item = request(); item["python"] = "print(1)"; cases.append(item)
        item = request(); item["params"]["storeys"] = 101; cases.append(item)
        item = request(); item["params"]["parameters"]["width_m"] = float("nan"); cases.append(item)
        item = request(); item["params"]["footprint_xy_m"][0][0] = 99; cases.append(item)
        item = request(); item["params"]["parameters"]["notch_width_m"] = 100; cases.append(item)
        item = request(); item["site"]["north_angle_deg"] = float("inf"); cases.append(item)
        for case in cases:
            with self.subTest(case=case), self.assertRaises(protocol.ProtocolError):
                protocol.validate_request(case)

    def test_rejects_nonfinite_json_and_oversized_input(self):
        with self.assertRaises(protocol.ProtocolError):
            protocol.parse_request(json.dumps(request()).replace("0.0", "NaN", 1).encode())
        with self.assertRaises(protocol.ProtocolError):
            protocol.parse_request(b" " * (1024 * 1024 + 1))
        for raw in (b"{", b"[]", b"{\"schema_version\": 1e999}"):
            with self.subTest(raw=raw), self.assertRaises(protocol.ProtocolError):
                protocol.parse_request(raw)

    def test_association_and_world_bounds(self):
        item = request(); item["project_id"] = "another-project"
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_request(item)
        item = request(); item["variant_id"] = "variant-" + "b" * 64
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_request(item)
        item = request(); item["params"]["parameters"]["origin_xy_m"] = [9999, 0]
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_request(item)

    def test_rectangle_from_analytic_source(self):
        item = request()
        case = json.loads((ROOT / "tests/fixtures/analytic/courtyard-shapes.json").read_text())["cases"][0]
        item["params"]["shape"] = "rectangle"
        item["params"]["parameters"] = {k: case[k] for k in ("shape", "origin_xy_m", "width_m", "depth_m")}
        item["params"]["footprint_xy_m"] = case["footprint_xy_m"]
        self.assertEqual(protocol.validate_request(item)["params"]["footprint_xy_m"], case["footprint_xy_m"])


if __name__ == "__main__":
    unittest.main()
