import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "packaging/arch"))
SPEC = importlib.util.spec_from_file_location("archy_notices", ROOT / "packaging/arch/notices.py")
NOTICES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NOTICES)


def sha(data):
    return hashlib.sha256(data).hexdigest()


class NoticeAdmissionTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.base = Path(tmp.name)
        self.source = self.base / "source"
        self.vendor = self.source / "vendor"
        self.output = self.base / "output"
        self.source.mkdir()
        self.vendor.mkdir()
        self.output.mkdir()
        (self.source / "Cargo.lock").write_text('''version = 4
[[package]]
name = "alpha"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "''' + "a" * 64 + '''"
[[package]]
name = "beta"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "''' + "b" * 64 + '''"
''')
        self.alpha = self.vendor / "alpha-1.0.0"
        self.beta = self.vendor / "beta-2.0.0"
        self.alpha.mkdir()
        self.beta.mkdir()
        self.alpha_text = b"Alpha MIT text\n"
        (self.alpha / "LICENSE-MIT").write_bytes(self.alpha_text)
        (self.alpha / ".cargo-checksum.json").write_text(json.dumps({
            "package": "a" * 64, "files": {"LICENSE-MIT": sha(self.alpha_text)}}))
        (self.beta / ".cargo-checksum.json").write_text(json.dumps({
            "package": "b" * 64, "files": {}}))
        (self.beta / ".cargo_vcs_info.json").write_text(json.dumps({
            "git": {"sha1": "c" * 40}, "path_in_vcs": "beta"}))
        self.rust_text = b"Rust standard library copyright\n"
        self.rust_file = self.source / "packaging/arch/notices-sources/rust-1.99.1/COPYRIGHT-library.html"
        self.rust_file.parent.mkdir(parents=True)
        self.rust_file.write_bytes(self.rust_text)
        self.inputs = {
            "schema_version": 1,
            "rust_std": {"version": "1.99.1", "files": [{
                "path": self.rust_file.relative_to(self.source).as_posix(),
                "sha256": sha(self.rust_text),
            }]},
            "external": [],
        }
        self.inputs_path = self.source / "packaging/arch/notice-inputs.json"
        self.write_inputs()
        self.metadata = {
            "packages": [
                {"id": "cli", "name": "archy-cli", "version": "0.1.0", "source": None},
                {"id": "alpha", "name": "alpha", "version": "1.0.0", "license": "MIT",
                 "source": "registry+https://github.com/rust-lang/crates.io-index"},
                {"id": "beta", "name": "beta", "version": "2.0.0", "license": "Apache-2.0",
                 "source": "registry+https://github.com/rust-lang/crates.io-index"},
            ],
            "resolve": {"nodes": [
                {"id": "cli", "deps": [{"pkg": "alpha", "dep_kinds": [{"kind": None}]}]},
                {"id": "alpha", "deps": [{"pkg": "beta", "dep_kinds": [{"kind": None}]}]},
                {"id": "beta", "deps": []},
            ]},
        }

    def write_inputs(self):
        self.inputs_path.write_text(json.dumps(self.inputs))

    def admit(self):
        return NOTICES.generate(self.source, self.vendor, self.metadata, "1.99.1", self.output)

    def add_verified_beta(self):
        data = b"Verified beta Apache text\n"
        path = self.source / "packaging/arch/notices-sources/beta-2.0.0/LICENSE-APACHE"
        path.parent.mkdir(parents=True)
        path.write_bytes(data)
        blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        self.inputs["external"] = [{"name": "beta", "version": "2.0.0",
                                    "path": path.relative_to(self.source).as_posix(),
                                    "repository": "example/beta", "commit": "c" * 40,
                                    "path_in_repository": "LICENSE-APACHE", "git_blob": blob,
                                    "source_url": "https://github.com/example/beta/blob/" + "c" * 40 + "/LICENSE-APACHE",
                                    "sha256": sha(data)}]
        self.write_inputs()

    def test_missing_primary_text_fails_without_output(self):
        with self.assertRaisesRegex(ValueError, "missing notice text.*beta"):
            self.admit()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_verified_sources_emit_full_text_and_hash_inventory(self):
        self.add_verified_beta()
        inventory_path, text_path = self.admit()
        inventory = json.loads(inventory_path.read_text())
        self.assertEqual([(c["name"], c["version"]) for c in inventory["crates"]],
                         [("alpha", "1.0.0"), ("beta", "2.0.0")])
        self.assertEqual(inventory["crates"][0]["source_sha256"], "a" * 64)
        self.assertEqual(inventory["crates"][0]["texts"][0]["sha256"], sha(self.alpha_text))
        self.assertEqual(inventory["rust_std"]["version"], "1.99.1")
        text = text_path.read_text()
        self.assertIn("Alpha MIT text", text)
        self.assertIn("Verified beta Apache text", text)
        self.assertIn("Rust standard library copyright", text)
        self.assertEqual(inventory["bundle_sha256"], sha(text_path.read_bytes()))

    def test_rejects_changed_vendor_notice_and_package_hash(self):
        self.add_verified_beta()
        (self.alpha / "LICENSE-MIT").write_text("tampered\n")
        with self.assertRaisesRegex(ValueError, "notice checksum mismatch"):
            self.admit()
        self.assertEqual(list(self.output.iterdir()), [])
        (self.alpha / "LICENSE-MIT").write_bytes(self.alpha_text)
        checksum = self.alpha / ".cargo-checksum.json"
        data = json.loads(checksum.read_text())
        data["package"] = "c" * 64
        checksum.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "source checksum mismatch"):
            self.admit()

    def test_rejects_changed_external_and_rust_std_hashes(self):
        self.add_verified_beta()
        entry = self.inputs["external"][0]
        entry["sha256"] = "0" * 64
        self.write_inputs()
        with self.assertRaisesRegex(ValueError, "external notice checksum mismatch"):
            self.admit()
        entry["sha256"] = sha(b"Verified beta Apache text\n")
        self.inputs["rust_std"]["files"][0]["sha256"] = "0" * 64
        self.write_inputs()
        with self.assertRaisesRegex(ValueError, "Rust std notice checksum mismatch"):
            self.admit()

    def test_external_primary_provenance_must_match_local_bytes_and_vendor_commit(self):
        self.add_verified_beta()
        entry = self.inputs["external"][0]
        entry["git_blob"] = "0" * 40
        self.write_inputs()
        with self.assertRaisesRegex(ValueError, "external Git blob mismatch"):
            self.admit()
        entry["git_blob"] = hashlib.sha1(b"blob " + str(len(b"Verified beta Apache text\n")).encode()
                                         + b"\0Verified beta Apache text\n").hexdigest()
        entry["commit"] = "d" * 40
        entry["source_url"] = "https://github.com/example/beta/blob/" + "d" * 40 + "/LICENSE-APACHE"
        self.write_inputs()
        with self.assertRaisesRegex(ValueError, "external source commit mismatch"):
            self.admit()
        entry["commit"] = "c" * 40
        self.write_inputs()
        with self.assertRaisesRegex(ValueError, "external source URL mismatch"):
            self.admit()

    def test_rejects_escape_duplicate_and_unneeded_external_evidence(self):
        self.add_verified_beta()
        entry = dict(self.inputs["external"][0])
        self.inputs["external"].append(entry)
        self.write_inputs()
        with self.assertRaisesRegex(ValueError, "duplicate external notice"):
            self.admit()
        self.inputs["external"] = [entry]
        entry["path"] = "../outside"
        self.write_inputs()
        with self.assertRaises(ValueError):
            self.admit()
        entry["path"] = self.rust_file.relative_to(self.source).as_posix()
        entry["name"] = "not-in-graph"
        self.write_inputs()
        with self.assertRaisesRegex(ValueError, "external notice outside graph"):
            self.admit()

    def test_output_collision_rolls_back_only_new_notice_file(self):
        self.add_verified_beta()
        sentinel = self.output / "THIRD-PARTY-NOTICES.txt"
        sentinel.write_text("preexisting")
        with self.assertRaises(FileExistsError):
            self.admit()
        self.assertEqual(sentinel.read_text(), "preexisting")
        self.assertFalse((self.output / "third-party-notices.json").exists())

    def test_notice_stage_preserves_unrelated_paths_and_inventory(self):
        self.add_verified_beta()
        notice_inventory, notice_text = self.admit()
        package_spec = importlib.util.spec_from_file_location("archy_package_stage", ROOT / "packaging/arch/package.py")
        package = importlib.util.module_from_spec(package_spec)
        package_spec.loader.exec_module(package)
        (self.source / "LICENSE").write_text("Archy MIT\n")
        binary = self.base / "archy"
        binary.write_bytes(b"binary fixture")
        manifest = self.base / "files.txt"
        manifest.write_text("@binary\tusr/bin/archy\t0755\nLICENSE\tusr/share/licenses/archy/LICENSE\t0644\n"
                            "@notices\tusr/share/licenses/archy/THIRD-PARTY-NOTICES.txt\t0644\n"
                            "@notice-inventory\tusr/share/archy/third-party-notices.json\t0644\n")
        dest = self.base / "dest"
        dest.mkdir()
        sentinel = dest / "home/person/project/run.json"
        sentinel.parent.mkdir(parents=True)
        sentinel.write_text("keep")
        stage_inventory = self.base / "stage-inventory.json"
        package.stage(SimpleNamespace(source=str(self.source), binary=str(binary), dest=str(dest),
                                      manifest=str(manifest), inventory=str(stage_inventory),
                                      notices_file=str(notice_text), notice_inventory_file=str(notice_inventory)))
        staged = json.loads(stage_inventory.read_text())
        self.assertEqual(len(staged), 4)
        for entry in staged:
            target = dest / entry["path"]
            self.assertEqual(sha(target.read_bytes()), entry["sha256"])
            target.unlink()  # package-owned removal simulation only
        self.assertEqual(sentinel.read_text(), "keep")
        self.assertEqual(sha(notice_text.read_bytes()), json.loads(notice_inventory.read_text())["bundle_sha256"])

    def test_stage_rejects_inconsistent_notice_pair_before_mutation(self):
        self.add_verified_beta()
        notice_inventory, notice_text = self.admit()
        notice_text.write_bytes(notice_text.read_bytes() + b"tampered")
        package_spec = importlib.util.spec_from_file_location("archy_package_stage", ROOT / "packaging/arch/package.py")
        package = importlib.util.module_from_spec(package_spec)
        package_spec.loader.exec_module(package)
        (self.source / "LICENSE").write_text("Archy MIT\n")
        binary = self.base / "archy"
        binary.write_bytes(b"binary fixture")
        manifest = self.base / "files.txt"
        manifest.write_text("@binary\tusr/bin/archy\t0755\nLICENSE\tusr/share/licenses/archy/LICENSE\t0644\n"
                            "@notices\tusr/share/licenses/archy/THIRD-PARTY-NOTICES.txt\t0644\n"
                            "@notice-inventory\tusr/share/archy/third-party-notices.json\t0644\n")
        dest = self.base / "dest"
        dest.mkdir()
        sentinel = dest / "home/person/project/run.json"
        sentinel.parent.mkdir(parents=True)
        sentinel.write_text("keep")
        with self.assertRaisesRegex(ValueError, "notice bundle/inventory checksum mismatch"):
            package.stage(SimpleNamespace(source=str(self.source), binary=str(binary), dest=str(dest),
                                          manifest=str(manifest), inventory=None, notices_file=str(notice_text),
                                          notice_inventory_file=str(notice_inventory)))
        self.assertEqual(sentinel.read_text(), "keep")
        self.assertFalse((dest / "usr").exists())


if __name__ == "__main__":
    unittest.main()
