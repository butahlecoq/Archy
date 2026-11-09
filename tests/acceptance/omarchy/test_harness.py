import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/validate-omarchy.py"


def run(*args, env=None):
    return subprocess.run([sys.executable, str(SCRIPT), *args], cwd=ROOT,
                          text=True, capture_output=True, check=False, env=env)


class HarnessTests(unittest.TestCase):
    def test_dry_run_enumerates_matrix_and_native_gates_without_pass(self):
        result = run("--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        ids = {gate["id"] for gate in report["gates"]}
        self.assertEqual(ids, {f"G{i:02}" for i in range(1, 12)}
                         | {f"C{i:02}" for i in range(1, 10)}
                         | {f"OM-A21-{i:02}" for i in range(1, 6)}
                         | {"OM-A22-01", "OM-A23-01"})
        self.assertEqual({gate["status"] for gate in report["gates"]}, {"deferred"})
        self.assertEqual(report["target_acceptance"], "deferred")
        self.assertEqual(report["probes"], [])
        self.assertIsNone(report["source_revision"])
        self.assertEqual(report["target_requirement"]["omarchy_version"], "4.0.6")
        guide = (ROOT / "docs/validation/omarchy-gates.md").read_text()
        for gate in report["gates"]:
            self.assertEqual(gate["procedure"], f"docs/validation/omarchy-gates.md#{gate['id'].lower()}")
            self.assertRegex(guide, rf"(?m)^### {re.escape(gate['id'])}$")

    def test_failure_and_unknown_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "observations.json"
            path.write_text(json.dumps({"results": {
                "G06": {"status": "fail", "reason": "GLB parse failed"},
                "G07": {"status": "unknown", "reason": "reader was unavailable"}}}))
            result = run("--dry-run", "--observations", str(path))
        self.assertEqual(result.returncode, 1, result.stderr)
        report = json.loads(result.stdout)
        gates = {gate["id"]: gate for gate in report["gates"]}
        self.assertEqual(gates["G06"]["status"], "fail")
        self.assertEqual(gates["G07"]["status"], "unknown")
        self.assertEqual(gates["G08"]["status"], "deferred")
        self.assertEqual(report["release_status"], "fail")

    def test_pass_and_agent_consent_claim_are_rejected(self):
        for payload in (
            {"results": {"G09": {"status": "pass", "reason": "agent says human approved"}}},
            {"results": {"G09": {"status": "deferred", "reason": "pending", "actor": "human"}}},
            {"results": {"G99": {"status": "fail", "reason": "invented"}}},
        ):
            with self.subTest(payload=payload), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "observations.json"
                path.write_text(json.dumps(payload))
                result = run("--observations", str(path))
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, "")

    def test_readonly_probe_keeps_acceptance_deferred(self):
        result = run("--probe-readonly")
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(len(report["probes"]), 4)
        self.assertEqual(report["target_acceptance"], "deferred")
        self.assertTrue(all(gate["status"] == "deferred" for gate in report["gates"]))
        self.assertEqual(report["probes"][1]["command"], ["/usr/bin/archy", "doctor", "--json"])
        self.assertTrue(all(item["status"] in ("observed", "unavailable") for item in report["probes"]))

    def test_hostile_path_does_not_execute_shims_or_forge_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            marker = folder / "executed"
            for name in ("git", "pacman", "omarchy-shell", "gpgv"):
                shim = folder / name
                shim.write_text(f"#!/bin/sh\necho {name} >> '{marker}'\necho FORGED\n")
                shim.chmod(0o755)
            env = {**os.environ, "PATH": f"{directory}:{os.environ['PATH']}"}
            for mode in ("--dry-run", "--probe-readonly"):
                with self.subTest(mode=mode):
                    result = run(mode, env=env)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    report = json.loads(result.stdout)
                    self.assertIsNone(report["source_revision"])
                    self.assertNotIn("FORGED", result.stdout)
                    self.assertFalse(marker.exists())

    def test_unsigned_review_cannot_promote_a_gate(self):
        with tempfile.TemporaryDirectory() as directory:
            record = Path(directory) / "review.json"
            signature = Path(directory) / "review.sig"
            record.write_text('{"results":{"G09":{"status":"pass","reason":"agent approved"}}}')
            signature.write_text("not a signature")
            for args in (("--reviewed-record", str(record)),
                         ("--reviewed-record", str(record), "--signature", str(signature))):
                result = run(*args)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, "")

    def test_unknown_without_failure_remains_unknown(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "observations.json"
            path.write_text(json.dumps({"results": {
                "G07": {"status": "unknown", "reason": "reader unavailable"}}}))
            result = run("--dry-run", "--observations", str(path))
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["release_status"], "unknown")
        self.assertEqual(report["target_acceptance"], "deferred")


if __name__ == "__main__":
    unittest.main()
