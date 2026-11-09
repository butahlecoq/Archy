"""Check the committed historical Arch system-library record's internal closure."""
import json
from pathlib import Path
import unittest


RECORD = Path(__file__).resolve().parents[2] / 'docs/evidence/issue21/arch-system-files-54.json'


class HistoricalArchInventoryTest(unittest.TestCase):
    def test_system_library_record_is_closed_and_unique(self):
        record = json.loads(RECORD.read_text())
        rows = record['libraries']
        self.assertEqual(record['historical_inventory_sha256'],
                         '2cb7c672f6a1866cc9b0214522b9e6f26b669a47db135f6a4fca86d90857afb4')
        self.assertEqual(record['historical_system_file_count'], 54)
        self.assertEqual(len(rows), 54)
        self.assertEqual(len({row['resolved_path'] for row in rows}), 54)
        self.assertEqual(len({row['owner'] for row in rows}), 36)
        providers = {soname for row in rows for soname in row['soname']}
        self.assertEqual(len(providers), 54)
        self.assertEqual({need for row in rows for need in row['needed']} - providers, set())
        self.assertTrue(all(row['alias'] == row['resolved_path'] for row in rows))
        self.assertTrue(all(row['sha256'] and row['package_version'] for row in rows))
        by_soname = {row['soname'][0]: row for row in rows}
        for name in ('libSM.so.6', 'libICE.so.6', 'libX11.so.6', 'libxcb.so.1',
                     'libXau.so.6', 'libXdmcp.so.6', 'libuuid.so.1', 'libc.so.6',
                     'ld-linux-x86-64.so.2'):
            self.assertIn(name, by_soname)


if __name__ == '__main__':
    unittest.main()
