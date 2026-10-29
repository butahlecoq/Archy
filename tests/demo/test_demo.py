import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "demo.py"
REVIEW_SHIM = Path(__file__).with_name("review_shim.py")
ARCHY = os.environ.get("ARCHY_BIN")


@unittest.skipUnless(ARCHY, "set ARCHY_BIN to the built real CLI")
class DemoCliTests(unittest.TestCase):
    def run_demo(self, directory, binary=None, extra=(), env=None):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--archy", str(binary or ARCHY),
             "--project-dir", str(directory), *extra],
            text=True, capture_output=True, check=False, env=env,
        )

    def review_probe(self, parent, mode=None, extra=()):
        project = Path(parent) / "demo"
        shim = Path(parent) / "archy-shim"
        shim.write_bytes(REVIEW_SHIM.read_bytes())
        shim.chmod(0o755)
        env = dict(os.environ, ARCHY_REAL_BIN=ARCHY)
        if mode:
            env["A18_REPRO"] = mode
        result = self.run_demo(project, shim, extra, env)
        return result, project

    def test_empty_backend_success_cannot_complete_draft_export(self):
        with tempfile.TemporaryDirectory() as parent:
            result, project = self.review_probe(parent, extra=(
                "--export-variant",
                "variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b",
            ))
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("DEMO STOPPED:", result.stderr)
            self.assertIn("no reviewed CLI receipt", result.stderr)
            self.assertNotIn("Draft export complete", result.stdout)
            self.assertNotIn("$ archy render --project", result.stdout)
            self.assertNotIn("$ archy export-ifc --project", result.stdout)
            self.assertEqual(list(project.rglob("*.glb")), [])
            self.assertEqual(list(project.rglob("*.png")), [])
            self.assertEqual(list(project.rglob("*.ifc")), [])
            self.assertEqual(len(list((project / "runs").iterdir())), 2)

    def test_malformed_evaluation_receipts_stop_cleanly(self):
        for mode in ("missing_checks", "wrong_checks_type", "duplicate_ids",
                     "wrong_ids", "same_run"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as parent:
                result, project = self.review_probe(parent, mode)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("DEMO STOPPED:", result.stderr)
                self.assertNotIn("Traceback", result.stderr)
                self.assertEqual(len(list((project / "runs").iterdir())), 2)

    def test_unavailable_render_keeps_real_core_runs_and_comparison(self):
        with tempfile.TemporaryDirectory() as parent:
            project = Path(parent) / "demo"
            result = self.run_demo(project)
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("UNAVAILABLE: archy render", result.stderr)
            self.assertIn("generate --project", result.stdout)
            self.assertIn("evaluate --project", result.stdout)
            self.assertIn("compare --project", result.stdout)
            self.assertTrue((project / "project.yaml").is_file())
            self.assertTrue((project / "comparison.md").is_file())
            self.assertTrue((project / "comparison.json").is_file())
            self.assertEqual(len(list((project / "runs").iterdir())), 2)

    def test_collision_preserves_existing_content(self):
        with tempfile.TemporaryDirectory() as parent:
            project = Path(parent) / "demo"
            project.mkdir()
            sentinel = project / "sentinel"
            sentinel.write_bytes(b"keep me")
            result = self.run_demo(project)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("already exists", result.stderr)
            self.assertEqual(sentinel.read_bytes(), b"keep me")
            self.assertEqual(sorted(p.name for p in project.iterdir()), ["sentinel"])

    def test_symlink_collision_preserves_target(self):
        with tempfile.TemporaryDirectory() as parent:
            target = Path(parent) / "target"
            target.mkdir()
            sentinel = target / "sentinel"
            sentinel.write_bytes(b"keep me")
            link = Path(parent) / "demo"
            link.symlink_to(target, target_is_directory=True)
            result = self.run_demo(link)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("already exists", result.stderr)
            self.assertTrue(link.is_symlink())
            self.assertEqual(sentinel.read_bytes(), b"keep me")

    def test_advertised_backend_without_reviewed_contract_keeps_accepted_runs(self):
        with tempfile.TemporaryDirectory() as parent:
            shim = Path(parent) / "archy-shim"
            shim.write_text(
                "#!/usr/bin/env python3\n"
                "import os, subprocess, sys\n"
                "args = sys.argv[1:]\n"
                "if args == ['--help']:\n"
                "    result = subprocess.run([os.environ['ARCHY_REAL_BIN'], *args])\n"
                "    print('  render  Simulated supervised backend')\n"
                "    sys.exit(result.returncode)\n"
                "if args == ['render', '--help']:\n"
                "    print('render --project --all --run --json')\n"
                "    sys.exit(0)\n"
                "if args and args[0] == 'render':\n"
                "    print('BACKEND_UNAVAILABLE: simulated worker', file=sys.stderr)\n"
                "    sys.exit(1)\n"
                "sys.exit(subprocess.run([os.environ['ARCHY_REAL_BIN'], *args]).returncode)\n",
                encoding="utf-8",
            )
            shim.chmod(0o755)
            project = Path(parent) / "demo"
            old = os.environ.get("ARCHY_REAL_BIN")
            os.environ["ARCHY_REAL_BIN"] = ARCHY
            try:
                result = self.run_demo(project, shim)
            finally:
                if old is None:
                    os.environ.pop("ARCHY_REAL_BIN", None)
                else:
                    os.environ["ARCHY_REAL_BIN"] = old
            self.assertEqual(result.returncode, 2)
            self.assertIn("no reviewed CLI receipt", result.stderr)
            self.assertNotIn("BACKEND_UNAVAILABLE: simulated worker", result.stderr)
            self.assertEqual(len(list((project / "runs").iterdir())), 2)
            self.assertTrue((project / "comparison.json").is_file())


if __name__ == "__main__":
    unittest.main()
