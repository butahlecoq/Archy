import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "packaging/arch/package.py"
SPEC = importlib.util.spec_from_file_location("archy_package_tool", TOOL)
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


def run_tool(*args):
    return subprocess.run([sys.executable, str(TOOL), *map(str, args)], text=True, capture_output=True)


class StageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.source = self.base / "source"
        self.dest = self.base / "dest"
        self.source.mkdir()
        self.dest.mkdir()
        (self.source / "LICENSE").write_text("core license\n")
        (self.source / "data").mkdir()
        (self.source / "data/schema.json").write_text("{}\n")
        self.binary = self.base / "archy"
        self.binary.write_bytes(b"ELF fixture")
        self.notices = self.base / "THIRD-PARTY-NOTICES.txt"
        self.notices.write_bytes(b"notice fixture\n")
        self.notice_inventory = self.base / "third-party-notices.json"
        self.notice_inventory.write_text(json.dumps({"bundle_sha256": hashlib.sha256(self.notices.read_bytes()).hexdigest()}))
        self.manifest = self.base / "files.txt"
        self.manifest.write_text("@binary\tusr/bin/archy\t0755\nLICENSE\tusr/share/licenses/archy/LICENSE\t0644\ndata/schema.json\tusr/share/archy/schema.json\t0644\n@notices\tusr/share/licenses/archy/THIRD-PARTY-NOTICES.txt\t0644\n@notice-inventory\tusr/share/archy/third-party-notices.json\t0644\n")
        self.inventory = self.base / "inventory.json"

    def stage(self):
        return run_tool("stage", "--source", self.source, "--binary", self.binary,
                        "--manifest", self.manifest, "--dest", self.dest, "--inventory", self.inventory,
                        "--notices-file", self.notices, "--notice-inventory-file", self.notice_inventory)

    def stage_direct(self):
        return PACKAGE.stage(SimpleNamespace(source=str(self.source), binary=str(self.binary),
                                             manifest=str(self.manifest), dest=str(self.dest),
                                             inventory=str(self.inventory), notices_file=str(self.notices),
                                             notice_inventory_file=str(self.notice_inventory)))

    def test_stage_inventory_and_simulated_removal_preserves_sentinels(self):
        sentinels = ["home/person/project/run.json", "home/person/approval.json",
                     "home/person/.config/archy/settings.json", "home/person/backups/shell.json"]
        for name in sentinels:
            path = self.dest / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
        result = self.stage()
        self.assertEqual(result.returncode, 0, result.stderr)
        entries = json.loads(self.inventory.read_text())
        self.assertEqual([e["path"] for e in entries], ["usr/bin/archy", "usr/share/archy/schema.json", "usr/share/archy/third-party-notices.json", "usr/share/licenses/archy/LICENSE", "usr/share/licenses/archy/THIRD-PARTY-NOTICES.txt"])
        self.assertEqual([e["mode"] for e in entries], ["0755", "0644", "0644", "0644", "0644"])
        for entry in entries:
            path = self.dest / entry["path"]
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), entry["sha256"])
            self.assertEqual(path.stat().st_mode & 0o777, int(entry["mode"], 8))
            path.unlink()  # package-owned path removal simulation, not pacman behavior
        for name in sentinels:
            self.assertEqual((self.dest / name).read_text(), name)

    def test_rejects_collision_without_mutation(self):
        target = self.dest / "usr/bin/archy"
        target.parent.mkdir(parents=True)
        target.write_text("previous")
        result = self.stage()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(target.read_text(), "previous")
        self.assertFalse((self.dest / "usr/share/archy/schema.json").exists())

    def test_stage_rejects_manifest_without_notice_pair_before_writing(self):
        lines = self.manifest.read_text().splitlines(keepends=True)
        self.manifest.write_text("".join(line for line in lines if not line.startswith(("@notices\t", "@notice-inventory\t"))))
        result = self.stage()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("required", result.stderr)
        self.assertFalse((self.dest / "usr").exists())
        self.assertFalse(self.inventory.exists())

    def test_stage_uses_notice_bytes_validated_before_source_changes(self):
        original = self.notices.read_bytes()
        original_inventory = self.notice_inventory.read_bytes()
        real_read = PACKAGE.read_regular_nofollow
        changed = False

        def mutate_after_preflight(path, *args, **kwargs):
            nonlocal changed
            data = real_read(path, *args, **kwargs)
            if path == self.notice_inventory and not changed:
                changed = True
                self.notices.write_bytes(b"changed after preflight\n")
            return data

        with patch.object(PACKAGE, "read_regular_nofollow", side_effect=mutate_after_preflight):
            self.stage_direct()
        self.assertTrue(changed)
        self.assertEqual((self.dest / "usr/share/licenses/archy/THIRD-PARTY-NOTICES.txt").read_bytes(), original)
        self.assertEqual((self.dest / "usr/share/archy/third-party-notices.json").read_bytes(), original_inventory)
        self.assertEqual(json.loads(original_inventory)["bundle_sha256"], hashlib.sha256(original).hexdigest())
        entries = json.loads(self.inventory.read_text())
        for entry in entries:
            self.assertEqual(entry["sha256"], hashlib.sha256((self.dest / entry["path"]).read_bytes()).hexdigest())

    def test_stage_rejects_oversized_notice_inputs_before_writing(self):
        for path, limit in ((self.notices, PACKAGE.NOTICE_LIMIT),
                            (self.notice_inventory, PACKAGE.NOTICE_INVENTORY_LIMIT)):
            with self.subTest(path=path.name):
                with path.open("wb") as stream:
                    stream.truncate(limit + 1)
                result = self.stage()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("byte limit", result.stderr)
                self.assertFalse((self.dest / "usr").exists())
                self.assertFalse(self.inventory.exists())
                if path == self.notices:
                    self.notices.write_bytes(b"notice fixture\n")

    def test_rejects_escaping_and_special_entries(self):
        for source, destination in [("../outside", "usr/share/archy/x"),
                                    ("LICENSE", "../outside"),
                                    ("LICENSE", "home/person/file")]:
            with self.subTest(source=source, destination=destination):
                self.manifest.write_text(f"{source}\t{destination}\t0644\n")
                self.assertNotEqual(self.stage().returncode, 0)
        (self.source / "link").symlink_to(self.source / "LICENSE")
        self.manifest.write_text("link\tusr/share/archy/link\t0644\n")
        self.assertNotEqual(self.stage().returncode, 0)
        (self.dest / "usr/share").mkdir(parents=True)
        (self.dest / "usr/share/archy").symlink_to(self.base)
        self.manifest.write_text("LICENSE\tusr/share/archy/LICENSE\t0644\n")
        self.assertNotEqual(self.stage().returncode, 0)
        self.assertFalse(self.inventory.exists())

    def test_failed_stage_rolls_back_only_new_files(self):
        unrelated = self.dest / "usr/share/other-owned/data"
        unrelated.parent.mkdir(parents=True)
        unrelated.write_text("keep")
        self.inventory.write_text("prior inventory")
        result = self.stage()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(unrelated.read_text(), "keep")
        self.assertEqual(self.inventory.read_text(), "prior inventory")
        self.assertFalse((self.dest / "usr/bin/archy").exists())
        self.assertFalse((self.dest / "usr/share/archy/schema.json").exists())

    def test_parent_substitution_does_not_escape_stage_root(self):
        parent = self.dest / "usr/bin"
        parent.mkdir(parents=True)
        outside = self.base / "outside"
        outside.mkdir()
        (outside / "keep.txt").write_text("outside sentinel")
        real_open = os.open
        swapped = False

        def swap_before_create(path, flags, *args, **kwargs):
            nonlocal swapped
            if not swapped and flags & os.O_CREAT and str(path).endswith("archy"):
                swapped = True
                parent.rename(self.dest / "usr/bin-detached")
                parent.symlink_to(outside)
            return real_open(path, flags, *args, **kwargs)

        with patch.object(PACKAGE.os, "open", side_effect=swap_before_create):
            with self.assertRaises((OSError, ValueError)):
                self.stage_direct()
        self.assertTrue(swapped)
        self.assertEqual((outside / "keep.txt").read_text(), "outside sentinel")
        self.assertFalse((outside / "archy").exists())
        self.assertFalse((self.dest / "usr/bin-detached/archy").exists())
        self.assertFalse(self.inventory.exists())

    def test_midcopy_failure_rollback_uses_original_parent_descriptor(self):
        parent = self.dest / "usr/bin"
        parent.mkdir(parents=True)
        outside = self.base / "outside"
        outside.mkdir()
        (outside / "archy").write_text("outside sentinel")
        real_open = os.open
        injected = False

        def fail_second_create(path, flags, *args, **kwargs):
            nonlocal injected
            if not injected and flags & os.O_CREAT and str(path).endswith("LICENSE"):
                injected = True
                parent.rename(self.dest / "usr/bin-detached")
                parent.symlink_to(outside)
                raise OSError("injected mid-copy failure")
            return real_open(path, flags, *args, **kwargs)

        with patch.object(PACKAGE.os, "open", side_effect=fail_second_create):
            with self.assertRaises(OSError):
                self.stage_direct()
        self.assertTrue(injected)
        self.assertEqual((outside / "archy").read_text(), "outside sentinel")
        self.assertFalse((self.dest / "usr/bin-detached/archy").exists())
        self.assertFalse(self.inventory.exists())

    def test_source_parent_substitution_is_not_followed(self):
        outside = self.base / "outside"
        outside.mkdir()
        (outside / "schema.json").write_text("untrusted")
        origin = self.source / "data/schema.json"
        real_check = PACKAGE.regular_nofollow
        swapped = False

        def swap_after_check(path):
            nonlocal swapped
            real_check(path)
            if not swapped and path == origin:
                swapped = True
                origin.parent.rename(self.source / "data-detached")
                origin.parent.symlink_to(outside)

        with patch.object(PACKAGE, "regular_nofollow", side_effect=swap_after_check):
            with self.assertRaises((OSError, ValueError)):
                self.stage_direct()
        self.assertTrue(swapped)
        self.assertEqual((outside / "schema.json").read_text(), "untrusted")
        self.assertFalse(self.inventory.exists())


