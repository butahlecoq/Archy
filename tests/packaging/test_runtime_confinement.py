import importlib.util
import errno
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


manifest = load('runtime_manifest_confinement', 'packaging/arch/runtime_manifest.py')
runtime = load('runtime_confinement', 'packaging/arch/runtime.py')
snapshot = load('runtime_snapshot_confinement', 'packaging/arch/runtime_snapshot.py')
recipe = load('runtime_recipe_confinement', 'packaging/arch/runtime_recipe.py')


class RuntimeConfinement(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)

    def test_source_parent_swap_cannot_read_external_file(self):
        root = self.base / 'source'
        parent = root / 'workers/ifc'
        parent.mkdir(parents=True)
        (parent / 'worker.py').write_bytes(b'reviewed')
        external = self.base / 'external'
        external.mkdir()
        (external / 'worker.py').write_bytes(b'unreviewed')
        original_open = manifest.open_component
        triggered = False

        def swap(fd, path, flags, mode=0):
            nonlocal triggered
            if not triggered and str(path).endswith('worker.py'):
                triggered = True
                parent.rename(parent.with_name('ifc-original'))
                parent.symlink_to(external, target_is_directory=True)
            return original_open(fd, path, flags, mode)

        with patch.object(manifest, 'open_component', swap):
            try:
                result = manifest.read_regular(root, 'workers/ifc/worker.py')
            except (OSError, ValueError):
                result = None
        self.assertTrue(triggered)
        self.assertIn(result, (None, b'reviewed'))

    def test_actual_captured_staging_ignores_later_source_changes(self):
        source = self.base / 'source'
        (source / 'packaging/arch').mkdir(parents=True)
        for name in ('runtime.py', 'runtime_manifest.py', 'runtime_snapshot.py',
                     'runtime_recipe.py', 'runtime-inputs.json'):
            shutil.copyfile(ROOT / 'packaging/arch' / name, source / 'packaging/arch' / name)
        for name in ('runtime-verifier', 'runtime-notices'):
            shutil.copytree(ROOT / 'packaging/arch' / name, source / 'packaging/arch' / name)
        for name in ('blender', 'ifc'):
            shutil.copytree(ROOT / 'workers' / name, source / 'workers' / name,
                            ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
        entries = manifest.capture_tree(source, allow_pending=True)
        targets = (
            'workers/ifc/archy_ifc_worker/worker.py',
            'packaging/arch/runtime-verifier/reader.cjs',
            'packaging/arch/runtime-notices/ifcopenshell-COPYING',
        )
        for name in targets:
            (source / name).write_bytes(b'unreviewed after capture')
        dest = self.base / 'stage'
        dest.mkdir()
        with manifest.AnchoredRoot(dest) as stage:
            runtime.stage_captured_inputs(entries, stage)
            runtime.verify_captured_inputs(entries, stage)
            stage.verify_outputs()
            staged_worker = dest / 'usr/lib/archy/runtime/workers/ifc/archy_ifc_worker/worker.py'
            staged_worker.write_bytes(b'unreviewed destination replacement')
            with self.assertRaisesRegex(ValueError, 'staged reviewed input changed'):
                runtime.verify_captured_inputs(entries, stage)
            with self.assertRaisesRegex(ValueError, 'staged file changed'):
                stage.verify_outputs()
            staged_worker.write_bytes(entries[targets[0]])
            staged_notice = dest / 'usr/share/licenses/archy-runtime/runtime/ifcopenshell-COPYING'
            staged_notice.write_bytes(b'unreviewed distribution notice')
            with self.assertRaisesRegex(ValueError, 'staged file changed'):
                stage.verify_outputs()
            staged_notice.write_bytes(entries[targets[2]])
            stage.verify_outputs()
        for original, staged in (
            (targets[0], 'usr/lib/archy/runtime/workers/ifc/archy_ifc_worker/worker.py'),
            (targets[0], 'usr/lib/archy/runtime/python/lib/python3.12/site-packages/archy_ifc_worker/worker.py'),
            (targets[1], 'usr/lib/archy/runtime/ifc-verifier/reader.cjs'),
            (targets[2], 'usr/lib/archy/runtime/licenses/ifcopenshell-COPYING'),
            (targets[2], 'usr/share/licenses/archy-runtime/runtime/ifcopenshell-COPYING'),
        ):
            self.assertEqual((dest / staged).read_bytes(), entries[original])

    def test_destination_parent_swap_cannot_write_external_file(self):
        dest = self.base / 'stage'
        dest.mkdir()
        external = self.base / 'external'
        external.mkdir()
        (external / 'sentinel').write_bytes(b'keep')
        parent = dest / 'safe'
        parent.mkdir()
        original_open = manifest.open_component
        triggered = False

        def swap(fd, path, flags, mode=0):
            nonlocal triggered
            if not triggered and str(path) == 'worker.py' and flags & os.O_CREAT:
                triggered = True
                parent.rename(dest / 'safe-original')
                parent.symlink_to(external, target_is_directory=True)
            return original_open(fd, path, flags, mode)

        with manifest.AnchoredRoot(dest) as stage, patch.object(manifest, 'open_component', swap):
            with self.assertRaises((OSError, ValueError)):
                stage.write_bytes('safe/worker.py', b'reviewed', 0o644)
        self.assertTrue(triggered)
        self.assertFalse((external / 'worker.py').exists())
        self.assertEqual((external / 'sentinel').read_bytes(), b'keep')

    def test_snapshot_parent_swap_cannot_write_external_file(self):
        parent = self.base / 'output'
        parent.mkdir()
        external = self.base / 'external'
        external.mkdir()
        namespace = snapshot.AnchoredRoot.create_file.__wrapped__.__globals__
        original_open = namespace['open_component']
        triggered = False

        def swap(fd, path, flags, mode=0):
            nonlocal triggered
            if not triggered and str(path) == 'snapshot.tar.gz' and flags & os.O_CREAT:
                triggered = True
                parent.rename(self.base / 'original-output')
                parent.symlink_to(external, target_is_directory=True)
            return original_open(fd, path, flags, mode)

        with patch.dict(namespace, {'open_component': swap}):
            with self.assertRaises((OSError, ValueError)):
                snapshot.write_snapshot({'worker.py': b'reviewed'}, parent / 'snapshot.tar.gz')
        self.assertTrue(triggered)
        self.assertFalse((external / 'snapshot.tar.gz').exists())
        self.assertFalse((self.base / 'original-output/snapshot.tar.gz').exists())

    def test_snapshot_replacement_after_verification_is_rejected(self):
        output = self.base / 'snapshot.tar.gz'
        original = snapshot.AnchoredRoot.verify_outputs
        replaced = False

        def swap_after_verification(root, *args, **kwargs):
            nonlocal replaced
            result = original(root, *args, **kwargs)
            replacement = self.base / 'replacement'
            replacement.write_bytes(b'UNREVIEWED SNAPSHOT')
            os.replace(replacement, output)
            replaced = True
            return result

        with patch.object(snapshot.AnchoredRoot, 'verify_outputs', swap_after_verification):
            with self.assertRaises((OSError, ValueError)):
                snapshot.write_snapshot({'worker.py': b'reviewed'}, output)
        self.assertTrue(replaced)

    def test_recipe_parent_swap_cannot_write_external_file(self):
        parent = self.base / 'recipe-output'
        parent.mkdir()
        external = self.base / 'recipe-external'
        external.mkdir()
        namespace = recipe.AnchoredRoot.create_file.__wrapped__.__globals__
        original_open = namespace['open_component']
        triggered = False

        def swap(fd, path, flags, mode=0):
            nonlocal triggered
            if not triggered and str(path) == 'PKGBUILD' and flags & os.O_CREAT:
                triggered = True
                parent.rename(self.base / 'recipe-original')
                parent.symlink_to(external, target_is_directory=True)
            return original_open(fd, path, flags, mode)

        with patch.dict(namespace, {'open_component': swap}):
            with self.assertRaises((OSError, ValueError)):
                recipe.write_recipe(parent / 'PKGBUILD', b'reviewed')
        self.assertTrue(triggered)
        self.assertFalse((external / 'PKGBUILD').exists())

    def test_recipe_writes_expected_bytes_and_hashes_regular_input(self):
        output = self.base / 'PKGBUILD'
        recipe.write_recipe(output, b'reviewed recipe')
        self.assertEqual(output.read_bytes(), b'reviewed recipe')
        self.assertEqual(recipe.sha(output), hashlib.sha256(b'reviewed recipe').hexdigest())
        link = self.base / 'link'
        link.symlink_to(output)
        with self.assertRaises(OSError):
            recipe.sha(link)

    def test_snapshot_in_place_mutation_after_hash_is_rejected(self):
        output = self.base / 'snapshot.tar.gz'
        original = snapshot.AnchoredRoot.open_read
        opened = 0

        def mutate_before_final_open(root, name):
            nonlocal opened
            opened += 1
            if opened == 2:
                output.write_bytes(b'changed after held-descriptor hash')
            return original(root, name)

        with patch.object(snapshot.AnchoredRoot, 'open_read', mutate_before_final_open):
            with self.assertRaises((OSError, ValueError)):
                snapshot.write_snapshot({'worker.py': b'reviewed'}, output)
        self.assertEqual(opened, 2)

    def test_recipe_replacement_after_verification_is_rejected(self):
        output = self.base / 'PKGBUILD'
        original = recipe.AnchoredRoot.verify_outputs
        replaced = False

        def swap_after_verification(root, *args, **kwargs):
            nonlocal replaced
            result = original(root, *args, **kwargs)
            replacement = self.base / 'replacement'
            replacement.write_bytes(b'unreviewed recipe')
            os.replace(replacement, output)
            replaced = True
            return result

        with patch.object(recipe.AnchoredRoot, 'verify_outputs', swap_after_verification):
            with self.assertRaises((OSError, ValueError)):
                recipe.write_recipe(output, b'reviewed')
        self.assertTrue(replaced)

    def test_bind_mount_transitions_are_confined(self):
        if shutil.which('unshare') is None or shutil.which('mount') is None:
            self.skipTest('unshare and mount are required for the bind-mount regression')
        available = subprocess.run(['unshare', '-Urnm', 'true'], text=True, capture_output=True)
        if available.returncode:
            self.skipTest('unprivileged user/mount namespace unavailable: ' + available.stderr.strip())
        result = subprocess.run(
            ['unshare', '-Urnm', sys.executable, str(ROOT / 'tests/packaging/runtime_mount_probe.py'),
             str(self.base)], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_missing_openat2_fails_closed(self):
        root = self.base / 'source'
        root.mkdir()
        (root / 'worker.py').write_bytes(b'reviewed')

        class Unavailable:
            def syscall(self, *args):
                manifest.ctypes.set_errno(errno.ENOSYS)
                return -1

        with manifest.AnchoredRoot(root) as source:
            with patch.object(manifest.ctypes, 'CDLL', return_value=Unavailable()):
                with self.assertRaises(OSError) as error:
                    source.read_bytes('worker.py')
        self.assertEqual(error.exception.errno, errno.ENOSYS)


if __name__ == '__main__':
    unittest.main()
