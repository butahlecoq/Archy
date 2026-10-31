"""Read-only H4 status transport on immutable generic Quickshell, never Omarchy acceptance."""
import base64
import json
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[3]
HERE = pathlib.Path(__file__).parent
IMAGE = "sha256:c4050a4bcd4c1f640d8c77b9635bfa70f448da232f1e4b878ce2258c3998de05"
PUBLICATION_BINARY = pathlib.Path("/home/butahlecoq/work/archy/.tools/targets/reviewed-publication/debug/archy")
SELECTED = "/tmp/My Project/project.yaml"


def run_case(mode, real_binary=False):
    with tempfile.TemporaryDirectory(prefix="archy-desktop-qs-") as tmp:
        fixture = pathlib.Path(tmp)
        fixture.chmod(0o755)
        (fixture / "mode").write_text(mode)
        (fixture / "capabilities.json").write_bytes((ROOT / "tests/fixtures/desktop/capabilities.json").read_bytes())
        if mode == "cap-unavailable":
            capabilities = json.loads((fixture / "capabilities.json").read_text())
            capabilities["operations"][0]["available"] = False
            capabilities["operations"][0]["reason"] = "fixture unavailable"
            (fixture / "capabilities.json").write_text(json.dumps(capabilities))
        status = json.loads((ROOT / "tests/fixtures/desktop/accepted_status.json").read_text())
        if mode == "stale-cursor":
            status["next_cursor"] = "v1:" + "a" * 64 + ":5:" + "b" * 64 + ":20"
        if mode == "wrong-default":
            status["selected_run"]["summary"] = status["runs"][1]
        (fixture / "status.json").write_text(json.dumps(status, ensure_ascii=True))
        binary = PUBLICATION_BINARY if real_binary else fixture / "archy-desktop-fixture.sh"
        if not real_binary:
            binary.write_bytes((HERE / "fixtures/archy-desktop-fixture.sh").read_bytes())
            binary.chmod(0o755)
        cmd = [
            "docker", "run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL",
            "--security-opt", "no-new-privileges", "--user", "65534:65534",
            "--tmpfs", "/tmp:rw,nosuid,nodev,size=16m,mode=1777",
            "--mount", f"type=bind,src={ROOT},dst=/workspace,readonly",
            "--mount", f"type=bind,src={fixture},dst=/run/archy-desktop,readonly",
            "--mount", f"type=bind,src={binary},dst=/usr/bin/archy,readonly",
            "--env", f"ARCHY_DESKTOP_CASE={mode}", "--env", "QT_QPA_PLATFORM=offscreen",
            "--env", "QT_QUICK_BACKEND=software", "--env", "HOME=/tmp/home",
            "--env", "XDG_RUNTIME_DIR=/tmp/runtime", IMAGE, "/bin/sh", "-c",
            "mkdir -p /tmp/home /tmp/runtime /tmp/archy-qml '/tmp/My Project' '/tmp/Second Project'; chmod 700 /tmp/runtime; "
            "cp /workspace/examples/courtyard-housing/project.yaml '/tmp/My Project/project.yaml'; "
            "cp /workspace/examples/courtyard-housing/project.yaml '/tmp/Second Project/project.yaml'; "
            "cp /workspace/integrations/omarchy/plugin/DesktopProcess.qml /workspace/integrations/omarchy/plugin/DesktopModel.js /tmp/archy-qml/; "
            "cp /workspace/tests/omarchy/integration/headless/desktop-shell.qml /tmp/archy-qml/shell.qml; "
            "quickshell --no-color -p /tmp/archy-qml/shell.qml; result=$?; "
            "if [ -f /tmp/archy-desktop-late-reply ]; then printf 'ARCHY_DESKTOP_LATE_REPLY\n'; fi; "
            "if [ -f /tmp/archy-desktop-argv ]; then printf 'ARCHY_DESKTOP_ARGV '; base64 -w0 /tmp/archy-desktop-argv; printf '\n'; fi; exit $result",
        ]
        proc = subprocess.run(cmd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=12)
    records = [line.split("ARCHY_DESKTOP_RESULT ", 1)[1] for line in proc.stdout.splitlines() if "ARCHY_DESKTOP_RESULT " in line]
    logs = [line.split("ARCHY_DESKTOP_ARGV ", 1)[1] for line in proc.stdout.splitlines() if "ARCHY_DESKTOP_ARGV " in line]
    if proc.returncode or len(records) != 1:
        raise AssertionError(f"{mode}: exit={proc.returncode}, records={len(records)}\n{proc.stdout[-3000:]}")
    if mode == "invalid-selection-in-flight" and "ARCHY_DESKTOP_LATE_REPLY" not in proc.stdout.splitlines():
        raise AssertionError(f"{mode}: fixture did not emit late reply\n{proc.stdout[-3000:]}")
    args = base64.b64decode(logs[0]).decode().split("BEGIN\n")[1:] if logs else []
    return json.loads(records[0]), [a.strip("\n").split("\n") for a in args]