class RecipeTests(unittest.TestCase):
    def test_recipe_rejects_archive_manifest_without_notice_pair(self):
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            archive = base / "source.tar.gz"
            with tarfile.open(archive, "w:gz") as tf:
                for name in ("Cargo.lock", "packaging/arch/package.py", "packaging/arch/notices.py",
                             "packaging/arch/notice-inputs.json", "packaging/arch/files.txt"):
                    data = b"@binary\tusr/bin/archy\t0755\n" if name.endswith("files.txt") else b"fixture"
                    info = tarfile.TarInfo("archy-source/" + name)
                    info.size = len(data)
                    tf.addfile(info, io.BytesIO(data))
            output = base / "PKGBUILD"
            result = run_tool("recipe", "--archive", archive, "--source-id", "v0.1.0-test",
                              "--sha256", hashlib.sha256(archive.read_bytes()).hexdigest(), "--output", output)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(output.exists())

    def test_recipe_does_not_follow_substituted_archive_parent(self):
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            source_dir = base / "source"
            source_dir.mkdir()
            archive = source_dir / "source.tar.gz"
            with tarfile.open(archive, "w:gz") as tf:
                for name in ("archy-source/Cargo.lock", "archy-source/packaging/arch/files.txt",
                             "archy-source/packaging/arch/package.py", "archy-source/packaging/arch/notices.py",
                             "archy-source/packaging/arch/notice-inputs.json"):
                    info = tarfile.TarInfo(name)
                    info.size = 4
                    tf.addfile(info, io.BytesIO(b"test"))
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            outside = base / "outside"
            outside.mkdir()
            (outside / archive.name).write_bytes(archive.read_bytes())
            output = base / "PKGBUILD"
            real_check = PACKAGE.regular_nofollow
            swapped = False

            def swap_after_check(path):
                nonlocal swapped
                real_check(path)
                if not swapped and path == archive:
                    swapped = True
                    source_dir.rename(base / "source-detached")
                    source_dir.symlink_to(outside)

            with patch.object(PACKAGE, "regular_nofollow", side_effect=swap_after_check):
                with self.assertRaises((OSError, ValueError)):
                    PACKAGE.recipe(SimpleNamespace(archive=str(archive), source_id="v0.1.0-test",
                                                   sha256=digest, output=str(output)))
            self.assertTrue(swapped)
            self.assertFalse(output.exists())

    def test_recipe_rejects_duplicate_and_nonfile_archive_members(self):
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            archive = base / "source.tar.gz"
            required = ["archy-source/Cargo.lock", "archy-source/packaging/arch/files.txt",
                        "archy-source/packaging/arch/package.py", "archy-source/packaging/arch/notices.py",
                        "archy-source/packaging/arch/notice-inputs.json"]
            cases = [
                [(x, "file") for x in required] + [("archy-source/Cargo.lock", "file")],
                [(required[0], "directory")] + [(x, "file") for x in required[1:]],
                [(x, "file") for x in required] + [("archy-source/packaging", "file")],
                [(x, "file") for x in required] + [("archy-source/packaging/arch", "directory"),
                                                    ("archy-source/packaging/arch/", "directory")],
            ]
            for index, members in enumerate(cases):
                with self.subTest(index=index):
                    output = base / f"PKGBUILD-{index}"
                    with tarfile.open(archive, "w:gz") as tf:
                        for name, kind in members:
                            info = tarfile.TarInfo(name)
                            if kind == "directory":
                                info.type = tarfile.DIRTYPE
                                tf.addfile(info)
                            else:
                                data = b"fixture"
                                info.size = len(data)
                                tf.addfile(info, io.BytesIO(data))
                    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                    result = run_tool("recipe", "--archive", archive, "--source-id", "v0.1.0-test",
                                      "--sha256", digest, "--output", output)
                    self.assertNotEqual(result.returncode, 0, result.stdout)
                    self.assertFalse(output.exists())

    def test_recipe_requires_verified_archive_and_identity(self):
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            archive = base / "archy-source.tar.gz"
            src = base / "archy-source"
            src.mkdir()
            (src / "Cargo.lock").write_text("lock")
            package = src / "packaging/arch"
            package.mkdir(parents=True)
            (package / "files.txt").write_text("@binary\tusr/bin/archy\t0755\n@notices\tusr/share/licenses/archy/THIRD-PARTY-NOTICES.txt\t0644\n@notice-inventory\tusr/share/archy/third-party-notices.json\t0644\n")
            (package / "package.py").write_text("tool")
            (package / "notices.py").write_text("tool")
            (package / "notice-inputs.json").write_text("{}")
            with tarfile.open(archive, "w:gz") as tf:
                tf.add(src, arcname="archy-source")
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            output = base / "PKGBUILD"
            args = ["recipe", "--archive", archive, "--source-id", "v0.1.0-test",
                    "--sha256", digest, "--output", output]
            self.assertNotEqual(run_tool(*args[:-3], "0" * 64, "--output", output).returncode, 0)
            self.assertFalse(output.exists())
            result = run_tool(*args)
            self.assertEqual(result.returncode, 0, result.stderr)
            recipe = output.read_text()
            self.assertIn(digest, recipe)
            self.assertIn("cargo build --offline --locked --release -p archy-cli", recipe)
            self.assertIn("python3 packaging/arch/notices.py", recipe)
            self.assertIn("RUSTUP_AUTO_INSTALL=0", recipe)
            self.assertIn("depends=('glibc' 'libgcc')", recipe)
            self.assertIn("url='https://github.com/butahlecoq/Archy'", recipe)
            self.assertIn("options=('!debug')", recipe)
            self.assertNotIn("gcc-libs", recipe)
            self.assertNotIn("SKIP", recipe)
            syntax = subprocess.run(["bash", "-n", str(output)], capture_output=True, text=True)
            self.assertEqual(syntax.returncode, 0, syntax.stderr)


if __name__ == "__main__":
    unittest.main()
