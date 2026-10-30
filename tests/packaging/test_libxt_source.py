import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[2]
RECIPE = REPO / 'packaging/arch/libxt_source.py'
spec = importlib.util.spec_from_file_location('libxt_source', RECIPE)
libxt_source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(libxt_source)


class LibXtSourceTest(unittest.TestCase):
    @unittest.skipUnless(sys.platform.startswith('linux') and shutil.which('cc') and shutil.which('readelf'),
                         'requires Linux, a C compiler, and readelf')
    def test_rejects_embedded_library_search_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'fixture.c'
            source.write_text('void fixture(void) {}\n')
            names = ('libSM.so.6', 'libICE.so.6', 'libX11.so.6', 'libc.so.6')
            for name in names:
                subprocess.run(['cc', '-shared', '-fPIC', '-nostdlib', str(source),
                                '-Wl,-soname,' + name, '-o', str(root / name)], check=True)
            for tag, flags in (('none', []),
                               ('RPATH', ['-Wl,--disable-new-dtags,-rpath,/fixture']),
                               ('RUNPATH', ['-Wl,--enable-new-dtags,-rpath,/fixture'])):
                with self.subTest(tag=tag):
                    library = root / (tag + '.so')
                    subprocess.run(['cc', '-shared', '-fPIC', '-nostdlib', str(source),
                                    '-Wl,-soname,libXt.so.6', '-Wl,--no-as-needed',
                                    '-L' + str(root), *(f'-l:{name}' for name in names),
                                    *flags, '-o', str(library)], check=True)
                    if tag == 'none':
                        self.assertEqual(libxt_source.check_linkage(library)[0], 'libXt.so.6')
                    else:
                        with self.assertRaisesRegex(ValueError, tag):
                            libxt_source.check_linkage(library)

    def test_wrong_source_stops_before_creating_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'libXt-1.3.1.tar.xz'
            source.write_bytes(b'not the pinned source')
            output = root / 'candidate'
            result = subprocess.run(
                [sys.executable, str(RECIPE), '--tarball', str(source), '--output', str(output)],
                capture_output=True, text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('source SHA-256 mismatch', result.stderr)
            self.assertFalse(output.exists())

    @unittest.skipUnless(os.environ.get('LIBXT_SOURCE_TARBALL'), 'set LIBXT_SOURCE_TARBALL for source build')
    def test_pinned_source_builds_expected_soname(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'candidate'
            command = [sys.executable, str(RECIPE), '--tarball', os.environ['LIBXT_SOURCE_TARBALL'],
                       '--output', str(output)]
            if os.environ.get('LIBXT_HOST_PROBE') == '1':
                command.append('--host-probe')
            result = subprocess.run(
                command, capture_output=True, text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue((output / 'stage/usr/lib/libXt.so.6.0.0').is_file())
            self.assertIn('"status": "source_candidate_only"', result.stdout)


if __name__ == '__main__':
    unittest.main()
