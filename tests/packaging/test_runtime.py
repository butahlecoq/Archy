import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

TOOL = Path(__file__).resolve().parents[2] / 'packaging/arch/runtime.py'
SPEC = importlib.util.spec_from_file_location('archy_runtime_tool', TOOL)
runtime = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runtime)


class RuntimeInputs(unittest.TestCase):
    def test_blender_archive_rejects_escaping_link(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            archive = base / 'blender.tar.xz'
            with tarfile.open(archive, 'w:xz') as tf:
                item = tarfile.TarInfo('blender-4.5.15-linux-x64/blender')
                data = b'bin'
                item.size = len(data)
                tf.addfile(item, io.BytesIO(data))
                item = tarfile.TarInfo('blender-4.5.15-linux-x64/lib/escape')
                item.type = tarfile.SYMTYPE
                item.linkname = '../../secret'
                tf.addfile(item)
            with runtime.AnchoredRoot(base) as stage, self.assertRaisesRegex(ValueError, 'unsafe archive member'):
                runtime.extract_blender(archive.read_bytes(), stage, Path('stage'))

    def test_wheel_hash_belongs_to_its_own_locked_package(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            house = base / 'wheels'
            house.mkdir()
            entries = []
            for n in range(15):
                name = f'pkg{n}'
                wheel = house / f'{name}-1.0-py3-none-any.whl'
                with zipfile.ZipFile(wheel, 'w') as archive:
                    archive.writestr(f'{name}/__init__.py', str(n))
                    archive.writestr(f'{name}-1.0.dist-info/METADATA',
                                     f'Name: {name}\nVersion: 1.0\nLicense-File: LICENSE\n')
                    archive.writestr(f'{name}-1.0.dist-info/licenses/LICENSE', 'MIT')
                entries.append((name, wheel, hashlib.sha256(wheel.read_bytes()).hexdigest()))
            lock = base / 'requirements.lock'
            lock.write_text(''.join(f'{name}==1.0 \\\n    --hash=sha256:{hash_value}\n' for name, _, hash_value in entries))
            with runtime.AnchoredRoot(base) as stage:
                runtime.install_wheels(house, lock.read_bytes(), stage, Path('site'), Path('notices'))
            self.assertEqual((base / 'site/pkg0/__init__.py').read_text(), '0')
            self.assertEqual((base / 'notices/pkg0-1.0/LICENSE').read_text(), 'MIT')
            # A digest present elsewhere in the lock must not authorize this package.
            bad = base / 'wrong.lock'
            bad.write_text(lock.read_text().replace(entries[0][2], entries[1][2]))
            with runtime.AnchoredRoot(base) as stage, self.assertRaisesRegex(ValueError, 'wheel absent from hash lock'):
                runtime.install_wheels(house, bad.read_bytes(), stage, Path('other-site'))

            with zipfile.ZipFile(entries[0][1], 'w') as archive:
                archive.writestr('pkg0-1.0.dist-info/METADATA',
                                 'Name: pkg0\nVersion: 1.0\nLicense-File: LICENSE\n')
            missing = hashlib.sha256(entries[0][1].read_bytes()).hexdigest()
            lock.write_text(lock.read_text().replace(entries[0][2], missing))
            with runtime.AnchoredRoot(base) as stage, self.assertRaisesRegex(ValueError, 'wheel license text missing'):
                runtime.install_wheels(house, lock.read_bytes(), stage, Path('missing-license'))


if __name__ == '__main__':
    unittest.main()
