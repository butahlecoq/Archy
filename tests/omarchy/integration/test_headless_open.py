"""Generic Quickshell transport for the A22 opener; no Omarchy target claim."""
import json
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[3]
IMAGE = "sha256:c4050a4bcd4c1f640d8c77b9635bfa70f448da232f1e4b878ce2258c3998de05"


@unittest.skipUnless(os.environ.get("ARCHY_RUN_OPEN_QS") == "1", "set ARCHY_RUN_OPEN_QS=1 for native Quickshell")
class NativeOpen(unittest.TestCase):
    def test_fixed_selected_report_and_receipt(self):
        status = json.loads((ROOT / "tests/fixtures/desktop/accepted_status.json").read_text())
        run = status["selected_run"]["summary"]["run_id"]
        with tempfile.TemporaryDirectory(prefix="archy-open-qs-") as tmp:
            fixture = pathlib.Path(tmp)
            fixture.chmod(0o755)
            script = fixture / "archy"
            script.write_text("#!/bin/sh\nprintf 'BEGIN\\n' >> /tmp/archy-open-argv\nprintf '%s\\n' \"$@\" >> /tmp/archy-open-argv\ncat /run/archy-open/receipt.json\n")
            script.chmod(0o755)
            (fixture / "receipt.json").write_text(json.dumps({"schema_version": "archy-desktop-open-v1", "status": "handed_to_viewer",
                "run_id": run, "artifact_id": "report.md", "kind": "report", "sha256": next(a["sha256"] for a in status["selected_run"]["artifacts"] if a["id"] == "report.md")}))
            cmd = ["docker", "run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL",
                   "--security-opt", "no-new-privileges", "--user", "65534:65534",
                   "--tmpfs", "/tmp:rw,nosuid,nodev,size=16m,mode=1777",
                   "--mount", f"type=bind,src={ROOT},dst=/workspace,readonly",
                   "--mount", f"type=bind,src={fixture},dst=/run/archy-open,readonly",
                   "--mount", f"type=bind,src={script},dst=/usr/bin/archy,readonly",
                   "--env", "QT_QPA_PLATFORM=offscreen", "--env", "QT_QUICK_BACKEND=software",
                   "--env", "HOME=/tmp/home", "--env", "XDG_RUNTIME_DIR=/tmp/runtime",
                   "--env", "ARCHY_STATUS_JSON=" + json.dumps(status), IMAGE, "/bin/sh", "-c",
                   "mkdir -p /tmp/home /tmp/runtime /tmp/archy-qml; chmod 700 /tmp/runtime; "
                   "cp /workspace/integrations/omarchy/plugin/OpenProcess.qml /workspace/integrations/omarchy/plugin/OpenModel.js /workspace/integrations/omarchy/plugin/DesktopModel.js /tmp/archy-qml/; "
                   "cp /workspace/tests/omarchy/integration/headless/open-shell.qml /tmp/archy-qml/shell.qml; "
                   "quickshell --no-color -p /tmp/archy-qml/shell.qml; result=$?; "
                   "if [ -f /tmp/archy-open-argv ]; then cat /tmp/archy-open-argv; fi; exit $result"]
            proc = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
        output = proc.stdout + proc.stderr
        self.assertEqual(proc.returncode, 0, output[-3000:])
        self.assertIn('"message":"Verified artifact handed to viewer"', output)
        self.assertIn("desktop\nopen\n--project\n/tmp/My Project/project.yaml\n--run\n" + run + "\n--artifact\nreport.md\n--kind\nreport\n--json", output)


if __name__ == "__main__":
    unittest.main()
