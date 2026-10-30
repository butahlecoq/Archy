"""Real Blender camera coverage and preview presence for bounded massing shapes."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from test_protocol import ROOT, request


def shape_request(shape, aspect):
    item = request()
    if aspect == "wide":
        width, depth, storeys, floor = 200.0, 2.0, 1, 3.2
    elif aspect == "deep":
        width, depth, storeys, floor = 2.0, 200.0, 1, 3.2
    else:
        width, depth, storeys, floor = 2.0, 2.0, 10, 20.0
    x = y = 2.0
    params = {"shape": shape, "origin_xy_m": [x, y], "width_m": width, "depth_m": depth}
    if shape == "rectangle":
        ring = [[x, y], [x + width, y], [x + width, y + depth], [x, y + depth], [x, y]]
    else:
        notch_width, notch_depth = width / 4, depth / 4
        params.update(notch_width_m=notch_width, notch_depth_m=notch_depth)
        ring = [[x, y], [x + width, y], [x + width, y + depth - notch_depth],
                [x + width - notch_width, y + depth - notch_depth],
                [x + width - notch_width, y + depth], [x, y + depth], [x, y]]
    item["params"].update(shape=shape, parameters=params, footprint_xy_m=ring,
                          storeys=storeys, floor_to_floor_m=floor)
    item["site"]["polygon_xy_m"] = [[0, 0], [width + 4, 0], [width + 4, depth + 4],
                                      [0, depth + 4], [0, 0]]
    return item


@unittest.skipUnless(os.environ.get("ARCHY_BLENDER_BIN"), "set ARCHY_BLENDER_BIN for real Blender integration")
class RenderPresetTests(unittest.TestCase):
    def test_six_shape_aspects_frame_massing_with_visible_previews(self):
        import numpy as np
        from PIL import Image
        import protocol

        output_root = os.environ.get("ARCHY_A08_OUTPUT_DIR")
        if output_root:
            Path(output_root).mkdir(parents=True, exist_ok=True)
        for shape in ("rectangle", "l_shape"):
            for aspect in ("wide", "deep", "tall"):
                with self.subTest(shape=shape, aspect=aspect), tempfile.TemporaryDirectory(dir=output_root) as tmp:
                    job = Path(tmp)
                    item = shape_request(shape, aspect)
                    protocol.validate_request(item)
                    (job / "request.json").write_text(json.dumps(item))
                    command = [os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                               "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                               "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                               "--", "--request", str(job / "request.json"),
                               "--response", str(job / "response.json")]
                    result = subprocess.run(command, cwd=job, capture_output=True, text=True, timeout=120)
                    self.assertEqual(result.returncode, 0, result.stdout[-2000:] + result.stderr[-2000:])
                    response = json.loads((job / "response.json").read_text())
                    protocol.validate_response(response)
                    preset = response["preset"]
                    self.assertEqual(set(preset["cameras"]), {"top", "front", "iso"})
                    self.assertEqual((preset["engine"], preset["device"], preset["samples"],
                                      preset["seed"], preset["resolution"]),
                                     ("CYCLES", "CPU", 16, 42, [512, 512]))
                    self.assertEqual((preset["display_device"], preset["view_transform"], preset["look"]),
                                     ("sRGB", "Standard", "Medium High Contrast"))
                    self.assertAlmostEqual(preset["world"]["strength"], 0.6, places=6)
                    self.assertAlmostEqual(preset["material"]["roughness"], 0.8, places=6)
                    self.assertEqual(preset["light"]["type"], "AREA")
                    self.assertLess(sum((job / artifact["path"]).stat().st_size
                                        for artifact in response["artifacts"]), 64 * 1024 * 1024)

                    ring = item["params"]["footprint_xy_m"][:-1]
                    top = item["params"]["storeys"] * item["params"]["floor_to_floor_m"]
                    mass_points = [(x, y, z) for x, y in ring for z in (0.0, top)]
                    for view in ("top", "front", "iso"):
                        camera = preset["cameras"][view]
                        inverse = np.linalg.inv(np.asarray(camera["matrix_world"], dtype=float))
                        half = camera["ortho_scale"] / 2
                        mass_coords = np.array([inverse @ np.array([*point, 1.0]) for point in mass_points])
                        self.assertLessEqual(float(np.max(np.abs(mass_coords[:, :2]))), half,
                                             f"{shape} {aspect} {view}: massing outside camera frame")
                        depths = -mass_coords[:, 2]
                        self.assertGreater(float(np.min(depths)), camera["clip_start"])
                        self.assertLess(float(np.max(depths)), camera["clip_end"])
                        image_path = job / f"view-{view}.png"
                        with Image.open(image_path) as image:
                            self.assertEqual((image.format, image.size), ("PNG", (512, 512)))
                            image.load()
                            pixels = np.asarray(image.convert("RGB"), dtype=np.int16)
                        changed = np.max(np.abs(pixels - pixels[0, 0]), axis=2) > 8
                        self.assertGreater(int(np.count_nonzero(changed)), 64,
                                           f"{shape} {aspect} {view}: preview lacks visible geometry")
                        x_pixels = 256 + mass_coords[:, 0] * 512 / camera["ortho_scale"]
                        y_pixels = 256 - mass_coords[:, 1] * 512 / camera["ortho_scale"]
                        left, right = max(0, int(np.floor(x_pixels.min()))), min(512, int(np.ceil(x_pixels.max())) + 1)
                        top_px, bottom = max(0, int(np.floor(y_pixels.min()))), min(512, int(np.ceil(y_pixels.max())) + 1)
                        crop = pixels[top_px:bottom, left:right]
                        background = np.median(pixels[:16, :16], axis=(0, 1))
                        visible = np.max(np.abs(crop - background), axis=2) > 12
                        self.assertGreater(int(np.count_nonzero(visible)), 32,
                                           f"{shape} {aspect} {view}: no foreground inside projected massing")

    def test_thin_massing_edges_remain_visible_at_camera_resolution(self):
        from PIL import Image
        import protocol

        output_root = os.environ.get("ARCHY_A08_OUTPUT_DIR")
        if output_root:
            Path(output_root).mkdir(parents=True, exist_ok=True)
        for width, depth in ((200, 2), (200, 0.1), (200, 0.05), (200, 0.02),
                             (200, 0.005), (200, 0.0001), (0.05, 200), (0.0001, 200)):
            with self.subTest(width=width, depth=depth), tempfile.TemporaryDirectory(dir=output_root) as tmp:
                job = Path(tmp)
                item = shape_request("rectangle", "wide")
                item["params"]["parameters"]["width_m"] = width
                item["params"]["parameters"]["depth_m"] = depth
                item["params"]["footprint_xy_m"] = [[2, 2], [2 + width, 2], [2 + width, 2 + depth],
                                                       [2, 2 + depth], [2, 2]]
                item["site"]["polygon_xy_m"] = [[0, 0], [width + 4, 0], [width + 4, depth + 4],
                                                 [0, depth + 4], [0, 0]]
                protocol.validate_request(item)
                (job / "request.json").write_text(json.dumps(item))
                result = subprocess.run(
                    [os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                     "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                     "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                     "--", "--request", str(job / "request.json"),
                     "--response", str(job / "response.json")],
                    cwd=job, capture_output=True, text=True, timeout=120)
                self.assertEqual(result.returncode, 0, result.stdout[-2000:] + result.stderr[-2000:])
                response = json.loads((job / "response.json").read_text())
                protocol.validate_response(response)
                self.assertEqual(response["status"], "ok")
                self.assertEqual(bool(response["warnings"]), min(width, depth) < 2 * response["preset"]["cameras"]["top"]["ortho_scale"] / 512)
                self.assertLess(sum((job / artifact["path"]).stat().st_size
                                    for artifact in response["artifacts"]), 64 * 1024 * 1024)
                for view in ("top", "front", "iso"):
                    with Image.open(job / f"view-{view}.png") as image:
                        self.assertEqual((image.size, image.mode), ((512, 512), "RGBA"))
                        pixels = image.convert("RGB")
                    background = pixels.getpixel((0, 0))
                    changed = sum(max(abs(a - b) for a, b in zip(pixel, background)) > 8
                                  for pixel in pixels.get_flattened_data())
                    self.assertGreaterEqual(changed, 128, (depth, view, changed))

    def test_positive_footprint_below_blender_tessellation_resolution_reports_backend_limit(self):
        import protocol

        with tempfile.TemporaryDirectory(dir=os.environ.get("ARCHY_A08_OUTPUT_DIR")) as tmp:
            job = Path(tmp)
            item = shape_request("rectangle", "wide")
            depth = 0.000002
            item["params"]["parameters"]["depth_m"] = depth
            item["params"]["footprint_xy_m"] = [[2, 2], [202, 2], [202, 2 + depth],
                                                   [2, 2 + depth], [2, 2]]
            item["site"]["polygon_xy_m"] = [[0, 0], [204, 0], [204, depth + 4],
                                             [0, depth + 4], [0, 0]]
            protocol.validate_request(item)
            (job / "request.json").write_text(json.dumps(item))
            result = subprocess.run(
                [os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                 "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                 "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                 "--", "--request", str(job / "request.json"),
                 "--response", str(job / "response.json")],
                cwd=job, capture_output=True, text=True, timeout=120)
            self.assertEqual(result.returncode, 20)
            response = json.loads((job / "response.json").read_text())
            protocol.validate_response(response)
            self.assertEqual(response["error"]["code"], "BACKEND_UNAVAILABLE")
            self.assertIn("tessellation cannot resolve", response["error"]["message"])
            self.assertEqual(response["artifacts"], [])

    def test_ten_micrometer_orientations_report_specific_backend_limits(self):
        import protocol

        for width, depth in ((200.0, 0.00001), (0.00001, 200.0)):
            with self.subTest(width=width, depth=depth), tempfile.TemporaryDirectory(
                dir=os.environ.get("ARCHY_A08_OUTPUT_DIR")
            ) as tmp:
                job = Path(tmp)
                item = shape_request("rectangle", "wide")
                item["params"]["parameters"].update(width_m=width, depth_m=depth)
                item["params"]["footprint_xy_m"] = [[2, 2], [2 + width, 2],
                    [2 + width, 2 + depth], [2, 2 + depth], [2, 2]]
                item["site"]["polygon_xy_m"] = [[0, 0], [width + 4, 0],
                    [width + 4, depth + 4], [0, depth + 4], [0, 0]]
                protocol.validate_request(item)
                (job / "request.json").write_text(json.dumps(item))
                result = subprocess.run(
                    [os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                     "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                     "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                     "--", "--request", str(job / "request.json"),
                     "--response", str(job / "response.json")],
                    cwd=job, capture_output=True, text=True, timeout=120)
                self.assertEqual(result.returncode, 20, result.stdout + result.stderr)
                response = json.loads((job / "response.json").read_text())
                protocol.validate_response(response)
                self.assertEqual(response["error"]["code"], "BACKEND_UNAVAILABLE")
                self.assertEqual(response["error"]["field"], "params.footprint_xy_m")
                self.assertIn("tessellation", response["error"]["message"])
                self.assertEqual(response["artifacts"], [])
                self.assertEqual(sorted(p.name for p in job.iterdir()), ["request.json", "response.json"])


if __name__ == "__main__":
    unittest.main()
