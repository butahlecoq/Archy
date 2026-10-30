import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[2]
RECIPE = REPO / 'packaging/arch/libxt_source.py'


class LibXtSourceTest(unittest.TestCase):
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
