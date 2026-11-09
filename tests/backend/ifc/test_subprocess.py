"""Bounded native-child behavior; no IFC import required."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

from test_protocol import ROOT, request

sys.path.insert(0, str(ROOT / "workers" / "ifc"))
from archy_ifc_worker import worker


class NativeChildBoundsTests(unittest.TestCase):
    def test_noisy_child_has_bounded_diagnostics(self):
        result = worker.run_native_process(
            [sys.executable, "-c", "import sys; sys.stdout.write('x' * 100000); sys.stdout.flush()"],
            timeout_s=2, max_output_bytes=1024)
        self.assertEqual(result.failure, "output_limit")
        self.assertLessEqual(len(result.stdout.encode()), 1024)

    def test_sleeping_child_times_out(self):
        started = time.monotonic()
        result = worker.run_native_process(
            [sys.executable, "-c", "import time; time.sleep(30)"],
            timeout_s=0.3, max_output_bytes=1024)
        self.assertEqual(result.failure, "timeout")
        self.assertLess(time.monotonic() - started, 3)

    def test_descendant_held_pipes_do_not_hold_parent(self):
        script = ("import subprocess,sys; "
                  "subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); "
                  "print('parent exited', flush=True)")
        started = time.monotonic()
        result = worker.run_native_process([sys.executable, "-c", script],
                                           timeout_s=0.4, max_output_bytes=1024)
        self.assertEqual(result.failure, "timeout")
        self.assertIn("parent exited", result.stdout)
        self.assertLess(time.monotonic() - started, 3)

    def test_native_failure_preserves_model_and_bounded_diagnostics(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            item = request()
            source = output / "request.json"
            source.write_text(json.dumps(item))
            fake = worker.NativeResult(None, "partial", "native stalled", "timeout")
            with patch.object(worker, "run_native_process", return_value=fake):
                with self.assertRaises(worker.protocol.ProtocolError) as caught:
                    worker.run(item, output, source)
            self.assertEqual(caught.exception.code, "BACKEND_TIMEOUT")
            self.assertTrue((output / "model.ifc").is_file())
            validation = json.loads((output / "validation.json").read_text())
            self.assertEqual(validation["reopen_process"]["failure"], "timeout")
            self.assertEqual(validation["reopen_process"]["stderr"], "native stalled")

    def test_validator_output_limit_is_artifact_error_with_retained_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            item = request()
            source = output / "request.json"
            source.write_text(json.dumps(item))
            reopen = worker.NativeResult(0, '{"status":"ok"}', "", None)
            noisy = worker.NativeResult(-9, "bounded diagnostic", "validator noisy", "output_limit")
            with patch.object(worker, "run_native_process", side_effect=[reopen, noisy]):
                with self.assertRaises(worker.protocol.ProtocolError) as caught:
                    worker.run(item, output, source)
            self.assertEqual(caught.exception.code, "ARTIFACT_INVALID")
            validation = json.loads((output / "validation.json").read_text())
            self.assertEqual(validation["express_validation"]["failure"], "output_limit")
            self.assertIn("bounded diagnostic", validation["express_validation"]["findings"])

    def test_fresh_process_scalar_json_is_artifact_error(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            item = request()
            source = output / "request.json"
            source.write_text(json.dumps(item))
            scalar = worker.NativeResult(0, "[]", "", None)
            with patch.object(worker, "run_native_process", return_value=scalar):
                with self.assertRaises(worker.protocol.ProtocolError) as caught:
                    worker.run(item, output, source)
            self.assertEqual(caught.exception.code, "ARTIFACT_INVALID")
            self.assertEqual(json.loads((output / "validation.json").read_text())["status"], "error")

    def test_fresh_process_caps_request_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / "large.json"
            source.write_bytes(b"x" * (1024 * 1024 + 1))
            result = subprocess.run([sys.executable, "-I", "-m", "archy_ifc_worker.verify",
                                     "--ifc", str(Path(temp) / "model.ifc"),
                                     "--request", str(source)], capture_output=True, text=True,
                                    timeout=5)
            self.assertEqual(result.returncode, 20, result.stderr)
            self.assertEqual(json.loads(result.stdout)["status"], "error")


if __name__ == "__main__":
    unittest.main()
