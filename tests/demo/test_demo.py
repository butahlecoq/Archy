import os
import json
import shlex
import subprocess
import sys
import tempfile
import unittest
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "demo.py"
REVIEW_SHIM = Path(__file__).with_name("review_shim.py")
ARCHY = os.environ.get("ARCHY_BIN")


class NativeStatusValidationTests(unittest.TestCase):
    def test_malformed_status_fields_stop_cleanly(self):
        spec = spec_from_file_location("archy_demo", SCRIPT)
        demo = module_from_spec(spec)
        spec.loader.exec_module(demo)
        summary = {"run_id": "run", "revision": 3, "operation": "render",
                   "variant_count": 3, "all_required_checks_passed": True,
                   "render_outcome": "verified"}
        for selected in (None, {"summary": None, "artifacts": []},
                         {"summary": {**summary, "all_required_checks_passed": "true"}, "artifacts": []},
                         {"summary": {k: v for k, v in summary.items()
                                      if k != "all_required_checks_passed"}, "artifacts": []},
                         {"summary": summary, "artifacts": [{"id": []}]}):
            with self.subTest(selected=selected), patch.object(
                    demo, "invoke", return_value=json.dumps({
                        "schema_version": "archy-desktop-status-v1", "selected_run": selected})):
                with self.assertRaises(demo.DemoStopped):
                    demo.accepted_render("archy", Path("project.yaml"),
                                         {"run_id": "run", "revision": 3}, Path("run"), [])


