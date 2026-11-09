"""Cross-check the frozen A12 caller against the separately owned runtime."""
import hashlib
import importlib.util
from pathlib import Path
import re
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


manifest = load('a12_runtime_manifest', 'packaging/arch/runtime_manifest.py')
runtime = load('a12_runtime_stage', 'packaging/arch/runtime.py')


class A12RuntimeContract(unittest.TestCase):
    def test_reviewed_verifier_bytes_are_the_staged_bytes_and_rust_pins(self):
        entries = manifest.capture_tree(ROOT, allow_pending=True)
        rust = (ROOT / 'crates/archy-cli/src/ifc_verifier.rs').read_text()
        with tempfile.TemporaryDirectory() as temp:
            dest = Path(temp)
            with manifest.AnchoredRoot(dest) as stage:
                runtime.stage_captured_inputs(entries, stage)
                runtime.verify_captured_inputs(entries, stage)
            for name, const in (('reader.cjs', 'READER_SHA'),
                                ('step-scope.cjs', 'STEP_SCOPE_SHA'),
                                ('verify-stdin.cjs', 'BRIDGE_SHA')):
                source = (ROOT / 'tests/backend/ifc-reader' / name).read_bytes()
                staged = (dest / 'usr/lib/archy/runtime/ifc-verifier' / name).read_bytes()
                pin = re.search(r'const\s+' + const + r':\s*&str\s*=\s*"([0-9a-f]{64})"', rust)
                self.assertIsNotNone(pin, const)
                self.assertEqual(staged, source, name)
                self.assertEqual(hashlib.sha256(staged).hexdigest(), pin.group(1), name)

    def test_fixed_caller_paths_match_runtime_gate_and_unique_package_ownership(self):
        rust = (ROOT / 'crates/archy-cli/src/ifc_verifier.rs').read_text()
        gate = (ROOT / 'crates/archy-service-gate/src/main.rs').read_text()
        def constant(name):
            match = re.search(r'const\s+' + name + r':\s*&str\s*=\s*"([^"]+)"', rust)
            self.assertIsNotNone(match, name)
            return match.group(1)
        node = constant('RUNTIME_NODE')
        bwrap = constant('RUNTIME_BWRAP')
        verifier = constant('RUNTIME_VERIFIER')
        self.assertEqual(node, '/usr/lib/archy/runtime/node/bin/node')
        self.assertEqual(bwrap, '/usr/lib/archy/runtime/bin/bwrap')
        self.assertEqual(verifier, '/usr/lib/archy/runtime/ifc-verifier')
        self.assertIn(node, gate)
        self.assertIn(bwrap, gate)
        self.assertIn(verifier + '/reader.cjs', gate)
        core_files = (ROOT / 'packaging/arch/files.txt').read_text()
        self.assertNotIn('usr/lib/archy/ifc-verifier/', core_files)
        self.assertNotIn('usr/lib/archy/runtime/ifc-verifier/', core_files)


if __name__ == '__main__':
    unittest.main()
