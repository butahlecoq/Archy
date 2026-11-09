"""Regression tests for the bounded Blender runtime probe."""

import json
import os
import subprocess
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from archy import doctor


class BlenderCheckTests(unittest.TestCase):
    def setUp(self):
        self.which = patch("archy.doctor.shutil.which", return_value="/usr/bin/blender")
        self.run = patch("archy.doctor.subprocess.run")
        self.which_mock = self.which.start()
        self.run_mock = self.run.start()
        self.addCleanup(self.which.stop)
        self.addCleanup(self.run.stop)

    def set_probe_output(self, stdout="", returncode=0):
        self.run_mock.return_value = SimpleNamespace(stdout=stdout, returncode=returncode)

    def test_missing_blender_is_reported_as_unavailable_without_launching(self):
        self.which_mock.return_value = None

        result = doctor._blender_check()

        self.assertEqual(result["status"], "unavailable")
        self.assertIn("not found", result["detail"])
        self.run_mock.assert_not_called()

    def test_timeout_is_reported_as_unavailable(self):
        self.run_mock.side_effect = subprocess.TimeoutExpired("blender", 10)

        result = doctor._blender_check()

        self.assertEqual(result["status"], "unavailable")
        self.assertIn("probe failed", result["detail"])
        self.assertEqual(self.run_mock.call_args.kwargs["timeout"], 10)

    def test_os_error_is_reported_as_unavailable(self):
        self.run_mock.side_effect = OSError("cannot execute")

        result = doctor._blender_check()

        self.assertEqual(result["status"], "unavailable")
        self.assertIn("probe failed", result["detail"])

    def test_nonzero_exit_is_reported_as_unavailable(self):
        self.set_probe_output(returncode=7)

        result = doctor._blender_check()

        self.assertEqual(result["status"], "unavailable")
        self.assertIn("status 7", result["detail"])

    def test_missing_probe_marker_is_reported_as_unavailable(self):
        self.set_probe_output("ordinary Blender output\n")

        result = doctor._blender_check()

        self.assertEqual(result["status"], "unavailable")
        self.assertIn("exactly one", result["detail"])

    def test_duplicate_probe_markers_are_reported_as_unavailable(self):
        marker = doctor._BLENDER_PROBE_MARKER + '{"version":"5.2","python":"3.14","prefix":"/usr"}'
        self.set_probe_output(f"{marker}\n{marker}\n")

        result = doctor._blender_check()

        self.assertEqual(result["status"], "unavailable")
        self.assertIn("exactly one", result["detail"])

    def test_malformed_probe_payload_is_reported_as_unavailable(self):
        self.set_probe_output(doctor._BLENDER_PROBE_MARKER + "{not json}\n")

        result = doctor._blender_check()

        self.assertEqual(result["status"], "unavailable")
        self.assertIn("malformed", result["detail"])

    def test_incomplete_probe_payload_is_reported_as_unavailable(self):
        payload = json.dumps({"version": "5.2", "python": "3.14"})
        self.set_probe_output(doctor._BLENDER_PROBE_MARKER + payload + "\n")

        result = doctor._blender_check()

        self.assertEqual(result["status"], "unavailable")
        self.assertIn("incomplete", result["detail"])

    def test_success_reports_runtime_and_cleans_child_environment(self):
        payload = json.dumps({"version": "Blender 5.2.2", "python": "3.14.8", "prefix": "/usr"})
        self.set_probe_output(doctor._BLENDER_PROBE_MARKER + payload + "\n")
        with patch.dict(os.environ, {
            "PATH": "/mise/python/bin:/usr/bin",
            "PYTHONHOME": "/mise/python",
            "PYTHONPATH": "/contaminating/modules",
            "PYTHONUNBUFFERED": "1",
            "KEEP_ME": "inherited",
        }, clear=True):
            result = doctor._blender_check()

        self.assertEqual(result["status"], "available")
        self.assertIn("Blender 5.2.2", result["detail"])
        self.assertIn("Python 3.14.8", result["detail"])
        self.which_mock.assert_called_once_with("blender", path=os.defpath)
        command = self.run_mock.call_args.args[0]
        self.assertIn("--background", command)
        self.assertIn("--factory-startup", command)
        self.assertEqual(command[command.index("--python-exit-code") + 1], "1")
        kwargs = self.run_mock.call_args.kwargs
        self.assertEqual(kwargs["env"]["PATH"], os.defpath)
        self.assertEqual(kwargs["env"]["KEEP_ME"], "inherited")
        self.assertFalse(any(name.startswith("PYTHON") for name in kwargs["env"]))


if __name__ == "__main__":
    unittest.main()