@unittest.skipUnless(ARCHY, "set ARCHY_BIN to the built real CLI")
class DemoCliTests(unittest.TestCase):
    def run_demo(self, directory, binary=None, extra=(), env=None):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--archy", str(binary or ARCHY),
             "--project-dir", str(directory), *extra],
            text=True, capture_output=True, check=False, env=env,
        )

    def review_probe(self, parent, mode="success", extra=()):
        project = Path(parent) / "demo"
        shim = Path(parent) / "archy-shim"
        shim.write_bytes(REVIEW_SHIM.read_bytes())
        shim.chmod(0o755)
        env = dict(os.environ, ARCHY_REAL_BIN=ARCHY, A18_REPRO=mode,
                   A18_EVAL_RECEIPT=str(Path(parent) / "evaluated.json"))
        return self.run_demo(project, shim, extra, env), project

    def test_forged_render_receipt_is_rejected_by_native_status(self):
        with tempfile.TemporaryDirectory() as parent:
            result, project = self.review_probe(parent)
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("ARTIFACT_INVALID", result.stdout)
            self.assertIn("run is not accepted", result.stdout)
            self.assertIn("$ archy render --project", result.stdout)
            self.assertNotIn("REVIEW READY:", result.stdout)
            self.assertNotIn("Verified artifact:", result.stdout)
            self.assertNotIn("$ archy export-ifc", result.stdout)
            self.assertEqual(len(list(project.rglob("*.glb"))), 3)
            self.assertEqual(len(list(project.rglob("*.png"))), 9)
            self.assertEqual(len(list((project / "runs").iterdir())), 3)
            self.assertTrue((project / "comparison.json").is_file())

    def test_malformed_native_status_stops_cleanly(self):
        with tempfile.TemporaryDirectory() as parent:
            result, _ = self.review_probe(parent, "status_null_summary")
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("DEMO STOPPED:", result.stderr)
            self.assertNotIn("Traceback", result.stderr)
            self.assertNotIn("REVIEW READY:", result.stdout)
            self.assertNotIn("Verified artifact:", result.stdout)

    def test_simulated_accepted_render_reaches_review_boundary(self):
        # The render bytes and status are simulated; this does not exercise Blender.
        with tempfile.TemporaryDirectory() as parent:
            candidate = "variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b"
            result, project = self.review_probe(parent, "simulated_ready",
                                                extra=("--export-variant", candidate))
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("REVIEW READY:", result.stdout)
            self.assertIn(f"Candidate for human review: {candidate}", result.stdout)
            self.assertEqual(result.stdout.count("Verified artifact:"), 18)
            self.assertTrue((project / "comparison.json").is_file())
            self.assertTrue((project / "comparison.md").is_file())
            self.assertEqual(list(project.rglob("*.ifc")), [])

    def test_simulated_status_with_wrong_artifact_hash_stops(self):
        with tempfile.TemporaryDirectory() as parent:
            result, _ = self.review_probe(parent, "status_bad_hash")
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertNotIn("REVIEW READY:", result.stdout)
            self.assertNotIn("Verified artifact:", result.stdout)

    def test_malformed_evaluation_receipts_stop_cleanly(self):
        for mode in ("missing_checks", "wrong_checks_type", "duplicate_ids",
                     "wrong_ids", "same_run"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as parent:
                result, project = self.review_probe(parent, mode)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("DEMO STOPPED:", result.stderr)
                self.assertNotIn("Traceback", result.stderr)
                self.assertEqual(len(list((project / "runs").iterdir())), 2)

    def test_unavailable_render_keeps_real_runs_receipt_and_comparison(self):
        if Path("/usr/lib/archy/archy-service-gate").exists():
            self.skipTest("installed gate present; this tests the absent-gate path")
        with tempfile.TemporaryDirectory() as parent:
            project = Path(parent) / "demo"
            result = self.run_demo(project)
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("BACKEND_UNAVAILABLE", result.stderr)
            self.assertNotIn("Verified artifact:", result.stdout)
            self.assertNotIn("REVIEW READY:", result.stdout)
            self.assertIn("generate --project", result.stdout)
            self.assertIn("evaluate --project", result.stdout)
            self.assertIn("render --project", result.stdout)
            self.assertIn("compare --project", result.stdout)
            self.assertTrue((project / "project.yaml").is_file())
            self.assertTrue((project / "comparison.md").is_file())
            self.assertTrue((project / "comparison.json").is_file())
            self.assertEqual(len(list((project / "runs").iterdir())), 3)
            self.assertEqual(len(list(project.rglob("*.glb"))), 0)
            self.assertEqual(len(list(project.rglob("*.png"))), 0)
            render_line = next(line for line in result.stdout.splitlines()
                               if line.startswith("$ archy render --project"))
            retry = subprocess.run([ARCHY, *shlex.split(render_line[2:])[1:]],
                                   text=True, capture_output=True, check=False)
            self.assertEqual(retry.returncode, 0, retry.stderr)
            self.assertEqual(json.loads(retry.stdout)["run_id"],
                             next(project.rglob("render.json")).parent.name)
            self.assertEqual(len(list((project / "runs").iterdir())), 3)

    def test_collision_preserves_existing_content(self):
        with tempfile.TemporaryDirectory() as parent:
            project = Path(parent) / "demo"
            project.mkdir()
            sentinel = project / "sentinel"
            sentinel.write_bytes(b"keep me")
            result = self.run_demo(project)
            self.assertEqual(result.returncode, 2)
            self.assertIn("already exists", result.stderr)
            self.assertEqual(sentinel.read_bytes(), b"keep me")

    def test_symlink_collision_preserves_target(self):
        with tempfile.TemporaryDirectory() as parent:
            target = Path(parent) / "target"
            target.mkdir()
            sentinel = target / "sentinel"
            sentinel.write_bytes(b"keep me")
            link = Path(parent) / "demo"
            link.symlink_to(target, target_is_directory=True)
            result = self.run_demo(link)
            self.assertEqual(result.returncode, 2)
            self.assertIn("already exists", result.stderr)
            self.assertTrue(link.is_symlink())
            self.assertEqual(sentinel.read_bytes(), b"keep me")

    def test_injected_bad_render_receipts_stop_without_success_claim(self):
        for mode in ("failed", "wrong_selector", "wrong_source", "wrong_revision",
                     "missing_artifact", "bad_digest", "wrong_verifier", "malformed",
                     "error_receipt"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as parent:
                result, project = self.review_probe(parent, mode)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("DEMO STOPPED:", result.stderr)
                self.assertNotIn("REVIEW READY:", result.stdout)
                self.assertNotIn("Verified artifact:", result.stdout)
                self.assertNotIn("Traceback", result.stderr)
                self.assertTrue((project / "project.yaml").is_file())

    def test_unknown_candidate_selector_stops_before_render(self):
        with tempfile.TemporaryDirectory() as parent:
            result, project = self.review_probe(parent, extra=("--export-variant", "unknown"))
            self.assertEqual(result.returncode, 2)
            self.assertIn("selected variant is absent", result.stderr)
            self.assertNotIn("$ archy render", result.stdout)
            self.assertEqual(len(list((project / "runs").iterdir())), 2)

    def test_known_candidate_selector_cannot_bypass_native_acceptance(self):
        with tempfile.TemporaryDirectory() as parent:
            candidate = ("variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b")
            result, project = self.review_probe(parent, extra=("--export-variant", candidate))
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("ARTIFACT_INVALID", result.stdout)
            self.assertNotIn("REVIEW READY:", result.stdout)
            self.assertNotIn(f"Candidate for human review: {candidate}", result.stdout)
            self.assertEqual(list(project.rglob("*.ifc")), [])


if __name__ == "__main__":
    unittest.main()
