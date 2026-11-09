import importlib.util
import errno
import pathlib
import tempfile
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[3]
SOURCE = ROOT / "integrations/omarchy/plugin"
spec = importlib.util.spec_from_file_location("archy_omarchy_install", ROOT / "integrations/omarchy/install.py")
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class LocalPluginTransaction(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.home = pathlib.Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)
        self.dest = self.home / ".config/omarchy/plugins/archy.status"
        self.state = self.home / ".local/state/archy/omarchy/install.json"

    def test_preview_is_nonmutating_and_apply_is_idempotent(self):
        self.assertEqual(installer.run(SOURCE, self.home)["action"], "install")
        self.assertEqual(list(self.home.iterdir()), [])
        result = installer.run(SOURCE, self.home, apply=True)
        self.assertTrue(result["applied"])
        self.assertEqual(installer.inventory(self.dest), installer.inventory(SOURCE))
        self.assertEqual(installer.run(SOURCE, self.home, apply=True)["action"], "already-installed")
        self.assertTrue(self.state.is_file())

    def test_unknown_collision_and_symlink_refuse_without_writes(self):
        self.dest.parent.mkdir(parents=True)
        self.dest.mkdir()
        (self.dest / "sentinel").write_text("owner")
        with self.assertRaisesRegex(installer.Refused, "without Archy ownership"):
            installer.run(SOURCE, self.home, apply=True)
        self.assertEqual((self.dest / "sentinel").read_text(), "owner")
        self.dest.rename(self.home / "moved")
        self.dest.symlink_to(self.home / "moved")
        with self.assertRaises(installer.Refused):
            installer.run(SOURCE, self.home, apply=True)

    def test_uninstall_backs_up_only_unchanged_owned_tree(self):
        installer.run(SOURCE, self.home, apply=True)
        sentinel = self.home / ".config/omarchy/plugins/other.plugin"
        sentinel.mkdir()
        (sentinel / "own").write_text("stay")
        self.assertEqual(installer.run(SOURCE, self.home, uninstall=True)["action"], "uninstall")
        result = installer.run(SOURCE, self.home, apply=True, uninstall=True)
        self.assertFalse(self.dest.exists())
        self.assertFalse(self.state.exists())
        self.assertEqual(installer.inventory(pathlib.Path(result["backup"])), installer.inventory(SOURCE))
        self.assertEqual((sentinel / "own").read_text(), "stay")
        self.assertEqual(installer.run(SOURCE, self.home, apply=True, uninstall=True)["action"], "already-absent")

    def test_uninstall_survives_missing_package_source(self):
        installer.run(SOURCE, self.home, apply=True)
        result = installer.run(self.home / "removed-package-root", self.home, apply=True, uninstall=True)
        self.assertTrue(result["applied"])
        self.assertFalse(self.dest.exists())

    def test_local_edit_refuses_uninstall_and_install(self):
        installer.run(SOURCE, self.home, apply=True)
        (self.dest / "Main.qml").write_text("local change")
        for uninstall in (False, True):
            with self.assertRaisesRegex(installer.Refused, "local edits"):
                installer.run(SOURCE, self.home, apply=True, uninstall=uninstall)
        self.assertTrue(self.state.exists())

    def test_same_content_replacement_is_not_owned(self):
        installer.run(SOURCE, self.home, apply=True)
        original = self.home / "original-plugin"
        self.dest.rename(original)
        self.dest.mkdir()
        for name in installer.FILES:
            (self.dest / name).write_bytes((SOURCE / name).read_bytes())
        with self.assertRaisesRegex(installer.Refused, "directory differs"):
            installer.run(SOURCE, self.home, apply=True, uninstall=True)
        self.assertTrue(self.state.exists())
        self.assertTrue(self.dest.exists())
        self.assertTrue(original.exists())

    def test_failed_state_write_rolls_back_plugin(self):
        with patch.object(installer, "atomic_state_fd", side_effect=OSError("injected")):
            with self.assertRaises(OSError):
                installer.run(SOURCE, self.home, apply=True)
        self.assertFalse(self.dest.exists())
        self.assertFalse(self.state.exists())

    def test_post_state_lookup_eio_rolls_back_for_retry(self):
        neighbor = self.home / ".config/omarchy/plugins/other.plugin"
        neighbor.mkdir(parents=True)
        (neighbor / "keep").write_text("unrelated")
        original = installer.entry_dir
        lookups = 0

        def fail_final_lookup(fd, name):
            nonlocal lookups
            if name == installer.PLUGIN_ID:
                lookups += 1
                if lookups == 2:
                    raise OSError(errno.EIO, "injected final lookup failure")
            return original(fd, name)

        with patch.object(installer, "entry_dir", fail_final_lookup):
            with self.assertRaises(OSError) as raised:
                installer.run(SOURCE, self.home, apply=True)
        self.assertEqual(raised.exception.errno, errno.EIO)
        self.assertFalse(self.dest.exists())
        self.assertFalse(self.state.exists())
        self.assertEqual((neighbor / "keep").read_text(), "unrelated")
        self.assertEqual(installer.run(SOURCE, self.home, apply=True)["action"], "install")
        self.assertEqual(installer.run(SOURCE, self.home, apply=True, uninstall=True)["action"], "uninstall")

    def test_rollback_state_unlink_eio_restores_owned_directory(self):
        original_entry = installer.entry_dir
        original_unlink = installer.os.unlink
        lookups = 0

        def fail_final_lookup(fd, name):
            nonlocal lookups
            if name == installer.PLUGIN_ID:
                lookups += 1
                if lookups == 2:
                    raise OSError(errno.EIO, "injected final lookup failure")
            return original_entry(fd, name)

        def fail_state_unlink(name, *args, **kwargs):
            if name == "install.json":
                raise OSError(errno.EIO, "injected state unlink failure")
            return original_unlink(name, *args, **kwargs)

        with patch.object(installer, "entry_dir", fail_final_lookup), patch.object(installer.os, "unlink", fail_state_unlink):
            with self.assertRaises(OSError):
                installer.run(SOURCE, self.home, apply=True)
        self.assertTrue(self.dest.is_dir())
        self.assertTrue(self.state.is_file())
        self.assertEqual(installer.run(SOURCE, self.home, apply=True)["action"], "already-installed")
        self.assertEqual(installer.run(SOURCE, self.home, apply=True, uninstall=True)["action"], "uninstall")

    def test_state_temp_cleanup_eio_keeps_published_state_coherent(self):
        original_unlink = installer.os.unlink

        def fail_temp_unlink(name, *args, **kwargs):
            if str(name).startswith(".install-"):
                raise OSError(errno.EIO, "injected temporary state cleanup failure")
            return original_unlink(name, *args, **kwargs)

        with patch.object(installer.os, "unlink", fail_temp_unlink):
            with self.assertRaises(OSError):
                installer.run(SOURCE, self.home, apply=True)
        self.assertEqual(self.dest.exists(), self.state.exists())
        self.assertEqual(installer.run(SOURCE, self.home, apply=True)["action"], "install")
        self.assertEqual(installer.run(SOURCE, self.home, apply=True, uninstall=True)["action"], "uninstall")

    def test_malformed_state_and_atomic_state_collision_refuse(self):
        self.state.parent.mkdir(parents=True)
        self.state.write_text('[]')
        with self.assertRaisesRegex(installer.Refused, "unrecognized ownership state"):
            installer.run(SOURCE, self.home, apply=True)
        with self.assertRaises(FileExistsError):
            installer.atomic_state(self.state, {"schema_version": 1})
        self.assertEqual(self.state.read_text(), '[]')

    def test_source_symlink_refused(self):
        clone = self.home / "source"
        clone.mkdir()
        for name in installer.FILES:
            (clone / name).write_bytes((SOURCE / name).read_bytes())
        (clone / "Main.qml").unlink()
        (clone / "Main.qml").symlink_to(SOURCE / "Main.qml")
        with self.assertRaisesRegex(installer.Refused, "regular file"):
            installer.run(clone, self.home, apply=True)

    def test_install_swap_cannot_overwrite_unrelated_file(self):
        outside = self.home / "outside"
        outside.mkdir()
        victim = outside / "Main.qml"
        victim.write_text("UNRELATED USER FILE")
        rename = installer.rename_exclusive
        def swap(src_fd, src, dst_fd, dst):
            if dst == installer.PLUGIN_ID:
                self.dest.mkdir()
                self.dest.rename(self.home / "displaced-empty-plugin")
                self.dest.symlink_to(outside, target_is_directory=True)
            return rename(src_fd, src, dst_fd, dst)
        with patch.object(installer, "rename_exclusive", swap):
            with self.assertRaises(OSError):
                installer.run(SOURCE, self.home, apply=True)
        self.assertEqual(victim.read_text(), "UNRELATED USER FILE")
        self.assertFalse(self.state.exists())

    def test_post_publication_swap_does_not_report_success(self):
        outside = self.home / "outside"
        outside.mkdir()
        saved = self.home / "saved-owned"
        publish = installer.atomic_state_fd
        def swap(fd, value):
            publish(fd, value)
            self.dest.rename(saved)
            self.dest.symlink_to(outside, target_is_directory=True)
        with patch.object(installer, "atomic_state_fd", swap):
            with self.assertRaisesRegex(installer.Refused, "changed after ownership publication"):
                installer.run(SOURCE, self.home, apply=True)
        self.assertTrue(self.state.exists())
        self.assertTrue(saved.exists())
        self.assertTrue(self.dest.is_symlink())

    def test_uninstall_swap_preserves_ownership_state_and_unrelated_tree(self):
        installer.run(SOURCE, self.home, apply=True)
        saved = self.home / "saved-owned"
        unrelated = self.home / "unrelated-plugin"
        unrelated.mkdir()
        for name in installer.FILES:
            (unrelated / name).write_bytes((SOURCE / name).read_bytes())
        rename = installer.rename_exclusive
        def swap(src_fd, src, dst_fd, dst):
            if src == installer.PLUGIN_ID and str(dst).startswith("removed-"):
                self.dest.rename(saved)
                unrelated.rename(self.dest)
            return rename(src_fd, src, dst_fd, dst)
        with patch.object(installer, "rename_exclusive", swap):
            with self.assertRaisesRegex(installer.Refused, "changed during uninstall"):
                installer.run(SOURCE, self.home, apply=True, uninstall=True)
        self.assertTrue(self.state.exists())
        self.assertTrue(saved.exists())
        self.assertEqual(installer.inventory(self.dest), installer.inventory(unrelated) if unrelated.exists() else installer.inventory(SOURCE))
        self.assertEqual(list(self.state.parent.glob("removed-*")), [])


if __name__ == "__main__":
    unittest.main()
