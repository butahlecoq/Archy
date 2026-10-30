"""Real offline graph and archive admission for the proposed authority binaries."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "packaging/arch"))
SPEC = importlib.util.spec_from_file_location("archy_authority_notices", ROOT / "packaging/arch/authority_notices.py")
ADMISSION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADMISSION)
CACHE = Path.home() / ".cargo/registry/cache/index.crates.io-1949cf8c6b5b557f"


class AuthorityAdmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        env = dict(os.environ, CARGO_NET_OFFLINE="true", RUSTUP_AUTO_INSTALL="0", CARGO_BUILD_JOBS="2")
        cls.metadata = json.loads(subprocess.check_output(
            ["cargo", "+1.99.1", "metadata", "--locked", "--offline", "--format-version", "1",
             "--filter-platform", "x86_64-unknown-linux-gnu"], cwd=ROOT, env=env))

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.output = Path(tmp.name)

    def admit(self, metadata=None):
        return ADMISSION.generate(ROOT, CACHE, metadata or self.metadata, "1.99.1", self.output)

    def test_real_graph_inventory_reruns_and_package_boundary(self):
        inventory_path, bundle_path = self.admit()
        inventory = json.loads(inventory_path.read_text())
        self.assertEqual(inventory["crate_count"], 159)
        self.assertEqual(sum(c["source"] == ADMISSION.REGISTRY for c in inventory["crates"]), 158)
        self.assertEqual(sum(c["source"] == "patched-local" for c in inventory["crates"]), 1)
        tracing = next(c for c in inventory["crates"] if c["name"] == "tracing-core")
        self.assertIn("src/spin/LICENSE", {text["path"] for text in tracing["texts"]})
        self.assertEqual(inventory["bundle_sha256"], hashlib.sha256(bundle_path.read_bytes()).hexdigest())
        self.assertIn(b"zbus 5.19.0 patched-local: LICENSE", bundle_path.read_bytes())
        original = (inventory_path.read_bytes(), bundle_path.read_bytes())
        self.admit()
        self.assertEqual((inventory_path.read_bytes(), bundle_path.read_bytes()), original)
        manifest = (ROOT / "packaging/arch/files.txt").read_text()
        self.assertNotIn("authority", manifest)
        self.assertNotIn("systemd", manifest)
        self.assertNotIn("dbus", manifest.lower())

    def test_added_registry_dependency_refused(self):
        metadata = copy.deepcopy(self.metadata)
        cli = {p["id"] for p in ADMISSION.normal_registry_graph(metadata)}
        authority = {p["id"] for p in ADMISSION.normal_registry_graph(metadata, ("archy-authority",))}
        extra = next(p for p in metadata["packages"] if p["source"] == ADMISSION.REGISTRY
                     and p["id"] not in cli | authority)
        root = next(p for p in metadata["packages"] if p["name"] == "archy-authority")
        node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == root["id"])
        node["deps"].append({"pkg": extra["id"], "dep_kinds": [{"kind": "normal"}]})
        with self.assertRaisesRegex(ValueError, "authority dependency pin coverage mismatch"):
            self.admit(metadata)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_added_local_dependency_refused(self):
        metadata = copy.deepcopy(self.metadata)
        root = next(p for p in metadata["packages"] if p["name"] == "archy-authority")
        node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == root["id"])
        metadata["packages"].append({"id": "unreviewed-local", "name": "unreviewed-local",
                                     "version": "0.1.0", "source": None})
        metadata["resolve"]["nodes"].append({"id": "unreviewed-local", "deps": []})
        node["deps"].append({"pkg": "unreviewed-local", "dep_kinds": [{"kind": "normal"}]})
        with self.assertRaisesRegex(ValueError, "unreviewed local authority dependency"):
            self.admit(metadata)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_missing_license_and_forged_license_refused(self):
        original = ADMISSION.archive_files
        def missing(*args):
            files = original(*args)
            if args[1] == "async-broadcast":
                del files["LICENSE-MIT"]
            return files
        with patch.object(ADMISSION, "archive_files", side_effect=missing):
            with self.assertRaisesRegex(ValueError, "authority license/source pin mismatch"):
                self.admit()
        self.assertEqual(list(self.output.iterdir()), [])
        def forged(*args):
            files = original(*args)
            if args[1] == "async-broadcast":
                files["LICENSE-MIT"] = b"forged license\n"
            return files
        with patch.object(ADMISSION, "archive_files", side_effect=forged):
            with self.assertRaisesRegex(ValueError, "authority license/source pin mismatch"):
                self.admit()

    def test_extra_unreviewed_license_refused(self):
        original = ADMISSION.archive_files
        cli_name = ADMISSION.normal_registry_graph(self.metadata)[0]["name"]
        def extra(*args):
            files = original(*args)
            if args[1] == cli_name:
                files["LICENSE-FORGED"] = b"unreviewed license\n"
            return files
        with patch.object(ADMISSION, "archive_files", side_effect=extra):
            with self.assertRaisesRegex(ValueError, "authority reviewed notice output mismatch"):
                self.admit()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_stale_cache_and_patched_source_refused(self):
        archive = self.output / "async-broadcast-0.7.2.crate"
        archive.write_bytes(b"stale cache bytes")
        pin_data = json.loads((ROOT / "packaging/arch/authority-notice-inputs.json").read_text())
        expected = next(p["source_sha256"] for p in pin_data["additional_registry_crates"]
                        if p["name"] == "async-broadcast")
        with self.assertRaisesRegex(ValueError, "crate archive checksum mismatch"):
            ADMISSION.archive_files(self.output, "async-broadcast", "0.7.2", expected)
        archive.unlink()
        pin = pin_data["zbus"]
        altered = copy.deepcopy(pin)
        altered["changed_files"]["src/utils.rs"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "patched zbus source/provenance checksum mismatch"):
            ADMISSION.patched_zbus(ROOT, CACHE, self.metadata, altered)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_stale_output_refused_without_replacing_either_file(self):
        inventory_path, bundle_path = self.admit()
        inventory_path.write_bytes(b"stale")
        original_bundle = bundle_path.read_bytes()
        with self.assertRaisesRegex(ValueError, "stale authority notice output"):
            self.admit()
        self.assertEqual(inventory_path.read_bytes(), b"stale")
        self.assertEqual(bundle_path.read_bytes(), original_bundle)


if __name__ == "__main__":
    unittest.main()
