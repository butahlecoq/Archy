import hashlib
import json
import os
import struct
import subprocess
import tempfile
import unittest
from pathlib import Path


from test_protocol import ROOT, request


def far_request(x, y, height=3.2):
    item = request()
    p = item["params"]
    p["shape"] = "rectangle"
    p["parameters"] = {"shape": "rectangle", "origin_xy_m": [x, y], "width_m": 2.0, "depth_m": 3.0}
    p["footprint_xy_m"] = [[x, y], [x + 2, y], [x + 2, y + 3], [x, y + 3], [x, y]]
    p["floor_to_floor_m"] = height
    item["site"]["polygon_xy_m"] = [[x - 1, y - 1], [x + 4, y - 1], [x + 4, y + 5], [x - 1, y + 5], [x - 1, y - 1]]
    return item


@unittest.skipUnless(os.environ.get("ARCHY_BLENDER_BIN"), "set ARCHY_BLENDER_BIN for real Blender integration")
class BlenderIntegrationTests(unittest.TestCase):
    def test_far_inexact_world_coordinates_fail_capacity(self):
        for x in (9990.12345, -9990.12345):
            with self.subTest(x=x), tempfile.TemporaryDirectory() as directory:
                job = Path(directory)
                (job / "request.json").write_text(json.dumps(far_request(x, -5.12345)))
                result = subprocess.run([os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                                         "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                                         "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                                         "--", "--request", str(job / "request.json"), "--response",
                                         str(job / "response.json")], cwd=job, capture_output=True, text=True, timeout=60)
                self.assertEqual(result.returncode, 20)
                response = json.loads((job / "response.json").read_text())
                self.assertEqual(response["status"], "error")
                self.assertEqual(response["error"]["code"], "BACKEND_UNAVAILABLE")
                self.assertIn("1e-6", response["error"]["message"])
                self.assertEqual(response["artifacts"], [])
                self.assertFalse((job / "model.glb").exists())

    def test_far_exact_world_coordinates_export_without_translation_loss(self):
        import numpy as np
        import trimesh
        for x in (9990.125, -9990.125):
            with self.subTest(x=x), tempfile.TemporaryDirectory() as directory:
                job = Path(directory)
                item = far_request(x, -5.125, height=3.25)
                (job / "request.json").write_text(json.dumps(item))
                result = subprocess.run([os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                                         "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                                         "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                                         "--", "--request", str(job / "request.json"), "--response",
                                         str(job / "response.json")], cwd=job, capture_output=True, text=True, timeout=180)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                response = json.loads((job / "response.json").read_text())
                self.assertEqual(response["status"], "ok")
                exported = trimesh.load(job / "model.glb", force="scene", process=False)
                bounds = []
                for node in exported.graph.nodes_geometry:
                    transform, name = exported.graph[node]
                    mesh = exported.geometry[name].copy(); mesh.apply_transform(transform)
                    verts = np.asarray(mesh.vertices)
                    world = np.column_stack((verts[:, 0], -verts[:, 2], verts[:, 1]))
                    z0 = round(float(world[:, 2].min()) / 3.25) * 3.25
                    expected = np.array([[px, py, pz] for px, py in item["params"]["footprint_xy_m"][:-1]
                                         for pz in (z0, z0 + 3.25)])
                    for vertex in world:
                        self.assertLessEqual(np.min(np.max(np.abs(expected - vertex), axis=1)), 1e-6)
                    for vertex in expected:
                        self.assertLessEqual(np.min(np.max(np.abs(world - vertex), axis=1)), 1e-6)
                    bounds.append((world.min(axis=0), world.max(axis=0)))
                np.testing.assert_allclose(np.min([b[0] for b in bounds], axis=0), [x, -5.125, 0], atol=1e-6, rtol=0)
                np.testing.assert_allclose(np.max([b[1] for b in bounds], axis=0), [x + 2, -2.125, 9.75], atol=1e-6, rtol=0)

    def test_repeat_semantic_scene_and_preset(self):
        import trimesh
        cases = json.loads((ROOT / "tests/fixtures/analytic/courtyard-shapes.json").read_text())["cases"]
        for case in cases:
            observations = []
            for _ in range(2):
                with tempfile.TemporaryDirectory() as directory:
                    job = Path(directory)
                    item = request()
                    shape = case["shape"]
                    keys = ("shape", "origin_xy_m", "width_m", "depth_m") if shape == "rectangle" else ("shape", "origin_xy_m", "width_m", "depth_m", "notch_width_m", "notch_depth_m")
                    item["params"]["shape"] = shape
                    item["params"]["parameters"] = {key: case[key] for key in keys}
                    item["params"]["footprint_xy_m"] = case["footprint_xy_m"]
                    (job / "request.json").write_text(json.dumps(item))
                    result = subprocess.run([os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                                             "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                                             "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                                             "--", "--request", str(job / "request.json"), "--response",
                                             str(job / "response.json")], cwd=job, capture_output=True, text=True, timeout=180)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    response = json.loads((job / "response.json").read_text())
                    scene = trimesh.load(job / "model.glb", force="scene", process=False)
                    observations.append((response["geometry"], response["preset"],
                                         sorted((len(scene.geometry[name].vertices), len(scene.geometry[name].faces))
                                                for name in scene.geometry)))
            self.assertEqual(observations[0], observations[1], case["shape"])

    def test_source_variant_translation_and_north_metadata(self):
        import numpy as np
        import trimesh
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            item = request()
            item["site"]["north_angle_deg"] = 27.0
            (job / "request.json").write_text(json.dumps(item))
            result = subprocess.run([os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                                     "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                                     "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                                     "--", "--request", str(job / "request.json"), "--response",
                                     str(job / "response.json")], cwd=job, capture_output=True, text=True, timeout=180)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            response = json.loads((job / "response.json").read_text())
            self.assertEqual(response["status"], "ok")
            scene = trimesh.load(job / "model.glb", force="scene", process=False)
            mins = []
            for node in scene.graph.nodes_geometry:
                transform, name = scene.graph[node]
                mesh = scene.geometry[name].copy(); mesh.apply_transform(transform)
                verts = np.asarray(mesh.vertices)
                mins.append(np.column_stack((verts[:, 0], -verts[:, 2], verts[:, 1])).min(axis=0))
            np.testing.assert_allclose(np.min(mins, axis=0)[:2], item["params"]["parameters"]["origin_xy_m"], atol=1e-6, rtol=0)
            self.assertEqual(response["geometry"]["storeys"], 3)
            reopen = subprocess.run([os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                                     "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                                     "--threads", "1", str(job / "scene.blend"), "--python",
                                     str(ROOT / "workers/blender/smoke.py")],
                                    cwd=job, capture_output=True, text=True, timeout=30)
            self.assertEqual(reopen.returncode, 0, reopen.stdout + reopen.stderr)
            self.assertIn(" 27.0", reopen.stdout)

    def test_collision_and_symlink_preserve_sentinels(self):
        for kind in ("collision", "symlink"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                job = Path(directory)
                item = request()
                (job / "request.json").write_text(json.dumps(item))
                sentinel = job / "prior.txt"
                sentinel.write_text("prior run")
                if kind == "collision":
                    (job / "model.glb").write_text("prior artifact")
                else:
                    (job / "response.json").symlink_to(sentinel)
                result = subprocess.run([os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                                         "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                                         "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                                         "--", "--request", str(job / "request.json"), "--response",
                                         str(job / "response.json")], cwd=job, capture_output=True, text=True, timeout=30)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(sentinel.read_text(), "prior run")
                self.assertFalse((job / "scene.blend").exists())
                if kind == "collision":
                    self.assertEqual((job / "model.glb").read_text(), "prior artifact")

    def test_rejects_traversal_without_touching_outside(self):
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            sentinel = job / "outside.txt"
            sentinel.write_text("unchanged")
            item = request()
            item["output_dir"] = "../outside"
            (job / "request.json").write_text(json.dumps(item))
            result = subprocess.run([os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                                     "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                                     "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                                     "--", "--request", str(job / "request.json"), "--response",
                                     str(job / "response.json")], cwd=job, capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 20)
            response = json.loads((job / "response.json").read_text())
            self.assertEqual(response["status"], "error")
            self.assertEqual(response["artifacts"], [])
            self.assertEqual(sentinel.read_text(), "unchanged")
            self.assertFalse((job / "scene.blend").exists())

    def test_rectangle_and_l_exports(self):
        import numpy as np
        import trimesh
        from PIL import Image
        for shape in ("rectangle", "l_shape"):
            with self.subTest(shape=shape), tempfile.TemporaryDirectory() as directory:
                job = Path(directory)
                case = next(c for c in json.loads((ROOT / "tests/fixtures/analytic/courtyard-shapes.json").read_text())["cases"] if c["shape"] == shape)
                item = request()
                item["params"]["shape"] = shape
                item["params"]["parameters"] = {key: case[key] for key in (("shape", "origin_xy_m", "width_m", "depth_m") if shape == "rectangle" else ("shape", "origin_xy_m", "width_m", "depth_m", "notch_width_m", "notch_depth_m"))}
                item["params"]["footprint_xy_m"] = case["footprint_xy_m"]
                input_path = job / "request.json"
                output_path = job / "response.json"
                input_path.write_text(json.dumps(item))
                cmd = [os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup", "--disable-autoexec", "--offline-mode", "--python-exit-code", "20", "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"), "--", "--request", str(input_path), "--response", str(output_path)]
                result = subprocess.run(cmd, cwd=job, capture_output=True, text=True, timeout=180)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                response = json.loads(output_path.read_text())
                self.assertEqual(response["status"], "ok", response)
                names = {entry["path"] for entry in response["artifacts"]}
                self.assertEqual(names, {"scene.blend", "model.glb", "view-top.png", "view-front.png", "view-iso.png"})
                self.assertEqual({path.name for path in job.iterdir()} - {"request.json", "response.json"}, names)
                for entry in response["artifacts"]:
                    data = (job / entry["path"]).read_bytes()
                    self.assertEqual(entry["sha256"], hashlib.sha256(data).hexdigest())
                    self.assertEqual(entry["size_bytes"], len(data))
                for view in ("top", "front", "iso"):
                    image_path = job / f"view-{view}.png"
                    with Image.open(image_path) as image:
                        self.assertEqual((image.format, image.size, image.n_frames), ("PNG", (512, 512), 1))
                        image.verify()
                    with Image.open(image_path) as image:
                        image.load()
                        self.assertEqual(image.mode, "RGBA")
                        pixels = np.asarray(image.convert("RGB"))
                        foreground = np.count_nonzero(np.max(np.abs(pixels.astype(np.int16) - pixels[0, 0]), axis=2) > 8)
                        self.assertGreater(foreground, 512 * 512 // 100, f"{shape} {view}: {foreground} foreground pixels")
                glb = (job / "model.glb").read_bytes()
                self.assertEqual(glb[:4], b"glTF")
                json_chunk_size = struct.unpack_from("<I", glb, 12)[0]
                self.assertEqual(glb[16:20], b"JSON")
                gltf = json.loads(glb[20:20 + json_chunk_size])
                self.assertEqual(gltf.get("extensionsRequired", []), [])
                self.assertTrue(all("uri" not in buffer for buffer in gltf["buffers"]))
                exported = trimesh.load(job / "model.glb", force="scene", process=False)
                self.assertEqual(len(exported.graph.nodes_geometry), 3)
                volumes = []
                elevations = []
                for node in exported.graph.nodes_geometry:
                    transform, name = exported.graph[node]
                    mesh = exported.geometry[name].copy()
                    mesh.apply_transform(transform)
                    # glTF normal seams duplicate positions; exact-position weld on a test copy.
                    unique, inverse = np.unique(np.asarray(mesh.vertices), axis=0, return_inverse=True)
                    mesh = trimesh.Trimesh(vertices=unique, faces=inverse[mesh.faces], process=False)
                    self.assertTrue(mesh.is_watertight)
                    self.assertTrue(mesh.is_winding_consistent)
                    self.assertGreater(mesh.volume, 0)
                    vertices = np.asarray(mesh.vertices)
                    world = np.column_stack((vertices[:, 0], -vertices[:, 2], vertices[:, 1]))
                    np.testing.assert_allclose(world.min(axis=0)[:2], case["origin_xy_m"], atol=1e-6, rtol=0)
                    np.testing.assert_allclose(world.max(axis=0)[:2],
                                               [case["origin_xy_m"][0] + case["width_m"], case["origin_xy_m"][1] + case["depth_m"]],
                                               atol=1e-6, rtol=0)
                    elevations.append((world[:, 2].min(), world[:, 2].max()))
                    volumes.append(mesh.volume)
                np.testing.assert_allclose(sorted(elevations), [(0, 3.2), (3.2, 6.4), (6.4, 9.6)], atol=1e-6, rtol=0)
                self.assertAlmostEqual(sum(volumes), case["footprint_area_m2"] * case["height_m"], places=4)
                reopen = subprocess.run([os.environ["ARCHY_BLENDER_BIN"], "--background", "--factory-startup",
                                         "--disable-autoexec", "--offline-mode", "--python-exit-code", "20",
                                         "--threads", "1", str(job / "scene.blend"), "--python",
                                         str(ROOT / "workers/blender/smoke.py")],
                                        cwd=job, capture_output=True, text=True, timeout=30)
                self.assertEqual(reopen.returncode, 0, reopen.stdout + reopen.stderr)
                self.assertIn("ARCHY_REOPEN_OK 3", reopen.stdout)
                self.assertEqual(response["geometry"]["storeys"], 3)
                self.assertAlmostEqual(response["geometry"]["footprint_area_m2"], case["footprint_area_m2"], places=5)
                self.assertAlmostEqual(response["geometry"]["volume_m3"], case["footprint_area_m2"] * case["height_m"], places=4)


if __name__ == "__main__":
    unittest.main()
