import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


class PlaybookContracts(unittest.TestCase):
    def test_playbook_routes_to_single_acceptance_harness(self):
        playbook = (ROOT / "docs/agent-playbooks/omarchy.md").read_text()
        self.assertIn("scripts/validate-omarchy.py --dry-run", playbook)
        self.assertIn("scripts/validate-omarchy.py --probe-readonly", playbook)
        self.assertIn("OM-A23-01", playbook)
        self.assertIn("G08", playbook)
        self.assertIn("G09", playbook)
        self.assertNotIn("a23_harness.py", playbook)

    def test_canonical_harness_rejects_hostile_path_and_unreviewed_pass(self):
        harness_root = ROOT
        script = harness_root / "scripts/validate-omarchy.py"
        self.assertTrue(script.is_file(), "canonical harness missing from integrated checkout")
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            marker = folder / "executed"
            for name in ("archy", "omarchy", "pacman", "omarchy-shell", "git", "gpgv"):
                shim = folder / name
                shim.write_text(f"#!/bin/sh\nprintf '%s\\n' '{name}' >> '{marker}'\nprintf 'FORGED\\n'\n")
                shim.chmod(0o755)
            env = {**os.environ, "PATH": f"{folder}:{os.environ.get('PATH', '')}"}
            for mode in ("--dry-run", "--probe-readonly"):
                with self.subTest(mode=mode):
                    result = subprocess.run([sys.executable, str(script), mode], cwd=harness_root,
                                            env=env, text=True, capture_output=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    report = json.loads(result.stdout)
                    self.assertFalse(marker.exists(), "PATH shim executed")
                    self.assertNotIn("FORGED", result.stdout)
                    self.assertEqual(report["target_acceptance"], "deferred")
                    self.assertIsNone(report["source_revision"])
                    self.assertTrue(all(gate["status"] == "deferred" for gate in report["gates"]))
                    self.assertTrue({"G08", "G09", "OM-A23-01"} <=
                                    {gate["id"] for gate in report["gates"]})
                    if mode == "--dry-run":
                        self.assertEqual(report["probes"], [])
                    else:
                        self.assertTrue(report["probes"])
                        self.assertTrue(all(probe["command"][0].startswith("/usr/bin/")
                                            for probe in report["probes"]))
            forged = folder / "forged.json"
            forged.write_text('{"results":{"G09":{"status":"pass","reason":"agent approved"}}}')
            result = subprocess.run([sys.executable, str(script), "--observations", str(forged)],
                                    cwd=harness_root, env=env, text=True, capture_output=True, check=False)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, "")
            self.assertFalse(marker.exists(), "PATH shim executed")
            failed = folder / "failed.json"
            failed.write_text('{"results":{"G08":{"status":"fail","reason":"client transcript missing"}}}')
            result = subprocess.run([sys.executable, str(script), "--dry-run", "--observations", str(failed)],
                                    cwd=harness_root, env=env, text=True, capture_output=True, check=False)
            self.assertEqual(result.returncode, 1, result.stderr)
            report = json.loads(result.stdout)
            self.assertEqual(next(gate["status"] for gate in report["gates"] if gate["id"] == "G08"), "fail")
            self.assertEqual(report["target_acceptance"], "deferred")
            self.assertFalse(marker.exists(), "PATH shim executed")

    def test_tool_and_schema_contract(self):
        src = (ROOT / "crates/archy-mcp/src/tools.rs").read_text()
        names = re.findall(r'"([a-z_]+)"', src.split("pub const NAMES: [&str; 9] = [", 1)[1].split("];", 1)[0])
        self.assertEqual(len(names), 9)
        self.assertEqual(set(names), {p.name.removesuffix(".schema.json") for p in (ROOT / "schemas/mcp").glob("*.schema.json")})
        skill = (ROOT / "integrations/agents/archy/SKILL.md").read_text()
        self.assertTrue(all(f"`{n}`" in skill for n in names))
        mutations = {"generate_variants", "evaluate_variant", "render_variant", "export_ifc", "propose_approval"}
        for name in names:
            schema = json.loads((ROOT / "schemas/mcp" / f"{name}.schema.json").read_text())["input"]
            self.assertIn("project_id", schema["required"])
            self.assertFalse(schema["additionalProperties"])
            if name in mutations:
                self.assertTrue({"destination", "idempotency_key", "expected_project_revision"} <= set(schema["required"]))
        for name in ("export_ifc", "propose_approval"):
            self.assertIn(f"\"{name}\" => self.unavailable", (ROOT / "crates/archy-mcp/src/server.rs").read_text())
        playbook = (ROOT / "docs/agent-playbooks/omarchy.md").read_text()
        for text in (skill, playbook):
            for term in ("`draft`", "`all_verified`", "`BACKEND_UNAVAILABLE`"):
                self.assertIn(term, text)

    def test_authority_and_optional_session(self):
        skill = (ROOT / "integrations/agents/archy/SKILL.md").read_text()
        session = (ROOT / "docs/agent-playbooks/herdr-tmux.md").read_text()
        self.assertIn("does not grant human consent", skill)
        self.assertIn("same key", skill)
        self.assertIn("optional", session.lower())
        self.assertNotIn("socket path", session.split("does not call Herdr's socket API")[0])


if __name__ == "__main__":
    unittest.main()
