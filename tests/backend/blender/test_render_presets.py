"""Real Blender camera coverage and preview presence for bounded A08 shapes."""

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
    def test_six_shape_aspects_frame_site_and_massing_with_visible_previews(self):
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
                    site_points = [(x, y, 0.0) for x, y in item["site"]["polygon_xy_m"][:-1]]
                    for view in ("top", "front", "iso"):
                        camera = preset["cameras"][view]
                        inverse = np.linalg.inv(np.asarray(camera["matrix_world"], dtype=float))
                        half = camera["ortho_scale"] / 2
                        for label, points in (("massing", mass_points), ("site", site_points)):
                            coords = np.array([inverse @ np.array([*point, 1.0]) for point in points])
                            self.assertLessEqual(float(np.max(np.abs(coords[:, :2]))), half,
                                                 f"{shape} {aspect} {view}: {label} outside camera frame")
                            depths = -coords[:, 2]
                            self.assertGreater(float(np.min(depths)), camera["clip_start"])
                            self.assertLess(float(np.max(depths)), camera["clip_end"])
                            if label == "massing":
                                mass_coords = coords
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


if __name__ == "__main__":
    unittest.main()
