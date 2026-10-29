"""Generic Quickshell runtime contract; no Omarchy shell or host home involved."""
import json
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[3]
HERE = pathlib.Path(__file__).parent
IMAGE = "sha256:c4050a4bcd4c1f640d8c77b9635bfa70f448da232f1e4b878ce2258c3998de05"


def run_case(mode, qml_file="shell.qml"):
    with tempfile.TemporaryDirectory(prefix="archy-qs-") as tmp:
        fixture_dir = pathlib.Path(tmp)
        fixture_dir.chmod(0o755)  # Read-only bind must be traversable with dropped DAC capabilities.
        (fixture_dir / "mode").write_text(mode)
        (fixture_dir / "report.json").write_bytes((HERE / "fixtures/doctor-report.json").read_bytes())
        cmd = [
            "docker", "run", "--rm", "--network", "none", "--read-only",
            "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--user", "65534:65534",
            "--tmpfs", "/tmp:rw,nosuid,nodev,size=16m,mode=1777",
            "--mount", f"type=bind,src={ROOT},dst=/workspace,readonly",
            "--mount", f"type=bind,src={fixture_dir},dst=/run/archy-fixture,readonly",
        ]
        if mode == "real-doctor":
            cmd += ["--mount", f"type=bind,src={ROOT / 'target/debug/archy'},dst=/usr/bin/archy,readonly"]
        elif mode != "startfailure":
            cmd += ["--mount", f"type=bind,src={HERE / 'fixtures/archy-doctor-fixture.sh'},dst=/usr/bin/archy,readonly"]
        cmd += [
            "--env", f"ARCHY_HARNESS_CASE={mode}",
            "--env", "QT_QPA_PLATFORM=offscreen", "--env", "QT_QUICK_BACKEND=software",
            "--env", "HOME=/tmp/home", "--env", "XDG_RUNTIME_DIR=/tmp/runtime",
            IMAGE, "/bin/sh", "-c",
            f"mkdir -p /tmp/home /tmp/runtime /tmp/archy-qml; chmod 700 /tmp/runtime; cp /workspace/integrations/omarchy/plugin/DoctorProcess.qml /workspace/integrations/omarchy/plugin/DoctorModel.js /tmp/archy-qml/; cp /workspace/tests/omarchy/integration/headless/{qml_file} /tmp/archy-qml/{qml_file}; exec quickshell --no-color -p /tmp/archy-qml/{qml_file}",
        ]
        proc = subprocess.run(cmd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30)
    records = [line.split("ARCHY_HARNESS_RESULT ", 1)[1] for line in proc.stdout.splitlines() if "ARCHY_HARNESS_RESULT " in line]
    if proc.returncode or len(records) != 1:
        raise AssertionError(f"{mode}: exit={proc.returncode}, records={len(records)}\n{proc.stdout[-4000:]}")
    return json.loads(records[0])


@unittest.skipUnless(os.environ.get("ARCHY_RUN_HEADLESS_QS") == "1", "set ARCHY_RUN_HEADLESS_QS=1 for immutable container runtime")
class HeadlessDoctor(unittest.TestCase):
    def test_valid_reports_display_on_real_qml_component(self):
        for mode in ("valid0", "valid1"):
            with self.subTest(mode=mode):
                result = run_case(mode)
                self.assertEqual(result["starts"], 1)
                self.assertEqual(result["busyEvents"], [True, False])
                self.assertIsNotNone(result["report"], result["error"])
                self.assertEqual(result["report"]["schema_version"], "1")
                self.assertEqual(result["report"]["target_acceptance"], "deferred")

    def test_failures_unavailable(self):
        for mode in ("exit2", "crash", "malformed", "truncated", "extra-document", "stderr-small", "stderr-flood", "stdout-flood", "nonascii", "hang", "startfailure"):
            with self.subTest(mode=mode):
                result = run_case(mode)
                self.assertIsNone(result["report"])
                self.assertFalse(result["busy"])
                self.assertEqual(result["busyEvents"], [True, False])
                if mode in ("stderr-flood", "stdout-flood"):
                    self.assertIn("exceeded limit", result["error"])
                if mode == "hang":
                    self.assertIn("timed out", result["error"])
                if mode == "startfailure":
                    self.assertEqual(result["error"], "Doctor process unavailable")
                if mode == "exit2":
                    self.assertEqual(result["error"], "doctor unavailable")
                if mode == "stderr-small":
                    self.assertIn("stderr", result["error"])
                if mode == "nonascii":
                    self.assertIn("invalid encoding", result["error"])

    def test_close_reopen_runs_second_child(self):
        result = run_case("close-reopen")
        self.assertEqual(result["starts"], 2)
        self.assertEqual(result["busyEvents"], [True, False, True, False])
        self.assertNotIn("exceeded limit", result["error"])
        self.assertEqual(result["report"]["schema_version"], "1")
        self.assertFalse(result["stale"])
        self.assertNotEqual(result["refreshedAt"], "Never")

    def test_split_unicode_never_silently_changes_text(self):
        result = run_case("split-unicode")
        self.assertIsNone(result["report"])
        self.assertIn("invalid encoding", result["error"])

    def test_exact_profile_parser_order_and_unicode_loss(self):
        ascii_result = run_case("valid0", "ordering.qml")
        expected = (HERE / "fixtures/doctor-report.json").read_text()
        self.assertEqual(ascii_result["stdout"], expected)
        self.assertEqual(ascii_result["events"][-1], "exited")
        self.assertIn("stdout", ascii_result["events"][:-1])
        self.assertEqual(ascii_result["exitCode"], 0)
        unicode_result = run_case("split-unicode", "ordering.qml")
        self.assertEqual(unicode_result["events"][-1], "exited")
        self.assertIn("\ufffd", unicode_result["stdout"])
        self.assertNotIn("café", unicode_result["stdout"])


@unittest.skipUnless(os.environ.get("ARCHY_RUN_REAL_DOCTOR") == "1", "set ARCHY_RUN_REAL_DOCTOR=1 for read-only WSL-built binary probe")
class RealDoctorTransport(unittest.TestCase):
    def test_ascii_wire_and_observed_order(self):
        result = run_case("real-doctor", "ordering.qml")
        self.assertIn(result["exitCode"], (0, 1))
        self.assertEqual(result["events"][-1], "exited")
        self.assertIn("stdout", result["events"][:-1])
        self.assertEqual(result["stderrChars"], 0)
        self.assertTrue(result["stdout"].isascii())
        report = json.loads(result["stdout"])
        self.assertEqual(report["schema_version"], "1")
        self.assertEqual(report["target_acceptance"], "deferred")

    def test_actual_component_displays_valid_report(self):
        result = run_case("real-doctor")
        self.assertEqual(result["busyEvents"], [True, False])
        self.assertEqual(result["report"]["schema_version"], "1")
        self.assertEqual(result["report"]["target_acceptance"], "deferred")
