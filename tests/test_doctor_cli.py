"""Regression coverage for the local doctor command."""

import builtins
import contextlib
import io
import json
import subprocess
import unittest
from unittest import mock

from archy import doctor


class DoctorCliTests(unittest.TestCase):
    def test_json_output_is_parseable_and_does_not_leak_blender_output(self):
        payload = {"version": "5.0", "python": "3.12.1", "prefix": "/blender/python"}
        blender_result = subprocess.CompletedProcess(
            args=["blender"],
            returncode=0,
            stdout="Blender startup chatter\n"
            + doctor._BLENDER_PROBE_MARKER
            + json.dumps(payload)
            + "\nMore Blender chatter\n",
            stderr="Blender stderr chatter\n",
        )
        stdout = io.StringIO()
        stderr = io.StringIO()
        with (
            mock.patch.object(doctor.shutil, "which", return_value="/usr/bin/blender"),
            mock.patch.object(doctor.subprocess, "run", return_value=blender_result) as run,
            mock.patch.object(
                doctor,
                "_ifc_check",
                return_value=doctor._check("ifc", "available", "fixture IFC runtime"),
            ),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            exit_code = doctor.main(["--json"])

        self.assertEqual(exit_code, 0)
        report = json.loads(stdout.getvalue())
        blender = next(item for item in report["checks"] if item["name"] == "blender")
        self.assertEqual(blender["status"], "available")
        self.assertIn("Blender 5.0", blender["detail"])
        self.assertNotIn("chatter", stdout.getvalue())
        self.assertEqual(stderr.getvalue(), "")
        self.assertTrue(run.call_args.kwargs["capture_output"])

    def test_text_output_includes_actionable_remedy(self):
        report = {
            "checks": [
                doctor._check(
                    "blender",
                    "unavailable",
                    "Blender was not found.",
                    "Install Blender and ensure its executable is on the system PATH.",
                )
            ]
        }
        stdout = io.StringIO()
        with mock.patch.object(doctor, "diagnose", return_value=report), contextlib.redirect_stdout(stdout):
            exit_code = doctor.main([])

        self.assertEqual(exit_code, 1)
        self.assertIn("UNAVAILABLE blender: Blender was not found.", stdout.getvalue())
        self.assertIn("Remedy: Install Blender", stdout.getvalue())

    def test_unavailable_required_check_exits_one(self):
        report = {"checks": [doctor._check("ifc", "unavailable", "missing dependency")]}
        with mock.patch.object(doctor, "diagnose", return_value=report), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(doctor.main(["--json"]), 1)

    def test_deferred_checks_alone_exit_zero(self):
        report = {"checks": [doctor._check("compatibility", "deferred", "not checked")]}
        with mock.patch.object(doctor, "diagnose", return_value=report), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(doctor.main(["--json"]), 0)

    def test_missing_ifc_import_is_reported_unavailable(self):
        self._assert_ifc_import_failure(ImportError("No module named ifcopenshell"))

    def test_native_ifc_import_error_is_reported_unavailable(self):
        self._assert_ifc_import_failure(OSError("native library unavailable"))

    def _assert_ifc_import_failure(self, failure):
        original_import = builtins.__import__

        def scoped_import(name, globals=None, locals=None, fromlist=(), level=0):
            if name == "ifcopenshell" or name.startswith("ifcopenshell."):
                raise failure
            return original_import(name, globals, locals, fromlist, level)

        with mock.patch("builtins.__import__", side_effect=scoped_import):
            result = doctor._ifc_check()

        self.assertEqual(result["name"], "ifc")
        self.assertEqual(result["status"], "unavailable")
        self.assertIn("Install Archy's IFC extra", result["remedy"])


if __name__ == "__main__":
    unittest.main()
