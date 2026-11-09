import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "workers" / "ifc"))
from archy_ifc_worker import protocol


def request():
    variant = json.loads((ROOT / "tests/fixtures/generation/courtyard.seed42.json").read_text())["variants"][0]
    site = json.loads((ROOT / "tests/fixtures/briefs/courtyard.normalized.json").read_text())["site"]
    return {"schema_version": "ifc-worker-v1", "operation": "export_schematic_ifc",
            "request_id": "req-1", "project_id": variant["project_id"], "variant_id": variant["id"],
            "run_id": "run-1", "variant": variant, "site": site,
            "source_digest": "a" * 64, "source_artifacts": [{"path": "source/variant.json", "sha256": "b" * 64}],
            "output_dir": "runs/run-1/ifc"}


class ProtocolTests(unittest.TestCase):
    def test_request_read_rejects_sibling_traversal_and_symlinks(self):
        from archy_ifc_worker.worker import input_bytes
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            project = base / "project"
            project.mkdir()
            secret = base / "secret.json"
            secret.write_bytes(b"outside-secret")
            legitimate = project / "request.json"
            legitimate.write_bytes(b"inside-request")
            self.assertEqual(input_bytes(project, legitimate), b"inside-request")
            (project / "link-file").symlink_to(secret)
            (project / "link-dir").symlink_to(base, target_is_directory=True)
            (base / "project-link").symlink_to(project, target_is_directory=True)
            for path_root, path in (
                (project, project / ".." / "secret.json"),
                (project, project / "link-file"),
                (project, project / "link-dir" / "secret.json"),
                (base / "project-link", base / "project-link" / "request.json"),
            ):
                with self.subTest(path=str(path)):
                    with self.assertRaises(protocol.ProtocolError) as caught:
                        input_bytes(path_root, path)
                    self.assertEqual(caught.exception.code, "PERMISSION_DENIED")
            self.assertEqual(secret.read_bytes(), b"outside-secret")

    @unittest.skipIf(importlib.util.find_spec("ifcopenshell"), "backend is installed")
    def test_missing_backend_response_echoes_association(self):
        from archy_ifc_worker.worker import main
        item = request()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "runs/run-1").mkdir(parents=True)
            source = root / "request.json"
            source.write_text(json.dumps(item))
            self.assertEqual(main(["--project-root", str(root), "--request", str(source)]), 20)
            response = json.loads((root / item["output_dir"] / "response.json").read_text())
            self.assertEqual(response["error"]["code"], "BACKEND_UNAVAILABLE")
            self.assertEqual(response["variant_id"], item["variant_id"])
            self.assertEqual(response["source_digest"], item["source_digest"])

    def test_canonical_task02_fixture(self):
        item = request()
        self.assertEqual(protocol.parse_request(json.dumps(item).encode()), item)

    def test_duplicate_and_unknown_fields(self):
        raw = json.dumps(request()).replace('"run_id": "run-1"', '"run_id": "run-1", "run_id": "run-2"')
        with self.assertRaisesRegex(protocol.ProtocolError, "duplicate"):
            protocol.parse_request(raw.encode())
        item = request(); item["approval"] = True
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_request(item)
        item = request(); item["variant"]["required_checks_passed"] = True
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_request(item)

    def test_authority_and_identity_tampering(self):
        for mutate in (
            lambda x: x.update(variant_id="variant-" + "0" * 64),
            lambda x: x["variant"].update(id="variant-" + "0" * 64),
            lambda x: x["variant"]["parameters"].update(width_m=15.0),
            lambda x: x["variant"].update(footprint_xy_m=[[0, 0]] * 7),
            lambda x: x["variant"].update(storeys=0),
            lambda x: x["variant"].update(storeys=101),
        ):
            with self.subTest(mutate=mutate):
                item = request(); mutate(item)
                with self.assertRaises(protocol.ProtocolError):
                    protocol.validate_request(item)

    def test_nonfinite_negative_zero_and_capacity(self):
        for raw in (b'{"x":NaN}', b'{"x":Infinity}', b'\xff', b' ' * (1024 * 1024 + 1),
                    b'{"x":' + b'9' * 5000 + b'}'):
            with self.subTest(raw=raw[:16]), self.assertRaises(protocol.ProtocolError):
                protocol.parse_request(raw)
        item = request(); item["variant"]["parameters"]["origin_xy_m"][0] = -0.0
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_request(item)
        item = request(); item["variant"]["floor_to_floor_m"] = 10001.0
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_request(item)

    def test_destinations_and_source_references(self):
        for path in ("/tmp/export", "../outside", "runs//ifc", "runs/./ifc", "runs/../ifc", "C:\\outside", "runs/ifc\x00"):
            item = request(); item["output_dir"] = path
            with self.subTest(path=path), self.assertRaises(protocol.ProtocolError):
                protocol.validate_request(item)
        item = request(); item["source_artifacts"][0]["path"] = "../../secret"
        with self.assertRaises(protocol.ProtocolError):
            protocol.validate_request(item)


if __name__ == "__main__":
    unittest.main()