@unittest.skipUnless(os.environ.get("ARCHY_RUN_DESKTOP_QS") == "1", "set ARCHY_RUN_DESKTOP_QS=1 for immutable native container")
class NativeDesktop(unittest.TestCase):
    def test_invalid_project_selection_clears_previous_verified_run(self):
        result, argv = run_case("invalid-selection")
        self.assertEqual(argv, [
            ["desktop", "capabilities", "--json"],
            ["desktop", "status", "--project", SELECTED, "--limit", "20", "--json"],
        ])
        self.assertIsNone(result["status"])
        self.assertEqual(result["manifest"], "")
        self.assertEqual(result["selectedRunId"], "")
        self.assertTrue(result["error"])

    def test_invalid_project_selection_ignores_in_flight_status_reply(self):
        result, argv = run_case("invalid-selection-in-flight")
        self.assertEqual(argv, [
            ["desktop", "capabilities", "--json"],
            ["desktop", "status", "--project", SELECTED, "--limit", "20", "--json"],
            ["desktop", "status", "--project", SELECTED, "--limit", "20", "--json"],
        ])
        self.assertIsNone(result["status"])
        self.assertEqual(result["manifest"], "")
        self.assertEqual(result["selectedRunId"], "")
        self.assertEqual(result["pageCursor"], "")
        self.assertFalse(result["stale"])
        self.assertFalse(result["busy"])
        self.assertTrue(result["selectedWhileBusy"])
        self.assertTrue(result["inFlightSettled"])
        self.assertEqual(result["error"], "Enter an absolute local manifest path")

    def test_fixed_commands_and_path_with_spaces(self):
        result, argv = run_case("normal")
        self.assertEqual(argv[0], ["desktop", "capabilities", "--json"])
        self.assertEqual(argv[1], ["desktop", "status", "--project", SELECTED, "--limit", "20", "--json"])
        self.assertEqual(result["busyEvents"], [True, False, True, False])
        self.assertEqual(result["status"]["project"]["id"], "courtyard-demo")
        self.assertEqual(result["status"]["backend_jobs"]["status"], "unknown")
        self.assertEqual(result["status"]["selected_run"]["artifacts"][0]["integrity"], "verified")

    def test_selected_run_is_typed_argv(self):
        result, argv = run_case("selected-run")
        chosen = result["status"]["runs"][1]["run_id"]
        self.assertEqual(argv[2], ["desktop", "status", "--project", SELECTED, "--limit", "20", "--run", chosen, "--json"])
        self.assertEqual(result["status"]["selected_run"]["summary"]["run_id"], chosen)

    def test_mismatched_selected_run_never_replaces_verified_page(self):
        default, argv = run_case("wrong-default")
        self.assertEqual(len(argv), 2)
        self.assertIsNone(default["status"])
        self.assertTrue(default["error"])
        explicit, argv = run_case("wrong-explicit")
        self.assertEqual(len(argv), 3)
        self.assertIn("--run", argv[2])
        self.assertEqual(explicit["status"]["selected_run"]["summary"]["run_id"],
                         explicit["status"]["runs"][0]["run_id"])
        self.assertNotEqual(explicit["status"]["selected_run"]["summary"]["run_id"],
                            explicit["selectedRunId"])
        self.assertTrue(explicit["stale"])
        self.assertTrue(explicit["error"])

    def test_stale_cursor_refreshes_first_page(self):
        result, argv = run_case("stale-cursor")
        self.assertEqual(len(argv), 4)
        self.assertIn("--cursor", argv[2])
        self.assertNotIn("--cursor", argv[3])
        self.assertEqual(result["pageCursor"], "")
        self.assertFalse(result["stale"])
        self.assertIsNotNone(result["status"])

    def test_malformed_and_flood_fail_closed(self):
        for mode in ("malformed", "flood", "stderr"):
            with self.subTest(mode=mode):
                result, argv = run_case(mode)
                self.assertEqual(len(argv), 2)
                self.assertIsNone(result["status"])
                self.assertTrue(result["error"])

    def test_capability_gate_prevents_status_request(self):
        for mode in ("cap-unavailable", "cap-malformed"):
            with self.subTest(mode=mode):
                result, argv = run_case(mode)
                self.assertEqual(argv, [["desktop", "capabilities", "--json"]])
                self.assertIsNone(result["status"])
                self.assertTrue(result["error"])

    def test_manifest_change_discards_old_completion(self):
        result, argv = run_case("switch-manifest")
        self.assertEqual(len(argv), 3)
        self.assertEqual(argv[1][3], SELECTED)
        self.assertEqual(argv[2][3], "/tmp/Second Project/project.yaml")
        self.assertEqual(result["manifest"], "/tmp/Second Project/project.yaml")
        self.assertEqual(result["status"]["schema_version"], "archy-desktop-status-v1")
        self.assertEqual(result["busyEvents"], [True, False, True, False, True, False])


@unittest.skipUnless(os.environ.get("ARCHY_RUN_DESKTOP_REAL") == "1", "set ARCHY_RUN_DESKTOP_REAL=1 for frozen published CLI")
class PublishedDesktop(unittest.TestCase):
    def test_capabilities_and_genesis_status(self):
        result, _ = run_case("normal", real_binary=True)
        self.assertEqual(result["capabilities"]["schema_version"], "archy-desktop-capabilities-v1")
        self.assertEqual(result["status"]["schema_version"], "archy-desktop-status-v1")
        self.assertEqual(result["status"]["runs"], [])
        self.assertEqual(result["status"]["backend_jobs"]["status"], "unknown")
