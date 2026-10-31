import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
VERIFIERS = ('reader.cjs', 'step-scope.cjs', 'verify-stdin.cjs')
WORKERS = (
    'workers/blender/LICENSE', 'workers/blender/worker.py',
    'workers/blender/scene.py', 'workers/blender/protocol.py',
    'workers/ifc/LICENSE', 'workers/ifc/requirements.lock',
    'workers/ifc/archy_ifc_worker/__init__.py', 'workers/ifc/archy_ifc_worker/__main__.py',
    'workers/ifc/archy_ifc_worker/geometry.py', 'workers/ifc/archy_ifc_worker/hierarchy.py',
    'workers/ifc/archy_ifc_worker/properties.py', 'workers/ifc/archy_ifc_worker/protocol.py',
    'workers/ifc/archy_ifc_worker/verify.py', 'workers/ifc/archy_ifc_worker/worker.py',
)
NOTICES = ('bubblewrap-COPYING', 'ifcopenshell-COPYING', 'ifcopenshell-COPYING.LESSER',
           'ifc-mvd-LICENSE', 'ifc-simple-spf-LICENSE')


def digest(data):
    return hashlib.sha256(data).hexdigest()


class SnapshotPreflight(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.source = self.base / 'source'
        self.source.mkdir()
        self.files = {}
        for path in WORKERS + tuple('packaging/arch/runtime-verifier/' + n for n in VERIFIERS):
            data = ('fixture: ' + path).encode()
            self.write(path, data)
            self.files[path] = digest(data)
        for name in ('runtime.py', 'runtime_manifest.py', 'runtime_recipe.py', 'runtime_snapshot.py'):
            path = 'packaging/arch/' + name
            self.write(path, (ROOT / path).read_bytes())
        notice_manifest = {}
        for name in NOTICES:
            data = ('notice: ' + name).encode()
            self.write('packaging/arch/runtime-notices/' + name, data)
            notice_manifest[name] = {'url': 'https://example.invalid/' + name, 'sha256': digest(data)}
        self.write('packaging/arch/runtime-notices/sources.json', json.dumps(notice_manifest).encode())
        bound = {path.relative_to(self.source).as_posix(): digest(path.read_bytes())
                 for path in self.source.rglob('*') if path.is_file() and
                 path.relative_to(self.source).as_posix() != 'packaging/arch/runtime_manifest.py'}
        identity = digest(json.dumps(bound, sort_keys=True, separators=(',', ':')).encode())
        self.reviews = {}
        for name in ('a12', 'distribution'):
            path = 'packaging/arch/runtime-review/' + name + '.md'
            data = ('synthetic test review: ' + name + '\nRuntime-Input-Set-SHA256: ' + identity + '\n').encode()
            self.write(path, data)
            self.reviews[name] = {'status': 'reviewed', 'evidence_path': path, 'sha256': digest(data)}
        guard = self.source / 'packaging/arch/runtime_manifest.py'
        text = guard.read_text()
        old = "TRUSTED_REVIEWS = {'a12': frozenset(), 'distribution': frozenset()}"
        new = ("TRUSTED_REVIEWS = {'a12': frozenset(['" + self.reviews['a12']['sha256'] +
               "']), 'distribution': frozenset(['" + self.reviews['distribution']['sha256'] + "'])}")
        if text.count(old) != 1:
            raise AssertionError('synthetic trust fixture no longer matches guard')
        guard.write_text(text.replace(old, new))
        spec = importlib.util.spec_from_file_location('fixture_runtime_manifest', guard)
        self.validator = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.validator)
        self.manifest = {'schema_version': 1, 'files': self.files, 'reviews': self.reviews}
        self.save_manifest()

    def write(self, path, data):
        target = self.source / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)

    def save_manifest(self):
        self.write('packaging/arch/runtime-inputs.json', json.dumps(self.manifest).encode())

    def snapshot(self, name, env=None):
        output = self.base / name
        result = subprocess.run(['/usr/bin/python3', '-I', '-S', '-B', str(self.source / 'packaging/arch/runtime_snapshot.py'), '--source', str(self.source),
                                 '--output', str(output)], capture_output=True, text=True, env=env)
        return result, output

    def test_missing_verifier_refused_before_output(self):
        shutil.rmtree(self.source / 'packaging/arch/runtime-verifier')
        result, output = self.snapshot('missing.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(output.exists())

    def test_changed_worker_and_unreviewed_inputs_refused_before_output(self):
        (self.source / 'workers/ifc/archy_ifc_worker/properties.py').write_bytes(b'changed')
        result, output = self.snapshot('changed.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(output.exists())
        self.write('workers/ifc/archy_ifc_worker/properties.py', b'fixture: workers/ifc/archy_ifc_worker/properties.py')
        self.reviews['distribution'] = {'status': 'pending', 'evidence_path': None, 'sha256': None}
        self.save_manifest()
        result, output = self.snapshot('pending.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(output.exists())
        dest = self.base / 'empty-stage'
        dest.mkdir()
        command = ['/usr/bin/python3', '-I', '-S', '-B', str(self.source / 'packaging/arch/runtime.py'),
                   '--dest', str(dest), '--source', str(self.source)]
        for option in ('python-prefix', 'python-license', 'blender-archive', 'blender-sha256',
                       'node-archive', 'node-sha256', 'web-ifc-archive', 'web-ifc-sha256',
                       'wheelhouse', 'bwrap', 'bwrap-sha256'):
            command += ['--' + option, '/not-accessed']
        staged = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(staged.returncode, 0)
        self.assertIn('distribution review is not complete', staged.stderr)
        self.assertEqual(list(dest.iterdir()), [])
        identity = subprocess.run(['/usr/bin/python3', '-I', '-S', '-B', str(self.source / 'packaging/arch/runtime_manifest.py'),
                                   '--source', str(self.source), '--identity'], capture_output=True, text=True)
        self.assertEqual(identity.returncode, 0, identity.stderr)
        self.assertFalse(json.loads(identity.stdout)['build_ready'])

    def test_changed_verifier_and_unlisted_worker_refused_before_output(self):
        verifier = self.source / 'packaging/arch/runtime-verifier/reader.cjs'
        original = verifier.read_bytes()
        verifier.write_bytes(b'changed')
        result, output = self.snapshot('verifier.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(output.exists())
        verifier.write_bytes(original)
        self.write('workers/ifc/extra.py', b'not in manifest')
        result, output = self.snapshot('extra.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(output.exists())

    def test_repinning_worker_does_not_reuse_stale_review(self):
        name = 'workers/ifc/archy_ifc_worker/properties.py'
        self.write(name, b'repinned worker')
        self.files[name] = digest(b'repinned worker')
        self.save_manifest()
        result, output = self.snapshot('stale-review.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('review does not bind current inputs', result.stderr)
        self.assertFalse(output.exists())

    def test_manifest_cannot_self_authorize_a_review(self):
        self.write('packaging/arch/runtime_manifest.py',
                   (ROOT / 'packaging/arch/runtime_manifest.py').read_bytes())
        result, output = self.snapshot('untrusted-review.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('review evidence is not trusted', result.stderr)
        self.assertFalse(output.exists())

    def test_manifest_cannot_omit_required_file_or_follow_symlink(self):
        name = 'packaging/arch/runtime-verifier/step-scope.cjs'
        self.files.pop(name)
        self.save_manifest()
        result, output = self.snapshot('omitted-pin.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(output.exists())
        data = (self.source / name).read_bytes()
        self.files[name] = digest(data)
        self.save_manifest()
        path = self.source / name
        path.unlink()
        external = self.base / 'external'
        external.write_bytes(data)
        path.symlink_to(external)
        result, output = self.snapshot('symlink.tar.gz')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(output.exists())

    def test_valid_reviewed_fixture_is_complete_and_deterministic(self):
        first, a = self.snapshot('a.tar.gz')
        second, b = self.snapshot('b.tar.gz')
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(a.read_bytes(), b.read_bytes())
        with tarfile.open(a, 'r:gz') as archive:
            names = set(archive.getnames())
        for path in self.files:
            self.assertIn('archy-source/' + path, names)
        self.assertIn('archy-source/packaging/arch/runtime-inputs.json', names)
        self.validator.validate_archive(a)
        hostile = self.base / 'hostile'
        hostile.mkdir()
        marker = self.base / 'executed'
        (hostile / 'sitecustomize.py').write_text(f'open({str(marker)!r}, "w").write("executed")\n')
        isolated, _ = self.snapshot('isolated.tar.gz', env={'PATH': str(hostile),
                                    'PYTHONPATH': str(hostile), 'PYTHONHOME': str(hostile)})
        self.assertEqual(isolated.returncode, 0, isolated.stderr)
        self.assertFalse(marker.exists())
        broken = self.base / 'broken.tar.gz'
        with tarfile.open(a, 'r:gz') as source, tarfile.open(broken, 'w:gz') as target:
            for item in source:
                if item.name.endswith('/runtime-verifier/reader.cjs'):
                    continue
                target.addfile(item, io.BytesIO(source.extractfile(item).read()))
        with self.assertRaisesRegex(ValueError, 'required input missing'):
            self.validator.validate_archive(broken)
        recipe = self.base / 'PKGBUILD'
        result = subprocess.run(['/usr/bin/python3', '-I', '-S', '-B', str(self.source / 'packaging/arch/runtime_recipe.py'),
            '--source', str(broken), '--python', '/missing/python', '--blender', '/missing/blender',
            '--node', '/missing/node', '--web-ifc', '/missing/web-ifc',
            '--bwrap-package', '/missing/bwrap', '--wheelhouse', '/missing/wheels',
            '--output', str(recipe)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('required input missing', result.stderr)
        self.assertFalse(recipe.exists())


if __name__ == '__main__':
    unittest.main()
