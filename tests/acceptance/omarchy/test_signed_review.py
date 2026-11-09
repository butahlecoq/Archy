"""Real signatures over private fixtures; native installation calls are modeled."""

import contextlib
from datetime import datetime, timezone
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[3] / "scripts/validate-omarchy.py"
spec = importlib.util.spec_from_file_location("omarchy_harness_signed", SCRIPT)
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class SignedReviewTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.home = self.base / "gnupg"
        self.home.mkdir(mode=0o700)
        self.env = {"GNUPGHOME": str(self.home), "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
        subprocess.run(["/usr/bin/gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                        "--quick-generate-key", "Test Reviewer <review@example.invalid>", "ed25519", "sign", "0"],
                       env=self.env, check=True, capture_output=True)
        keys = subprocess.run(["/usr/bin/gpg", "--with-colons", "--list-keys"], env=self.env,
                              check=True, capture_output=True, text=True).stdout
        self.fingerprint = next(line.split(":")[9] for line in keys.splitlines() if line.startswith("fpr:"))
        self.keyring = self.base / "reviewers.gpg"
        self.keyring.write_bytes(subprocess.run(["/usr/bin/gpg", "--export", self.fingerprint],
                                                env=self.env, check=True, capture_output=True).stdout)
        self.policy = self.base / "reviewers.json"
        self.policy.write_text(json.dumps({self.fingerprint: "authorized-reviewer"}))
        self.marker = self.base / "transaction.json"
        self.marker.write_text(json.dumps({"transaction_id": "11111111-1111-4111-8111-111111111111"}))
        self.root = self.base / "source"
        (self.root / "docs/specs").mkdir(parents=True)
        (self.root / "docs/validation").mkdir(parents=True)
        (self.root / "packaging/arch").mkdir(parents=True)
        (self.root / "docs/specs/release-0.1.txt").write_text("release spec")
        (self.root / "docs/validation/acceptance-matrix.md").write_text("matrix predicates")
        (self.root / "packaging/arch/compatibility.json").write_text("{}")
        self.archive = self.write("source.tar", "source archive")
        self.package = self.write("archy.pkg.tar", "package")
        self.lock = self.write("Cargo.lock", "lock")
        self.worker = self.write("blender-worker.tar", "worker")
        self.binary = self.write("installed-archy", "binary")
        self.installed = ["archy 0.1.0-1", "omarchy 4.0.6-1", "omarchy-dev 4.0.6-1"]
        self.target = {
            "candidate_revision": "a" * 40, "source_tree": "b" * 40,
            "source_archive": {"path": str(self.archive), "sha256": sha(self.archive)},
            "spec_sha256": sha(self.root / "docs/specs/release-0.1.txt"),
            "matrix_sha256": sha(self.root / "docs/validation/acceptance-matrix.md"),
            "compat_sha256": sha(self.root / "packaging/arch/compatibility.json"),
            "package": {"path": str(self.package), "sha256": sha(self.package)},
            "binary_sha256": sha(self.binary),
            "locks": {"cargo": {"path": str(self.lock), "sha256": sha(self.lock), "version": "0.1"}},
            "workers": {"blender": {"path": str(self.worker), "sha256": sha(self.worker), "version": "4.5.15"}},
            "installed_packages": list(self.installed),
            "transaction_id": "11111111-1111-4111-8111-111111111111",
            "transaction_sha256": sha(self.marker),
        }
        self.host = {"wsl": False, "container": False}

    def write(self, name, content):
        path = self.base / name
        path.write_text(content)
        return path

    def record(self):
        target_hash = hashlib.sha256(json.dumps(self.target, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        results = {}
        for gate, predicates in h.PREDICATES.items():
            artifact = self.write(f"{gate}.log", f"observed command and output for {gate}")
            manifest = {"gate": gate, "target_sha256": target_hash,
                        "artifacts": {artifact.name: sha(artifact)},
                        "checks": {name: {"status": "pass", "artifacts": [artifact.name]} for name in predicates}}
            manifest_path = self.write(f"{gate}.json", json.dumps(manifest))
            results[gate] = {"status": "pass", "reason": "human examined named check and artifact",
                             "manifest": {"name": manifest_path.name, "sha256": sha(manifest_path)},
                             "verified_predicates": {name: "reviewed named observation and output" for name in predicates}}
        return {"target": json.loads(json.dumps(self.target)), "machine": self.host, "reviewer": "authorized-reviewer",
                "reviewed_at_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                "independent_human_review": True, "decision": "approve", "results": results}

    def sign(self, data):
        path = self.write("review.json", json.dumps(data))
        signature = self.base / "review.sig"
        subprocess.run(["/usr/bin/gpg", "--batch", "--yes", "--pinentry-mode", "loopback",
                        "--passphrase", "", "--local-user", getattr(self, "signing_key", self.fingerprint),
                        "--detach-sign", "--output", str(signature), str(path)],
                       env=self.env, check=True, capture_output=True)
        return path, signature

    @contextlib.contextmanager
    def modeled_native(self):
        real_read = Path.read_text
        real_digest = h.digest

        def read(path, *args, **kwargs):
            return "ID=arch\n" if str(path) == "/etc/os-release" else real_read(path, *args, **kwargs)

        def digest(path):
            return sha(self.binary) if str(path) == "/usr/bin/archy" else real_digest(path)

        def run(argv, timeout=5, input_data=None, pass_fds=()):
            if argv[0] == "/usr/bin/gpgv":
                return subprocess.run(argv, input=input_data, capture_output=True,
                                      check=False, pass_fds=pass_fds)
            if argv[0] == "/usr/bin/pacman":
                return subprocess.CompletedProcess(argv, 0, "\n".join(self.installed) + "\n", "")
            if argv[0] == "/usr/bin/git":
                output = self.target["candidate_revision"] + "\n" + self.target["source_tree"] + "\n" if "rev-parse" in argv else ""
                return subprocess.CompletedProcess(argv, 0, output, "")
            raise AssertionError(argv)

        with mock.patch.object(h, "ROOT", self.root), mock.patch.object(h, "MATRIX", self.root / "docs/validation/acceptance-matrix.md"), mock.patch.object(h, "COMPAT", self.root / "packaging/arch/compatibility.json"), mock.patch.object(h, "REVIEW_KEYS", self.keyring), mock.patch.object(h, "REVIEW_POLICY", self.policy), mock.patch.object(h, "TRANSACTION", self.marker), mock.patch.object(h, "trusted_system_file", side_effect=lambda path: str(path)), mock.patch.object(h, "trusted_fd_check"), mock.patch.object(h, "read_trusted", side_effect=lambda path, limit: h.read_regular(path, limit)), mock.patch.object(h, "safe_run", side_effect=run), mock.patch.object(h, "digest", side_effect=digest), mock.patch.object(h, "native_host", return_value=self.host), mock.patch.object(Path, "read_text", read):
            yield

    def verify(self, data):
        record, signature = self.sign(data)
        with self.modeled_native():
            return h.reviewed_record(record, signature, self.host)

    def test_legitimate_signed_target_and_signer(self):
        reviewed, signer, _, _, primary = self.verify(self.record())
        self.assertEqual(signer, self.fingerprint)
        self.assertEqual(primary, self.fingerprint)
        self.assertEqual(reviewed["decision"], "approve")

    def test_legitimate_signed_target_report_path(self):
        record, signature = self.sign(self.record())
        compat = json.loads((SCRIPT.parents[1] / "packaging/arch/compatibility.json").read_text())
        output = io.StringIO()
        with self.modeled_native(), mock.patch.object(h, "source_check", return_value=compat), mock.patch.object(h, "native_host", return_value=self.host), mock.patch.object(sys, "argv", [str(SCRIPT), "--reviewed-record", str(record), "--signature", str(signature)]), redirect_stdout(output):
            code = h.main()
        report = json.loads(output.getvalue())
        self.assertEqual(code, 0)
        self.assertEqual(report["target_acceptance"], "pass")
        self.assertEqual(report["review"]["signer_fingerprint"], self.fingerprint)
        self.assertEqual(report["review"]["enrolled_primary_fingerprint"], self.fingerprint)
        self.assertEqual({gate["status"] for gate in report["gates"]}, {"pass"})

    def test_signed_forged_all_pass_single_text_evidence_rejected(self):
        data = self.record()
        artifact = self.write("invented.txt", "invented gate evidence")
        for item in data["results"].values():
            item["manifest"] = {"name": artifact.name, "sha256": sha(artifact)}
        with self.assertRaises((ValueError, json.JSONDecodeError)):
            self.verify(data)

    def test_reviewer_text_fingerprint_mismatch(self):
        data = self.record()
        data["reviewer"] = "Someone Other Than Key Owner"
        with self.assertRaisesRegex(ValueError, "reviewed identity"):
            self.verify(data)

    def test_replay_after_installed_revision_change(self):
        data = self.record()
        self.verify(data)
        self.installed[1] = "omarchy 4.0.6-2"
        with self.assertRaisesRegex(ValueError, "installed package revision changed"):
            self.verify(data)

    def test_target_and_evidence_changes_invalidate_record(self):
        data = self.record()
        self.verify(data)
        for path in (self.archive, self.package, self.lock, self.worker, self.marker,
                     self.root / "docs/specs/release-0.1.txt",
                     self.root / "docs/validation/acceptance-matrix.md", self.base / "G01.log"):
            with self.subTest(path=path):
                original = path.read_bytes()
                path.write_bytes(original + b"changed")
                with self.assertRaises(ValueError):
                    self.verify(data)
                path.write_bytes(original)

    def test_candidate_commit_and_binary_change_invalidate_record(self):
        data = self.record()
        self.verify(data)
        self.target["candidate_revision"] = "c" * 40
        with self.assertRaisesRegex(ValueError, "source commit/tree differs"):
            self.verify(data)
        self.target["candidate_revision"] = "a" * 40
        original = self.binary.read_bytes()
        self.binary.write_bytes(original + b"changed")
        with self.assertRaisesRegex(ValueError, "binary_sha256"):
            self.verify(data)

    def test_unchecked_predicate_and_agent_review_rejected(self):
        data = self.record()
        data["results"]["G01"]["verified_predicates"] = {"repeat": "reviewed repeat output"}
        with self.assertRaisesRegex(ValueError, "unverified required predicate"):
            self.verify(data)
        data = self.record()
        data["independent_human_review"] = False
        with self.assertRaisesRegex(ValueError, "reviewed identity"):
            self.verify(data)

    def test_matrix_predicate_drift_fails_source_inventory(self):
        original = h.MATRIX.read_text()
        changed = self.write("matrix-drift.md", original.replace(
            "Same brief, seed and pinned versions yield byte-identical",
            "Changed G01 predicate requires different evidence", 1))
        self.assertNotEqual(changed.read_text(), original)
        with mock.patch.object(h, "MATRIX", changed), self.assertRaisesRegex(ValueError, "matrix content changed"):
            h.source_check()

    def test_manifest_replaced_between_digest_and_parse_is_rejected(self):
        data = self.record()
        original = h.read_regular
        replaced = False
        def replace_after_read(path, limit, **kwargs):
            nonlocal replaced
            result = original(path, limit, **kwargs)
            if Path(path).name == "G01.json" and not replaced:
                replaced = True
                alternate = self.write("G01-alternate.log", "invented replacement")
                manifest = json.loads(result)
                manifest["artifacts"] = {alternate.name: sha(alternate)}
                for check in manifest["checks"].values():
                    check["artifacts"] = [alternate.name]
                Path(path).write_text(json.dumps(manifest))
            return result
        with mock.patch.object(h, "read_regular", side_effect=replace_after_read):
            with self.assertRaisesRegex(ValueError, "changed evidence"):
                self.verify(data)

    def test_subkey_signature_enrolls_primary_and_reports_signer(self):
        subprocess.run(["/usr/bin/gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                        "--quick-generate-key", "Subkey Reviewer <sub@example.invalid>", "ed25519", "cert", "0"],
                       env=self.env, check=True, capture_output=True)
        listing = subprocess.run(["/usr/bin/gpg", "--with-colons", "--list-keys", "Subkey Reviewer"],
                                 env=self.env, check=True, capture_output=True, text=True).stdout
        primary = next(line.split(":")[9] for line in listing.splitlines() if line.startswith("fpr:"))
        subprocess.run(["/usr/bin/gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                        "--quick-add-key", primary, "ed25519", "sign", "0"],
                       env=self.env, check=True, capture_output=True)
        listing = subprocess.run(["/usr/bin/gpg", "--with-colons", "--list-keys", "Subkey Reviewer"],
                                 env=self.env, check=True, capture_output=True, text=True).stdout
        subkey = [line.split(":")[9] for line in listing.splitlines() if line.startswith("fpr:")][-1]
        self.signing_key = subkey + "!"
        self.keyring.write_bytes(subprocess.run(["/usr/bin/gpg", "--export", primary],
                                                env=self.env, check=True, capture_output=True).stdout)
        self.policy.write_text(json.dumps({primary: "authorized-reviewer"}))
        reviewed, signer, _, _, enrolled = self.verify(self.record())
        self.assertEqual(reviewed["decision"], "approve")
        self.assertEqual(signer, subkey)
        self.assertEqual(enrolled, primary)
        self.policy.write_text(json.dumps({subkey: "authorized-reviewer"}))
        with self.assertRaisesRegex(ValueError, "signer is not an authorized named reviewer"):
            self.verify(self.record())

    def test_real_signature_mutation_and_wrong_primary_rejected(self):
        record, signature = self.sign(self.record())
        record.write_bytes(record.read_bytes() + b" ")
        with self.modeled_native(), self.assertRaisesRegex(ValueError, "signature verification failed"):
            h.reviewed_record(record, signature, self.host)
        record, signature = self.sign(self.record())
        self.policy.write_text(json.dumps({"A" * 40: "authorized-reviewer"}))
        with self.modeled_native(), self.assertRaisesRegex(ValueError, "signer is not an authorized"):
            h.reviewed_record(record, signature, self.host)

    def test_mutated_manifest_never_emits_acceptance_report(self):
        record, signature = self.sign(self.record())
        manifest = self.base / "G01.json"
        manifest.write_bytes(manifest.read_bytes() + b"changed")
        output = io.StringIO()
        with self.modeled_native(), mock.patch.object(h, "source_check", return_value={}), mock.patch.object(sys, "argv", [str(SCRIPT), "--reviewed-record", str(record), "--signature", str(signature)]), redirect_stdout(output), mock.patch.object(sys, "stderr", io.StringIO()):
            code = h.main()
        self.assertEqual(code, 2)
        self.assertEqual(output.getvalue(), "")

    def test_symlinks_in_evidence_signature_and_trust_inputs_rejected(self):
        data = self.record()
        record, signature = self.sign(data)
        original = self.base / "G01.json"
        replacement = self.base / "manifest-copy.json"
        replacement.write_bytes(original.read_bytes())
        original.unlink()
        original.symlink_to(replacement)
        with self.modeled_native(), self.assertRaises(OSError):
            h.reviewed_record(record, signature, self.host)
        original.unlink()
        original.write_bytes(replacement.read_bytes())
        for protected in (signature, self.keyring, self.policy, self.marker):
            with self.subTest(protected=protected.name):
                real = self.base / (protected.name + ".real")
                protected.rename(real)
                protected.symlink_to(real)
                try:
                    with self.modeled_native(), self.assertRaises((OSError, ValueError)):
                        h.reviewed_record(record, signature, self.host)
                finally:
                    protected.unlink()
                    real.rename(protected)

    def test_target_mutation_during_evidence_scan_fails_final_recheck(self):
        data = self.record()
        original = h.verify_evidence
        calls = 0
        def mutate_after_evidence(*args):
            nonlocal calls
            result = original(*args)
            calls += 1
            if calls == 1:
                self.package.write_bytes(self.package.read_bytes() + b"changed")
            return result
        with mock.patch.object(h, "verify_evidence", side_effect=mutate_after_evidence):
            with self.assertRaisesRegex(ValueError, "target file changed"):
                self.verify(data)

    def test_oversized_signature_and_keyring_rejected_before_gpgv(self):
        record, signature = self.sign(self.record())
        with signature.open("ab") as stream:
            stream.truncate(h.MAX_MANIFEST + 1)
        with self.modeled_native(), self.assertRaisesRegex(ValueError, "byte limit"):
            h.reviewed_record(record, signature, self.host)
        record, signature = self.sign(self.record())
        with self.keyring.open("ab") as stream:
            stream.truncate(h.MAX_RECORD + 1)
        with self.modeled_native(), self.assertRaisesRegex(ValueError, "byte limit"):
            h.reviewed_record(record, signature, self.host)


if __name__ == "__main__":
    unittest.main()
