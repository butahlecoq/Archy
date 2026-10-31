#!/usr/bin/env python3
"""Read-only inventory and evidence ledger for deferred Omarchy release gates."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import sys

MAX_RECORD = 8 * 1024 * 1024
MAX_MANIFEST = 1024 * 1024
MAX_ARTIFACT = 1024 * 1024 * 1024

ROOT = Path(__file__).resolve().parents[1]
MATRIX = ROOT / "docs/validation/acceptance-matrix.md"
COMPAT = ROOT / "packaging/arch/compatibility.json"
REVIEW_KEYS = Path("/etc/archy/acceptance-reviewers.gpg")
REVIEW_POLICY = Path("/etc/archy/acceptance-reviewers.json")
TRANSACTION = Path("/etc/archy/acceptance-transaction.json")
REVIEWED_MATRIX_SHA256 = "5c5f2dac322ac56d31702d4ca9d268ec26e686b79a32dc8231ff6808b200e0d7"
PROBE_COMMANDS = (("/usr/bin/pacman", "-Q", "omarchy", "omarchy-dev"),
                  ("/usr/bin/archy", "doctor", "--json"),
                  ("/usr/bin/archy", "doctor"),
                  ("/usr/bin/omarchy-shell", "shell", "ping"))

# The first twenty IDs come from the A20 matrix. OM IDs are the named native
# transactions in the A21 plan. A changed matrix must fail source validation.
GATES = [
    ("G01", "Reproducibility", "Repeat the full pinned demo twice; compare canonical manifests, metrics and measured per-view pixel differences."),
    ("G02", "Validity", "Run invalid dimensions, self-intersection, impossible setbacks and height cases; retain errors and UNKNOWN states."),
    ("G03", "Variation", "Run seed 42 demo; retain three distinct feasible footprints or bounded fewer-feasible explanation."),
    ("G04", "Measurement", "Compare analytic fixture values and parsed GLB/IFC bounds with recorded tolerances and units."),
    ("G05", "Artifact integrity", "Parse and hash every accepted run artifact; compare prior-run hashes after retries."),
    ("G06", "Blender on target", "Run pinned headless CPU Blender; parse three GLBs and at least one PNG each; separately verify nine demo views."),
    ("G07", "IFC", "Reopen candidate IFC4 with IfcOpenShell and a separate web-ifc reader; compare hierarchy, placement, quantities and bounds."),
    ("G08", "Agent autonomy", "Retain real stdio MCP client transcript for generate, evaluate, render and compare without manual 3D editing."),
    ("G09", "Human control", "Prove absent/forged agent consent cannot approve; retain a distinct actual human decision event and DRAFT preapproval export."),
    ("G10", "Resilience", "Exercise timeout, crash, malformed response, duplicate and cancellation; compare accepted-run hashes before/after."),
    ("G11", "Release aggregate", "Check G01-G10 and C01-C09, clean target demo, default authority, README, review and candidate identity."),
    ("C01", "Brief and provenance", "Validate YAML/JSON, SI normalization, field errors, source and authority snapshots."),
    ("C02", "Immutable iterations", "Edit from an accepted run; verify parent and original bytes persist and evaluation precedes rendering."),
    ("C03", "Typed workers and report", "Run bounded workers, cancellation, three views each and sourced comparison."),
    ("C04", "Schematic IFC mapping", "Check IFC4 typing, hierarchy, transforms, quantities and independent reader."),
    ("C05", "CLI and MCP surface", "Check all eight CLI commands and nine scoped MCP tools including revision, idempotency and destinations."),
    ("C06", "Errors and security", "Exercise stable errors, path confinement, denied home/network, limits and no agent code execution."),
    ("C07", "Install and demo", "Install frozen package on clean native target; run complete demo and record timing, hardware and sizes."),
    ("C08", "Distribution and review", "Audit exact locks/notices/licenses, candidate CI, protected branch and independent review."),
    ("C09", "Omarchy integration", "Complete doctor, package and desktop transaction gates on installed target."),
    ("OM-A21-01", "Clean native identity", "Record pacman -Q omarchy omarchy-dev, OS/kernel/arch and exact reviewed package identity on clean target."),
    ("OM-A21-02", "Installed doctor", "Run /usr/bin/archy doctor --json and text with stdout/stderr/exit and independent nonmutation trace."),
    ("OM-A21-03", "Pinned backend runtime", "Check Blender archive/build/bundled Python and isolated IFC Python/IfcOpenShell lock; run native smoke."),
    ("OM-A21-04", "Native containment", "Prove denied network/home, effective CPU/memory/tasks limits, timeout and whole-descendant cancellation."),
    ("OM-A21-05", "Install lifecycle", "On disposable target, preserve sentinels through install/upgrade/pacman removal and plugin uninstall conflict."),
    ("OM-A22-01", "Plugin and desktop", "Validate plugin, install, discover, enable/disable, summon/toggle/focus, theme, binding, viewer and screenshots."),
    ("OM-A23-01", "Golden target review", "Run clean CPU golden journey, independent IFC inspection and visual contact-sheet review; optional GPU separately."),
]

# Each item needs its own retained observation and an explicit human judgment.
# These names are a completeness contract, not automated semantic validation.
PREDICATES = {
    "G01": ("repeat", "pixels"), "G02": ("invalid_inputs", "unknown_state"),
    "G03": ("distinct_variants",), "G04": ("analytic_values", "artifact_bounds"),
    "G05": ("artifact_parse", "immutable_hashes"),
    "G06": ("cpu_glb", "nine_views", "visual_review"),
    "G07": ("ifc_reopen", "independent_reader"),
    "G08": ("real_client_transcript", "artifact_identity"),
    "G09": ("forged_consent_denied", "draft_export", "witnessed_human_event"),
    "G10": ("fault_matrix", "unchanged_accepted_runs"),
    "G11": ("all_required_gates", "candidate_release_review"),
    "C01": ("brief_and_authority",), "C02": ("immutable_edit",),
    "C03": ("typed_workers", "truthful_report"),
    "C04": ("schematic_ifc", "independent_reader"),
    "C05": ("cli_surface", "mcp_surface", "revision_guards"),
    "C06": ("security_fixtures", "native_containment"),
    "C07": ("clean_install", "full_demo"),
    "C08": ("candidate_checks", "distribution_review", "independent_review"),
    "C09": ("native_integration", "provider_neutral_session"),
    "OM-A21-01": ("clean_target", "installed_identity"),
    "OM-A21-02": ("doctor_outputs", "nonmutation"),
    "OM-A21-03": ("pinned_runtimes", "native_smoke"),
    "OM-A21-04": ("denied_access", "effective_limits", "descendant_cancellation"),
    "OM-A21-05": ("lifecycle", "sentinels"),
    "OM-A22-01": ("plugin_lifecycle", "desktop_visuals"),
    "OM-A23-01": ("cpu_journey", "independent_ifc", "human_visual_review"),
}


def source_check():
    text = MATRIX.read_text()
    if hashlib.sha256(text.encode()).hexdigest() != REVIEWED_MATRIX_SHA256:
        raise ValueError("matrix content changed; review predicates and update harness contract")
    expected = {f"G{n:02}" for n in range(1, 12)} | {f"C{n:02}" for n in range(1, 10)}
    actual = set(re.findall(r"^\| ((?:G|C)\d\d)\b", text, re.MULTILINE))
    listed = {gate[0] for gate in GATES}
    if actual != expected or listed & expected != expected or len(listed) != len(GATES):
        raise ValueError(f"matrix/inventory mismatch: matrix={sorted(actual)} inventory={sorted(listed)}")
    compat = json.loads(COMPAT.read_text())
    if set(PREDICATES) != listed or not all(PREDICATES.values()):
        raise ValueError("gate predicate inventory mismatch")
    if compat["omarchy"]["approved_upstream_version"] != "4.0.6":
        raise ValueError("Omarchy version contract changed; review target requirement")
    return compat


def trusted_system_file(path):
    """Reject caller-controlled executables and writable system path components."""
    if not path.is_absolute() or not str(path).startswith("/usr/bin/") and path not in (REVIEW_KEYS, REVIEW_POLICY, TRANSACTION):
        raise ValueError(f"not a fixed system path: {path}")
    for part in (path, *path.parents):
        info = part.lstat()
        if info.st_uid != 0 or info.st_mode & 0o022:
            raise ValueError(f"untrusted system path: {part}")
    resolved = path.resolve(strict=True)
    for part in (resolved, *resolved.parents):
        info = part.lstat()
        if info.st_uid != 0 or info.st_mode & 0o022:
            raise ValueError(f"untrusted resolved path: {part}")
    if not stat.S_ISREG(resolved.stat().st_mode):
        raise ValueError(f"not a regular system file: {path}")
    return str(resolved)


def safe_run(argv, timeout=5, input_data=None, pass_fds=()):
    executable = trusted_system_file(Path(argv[0]))
    return subprocess.run([executable, *argv[1:]], capture_output=True,
                          text=input_data is None, input=input_data,
                          timeout=timeout, check=False, pass_fds=pass_fds,
                          env={"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
                               "HOME": "/nonexistent", "XDG_CONFIG_HOME": "/nonexistent",
                               "MISE_AUTO_INSTALL": "false", "RUSTUP_AUTO_INSTALL": "0"})


def probe(argv):
    try:
        result = safe_run(argv)
        return {"command": argv, "status": "observed", "exit_code": result.returncode,
                "stdout": result.stdout[:65536], "stderr": result.stderr[:65536]}
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        return {"command": argv, "status": "unavailable", "exit_code": None,
                "error": type(error).__name__}


def native_host():
    os_release = Path("/etc/os-release").read_text() if Path("/etc/os-release").exists() else ""
    kernel = platform.release()
    cgroup = Path("/proc/1/cgroup").read_text() if Path("/proc/1/cgroup").exists() else ""
    return {"os_release_sha256": hashlib.sha256(os_release.encode()).hexdigest(),
            "kernel": kernel, "architecture": platform.machine(),
            "wsl": "microsoft" in kernel.lower() or "wsl" in kernel.lower(),
            "container": Path("/.dockerenv").exists() or Path("/run/.containerenv").exists()
            or bool(re.search(r"docker|podman|kubepods|containerd", cgroup, re.IGNORECASE)),
            "hostname": platform.node()}


def observations(path):
    if path is None:
        return {}
    data = json.loads(path.read_text())
    if not isinstance(data, dict) or set(data) != {"results"} or not isinstance(data["results"], dict):
        raise ValueError("observation file needs only a results object")
    known = {row[0] for row in GATES}
    if set(data["results"]) - known:
        raise ValueError("unknown gate ID in observations")
    for gate_id, item in data["results"].items():
        if (not isinstance(item, dict) or set(item) != {"status", "reason"}
                or item["status"] not in ("fail", "unknown", "deferred")
                or not isinstance(item["reason"], str) or not item["reason"].strip()):
            raise ValueError(f"invalid observation for {gate_id}; pass is never self-reported")
    return data["results"]


def digest(path):
    return hashlib.sha256(read_regular(path, MAX_ARTIFACT)).hexdigest()


def trusted_fd_check(fd):
    info = os.fstat(fd)
    if info.st_uid != 0 or info.st_mode & 0o022:
        raise ValueError("untrusted opened system path component")


def open_regular_fd(path, *, trusted=False, max_size=None):
    """Walk from / using openat; reject symlinks at every component."""
    path = Path(path)
    if not path.is_absolute() or not path.parts[1:]:
        raise ValueError("expected absolute regular file path")
    if trusted:
        trusted_system_file(path)
    directory = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        if trusted:
            trusted_fd_check(directory)
        for component in path.parts[1:-1]:
            next_dir = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                               dir_fd=directory)
            os.close(directory)
            directory = next_dir
            if trusted:
                trusted_fd_check(directory)
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=directory)
        try:
            info = os.fstat(fd)
            if not stat.S_ISREG(info.st_mode):
                raise ValueError(f"not a regular file: {path}")
            if max_size is not None and info.st_size > max_size:
                raise ValueError(f"file exceeds byte limit: {path}")
            if trusted:
                trusted_fd_check(fd)
            return fd
        except BaseException:
            os.close(fd)
            raise
    finally:
        os.close(directory)


def read_regular(path, limit, *, trusted=False):
    fd = open_regular_fd(path, trusted=trusted)
    try:
        before = os.fstat(fd)
        if before.st_size > limit:
            raise ValueError(f"file exceeds byte limit: {path}")
        chunks = []
        size = 0
        while True:
            chunk = os.read(fd, min(1024 * 1024, limit + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > limit:
                raise ValueError(f"file exceeds byte limit: {path}")
            chunks.append(chunk)
        after = os.fstat(fd)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise ValueError(f"file changed during read: {path}")
        return b"".join(chunks)
    finally:
        os.close(fd)


def read_trusted(path, limit):
    return read_regular(path, limit, trusted=True)


def evidence_file(base, name, expected, *, limit=MAX_ARTIFACT):
    if (not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", name)
            or not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected)):
        raise ValueError("invalid evidence reference")
    item = base / name
    data = read_regular(item, limit)
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError(f"missing or changed evidence: {name}")
    return data


def target_file(name, expected):
    if not isinstance(name, str) or not Path(name).is_absolute() or not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValueError("invalid target file identity")
    path = Path(name)
    if digest(path) != expected:
        raise ValueError(f"target file changed: {name}")


def current_target(target, installed):
    required = {"candidate_revision", "source_tree", "source_archive", "spec_sha256", "matrix_sha256",
                "compat_sha256", "package", "binary_sha256", "locks", "workers", "installed_packages",
                "transaction_id", "transaction_sha256"}
    if not isinstance(target, dict) or set(target) != required:
        raise ValueError("incomplete target transaction")
    if not re.fullmatch(r"[0-9a-f]{40}", target["candidate_revision"]) or not re.fullmatch(r"[0-9a-f]{40}", target["source_tree"]):
        raise ValueError("invalid source identity")
    for field, path in (("spec_sha256", ROOT / "docs/specs/release-0.1.txt"),
                        ("matrix_sha256", MATRIX), ("compat_sha256", COMPAT),
                        ("binary_sha256", Path("/usr/bin/archy"))):
        if digest(path) != target[field]:
            raise ValueError(f"current {field} differs from reviewed target")
    git = safe_run(["/usr/bin/git", "-C", str(ROOT), "rev-parse", "HEAD", "HEAD^{tree}"])
    if git.returncode or git.stdout.splitlines() != [target["candidate_revision"], target["source_tree"]]:
        raise ValueError("source commit/tree differs from reviewed target")
    dirty = safe_run(["/usr/bin/git", "-C", str(ROOT), "status", "--porcelain", "--untracked-files=no"])
    if dirty.returncode or dirty.stdout.strip():
        raise ValueError("source checkout differs from reviewed commit")
    for field in ("source_archive", "package"):
        item = target[field]
        if not isinstance(item, dict) or set(item) != {"path", "sha256"}:
            raise ValueError(f"invalid {field} identity")
        target_file(item["path"], item["sha256"])
    for field in ("locks", "workers"):
        items = target[field]
        if not isinstance(items, dict) or not items:
            raise ValueError(f"missing {field} identity")
        for name, item in items.items():
            if not isinstance(name, str) or not isinstance(item, dict) or set(item) != {"path", "sha256", "version"} or not isinstance(item["version"], str) or not item["version"].strip():
                raise ValueError(f"invalid {field} identity")
            target_file(item["path"], item["sha256"])
    if target["installed_packages"] != installed:
        raise ValueError("installed package revision changed")
    marker_bytes = read_trusted(TRANSACTION, MAX_MANIFEST)
    marker = json.loads(marker_bytes)
    if not isinstance(marker, dict) or set(marker) != {"transaction_id"} or not re.fullmatch(r"[0-9a-f-]{36}", marker["transaction_id"]):
        raise ValueError("invalid administrator transaction marker")
    if marker["transaction_id"] != target["transaction_id"] or hashlib.sha256(marker_bytes).hexdigest() != target["transaction_sha256"]:
        raise ValueError("target installation transaction changed")


def verify_evidence(path, data, target_hash):
    used = set()
    for gate_id, item in data["results"].items():
        required = set(PREDICATES[gate_id])
        if (not isinstance(item, dict) or set(item) != {"status", "reason", "manifest", "verified_predicates"}
                or item["status"] not in ("pass", "fail", "unknown", "deferred")
                or not isinstance(item["reason"], str) or not item["reason"].strip()
                or not isinstance(item["manifest"], dict) or set(item["manifest"]) != {"name", "sha256"}
                or not isinstance(item["verified_predicates"], dict)
                or set(item["verified_predicates"]) - required
                or any(not isinstance(reason, str) or not reason.strip()
                       for reason in item["verified_predicates"].values())):
            raise ValueError(f"invalid reviewed decision for {gate_id}")
        manifest_name = item["manifest"]["name"]
        if manifest_name in used:
            raise ValueError("gate manifests must be distinct")
        used.add(manifest_name)
        manifest = json.loads(evidence_file(path.parent, manifest_name, item["manifest"]["sha256"], limit=MAX_MANIFEST))
        if not isinstance(manifest, dict) or set(manifest) != {"gate", "target_sha256", "checks", "artifacts"} or manifest["gate"] != gate_id or manifest["target_sha256"] != target_hash:
            raise ValueError(f"manifest target/gate mismatch for {gate_id}")
        artifacts = manifest["artifacts"]
        if not isinstance(artifacts, dict) or not artifacts or not isinstance(manifest["checks"], dict) or set(manifest["checks"]) != required:
            raise ValueError(f"incomplete evidence manifest for {gate_id}")
        for name, sha in artifacts.items():
            if name in used:
                raise ValueError("evidence artifacts must be distinct per gate")
            used.add(name)
            evidence_file(path.parent, name, sha)
        for predicate, check in manifest["checks"].items():
            if (not isinstance(check, dict) or set(check) != {"status", "artifacts"}
                    or check["status"] not in ("pass", "fail", "unknown", "deferred")
                    or not isinstance(check["artifacts"], list) or not check["artifacts"]
                    or not set(check["artifacts"]) <= set(artifacts)):
                raise ValueError(f"invalid evidence check for {gate_id}/{predicate}")
        if item["status"] == "pass" and (set(item["verified_predicates"]) != required or any(
                check["status"] != "pass" for check in manifest["checks"].values())):
            raise ValueError(f"unverified required predicate for {gate_id}")


def reviewed_record(path, signature, host):
    """Load only a detached signature verified against administrator-owned reviewer keys."""
    if host["wsl"] or host["container"] or platform.machine() != "x86_64":
        raise ValueError("reviewed target record requires native x86_64 Omarchy")
    os_release = Path("/etc/os-release").read_text()
    if not re.search(r'^ID=arch$', os_release, re.MULTILINE):
        raise ValueError("reviewed target record requires Arch OS identity")
    installed = safe_run(["/usr/bin/pacman", "-Q", "archy", "omarchy", "omarchy-dev"])
    if installed.returncode != 0 or not re.search(r"^omarchy 4\.0\.6(?:-|$)", installed.stdout, re.MULTILINE):
        raise ValueError("reviewed target record requires installed Omarchy 4.0.6 packages")
    signed_bytes = read_regular(path, MAX_RECORD)
    signature_fd = open_regular_fd(signature, max_size=MAX_MANIFEST)
    try:
        keyring_fd = open_regular_fd(REVIEW_KEYS, trusted=True, max_size=MAX_RECORD)
        try:
            signature_before = os.fstat(signature_fd)
            keyring_before = os.fstat(keyring_fd)
            result = safe_run(["/usr/bin/gpgv", "--status-fd", "1", "--keyring",
                               f"/proc/self/fd/{keyring_fd}", f"/proc/self/fd/{signature_fd}", "-"],
                              input_data=signed_bytes, pass_fds=(keyring_fd, signature_fd))
            for before, fd in ((signature_before, signature_fd), (keyring_before, keyring_fd)):
                after = os.fstat(fd)
                if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                    raise ValueError("signature or keyring changed during verification")
        finally:
            os.close(keyring_fd)
    finally:
        os.close(signature_fd)
    if result.returncode != 0:
        raise ValueError("reviewer signature verification failed")
    statuses = [line.split() for line in result.stdout.splitlines()
                if line.startswith(b"[GNUPG:] VALIDSIG ")]
    if len(statuses) != 1 or len(statuses[0]) != 12:
        raise ValueError("reviewer signature identity unavailable")
    signer, primary = statuses[0][2], statuses[0][11]
    if not all(re.fullmatch(rb"[0-9A-F]{40}|[0-9A-F]{64}", fp) for fp in (signer, primary)):
        raise ValueError("reviewer signature identity unavailable")
    fingerprint = signer.decode()
    enrolled = primary.decode()
    policy = json.loads(read_trusted(REVIEW_POLICY, MAX_MANIFEST))
    if (not isinstance(policy, dict) or any(not isinstance(key, str) or not re.fullmatch(r"[0-9A-F]{40}|[0-9A-F]{64}", key)
                                            or not isinstance(value, str) or not value.strip()
                                            for key, value in policy.items()) or enrolled not in policy
            or not isinstance(policy[enrolled], str) or not policy[enrolled].strip()):
        raise ValueError("signer is not an authorized named reviewer")
    data = json.loads(signed_bytes)
    if not isinstance(data, dict) or set(data) != {"target", "machine", "reviewer", "reviewed_at_utc",
                     "decision", "independent_human_review", "results"}:
        raise ValueError("invalid reviewed record identity")
    if (data["machine"] != host or data["reviewer"] != policy[enrolled]
            or data["decision"] not in ("approve", "reject")
            or data["independent_human_review"] is not True):
        raise ValueError("reviewed identity, machine, or decision mismatch")
    reviewed_at = datetime.strptime(data["reviewed_at_utc"], "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    if reviewed_at > datetime.now(timezone.utc):
        raise ValueError("review timestamp is in the future")
    current_target(data["target"], installed.stdout.splitlines())
    if not isinstance(data["results"], dict) or set(data["results"]) != {row[0] for row in GATES}:
        raise ValueError("reviewed record must decide every required gate")
    target_hash = hashlib.sha256(json.dumps(data["target"], sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    verify_evidence(path, data, target_hash)
    # A final observation closes mutations during the evidence scan. This is a
    # point-in-time check, not an atomic snapshot of independent OS resources.
    verify_evidence(path, data, target_hash)
    installed_final = safe_run(["/usr/bin/pacman", "-Q", "archy", "omarchy", "omarchy-dev"])
    if installed_final.returncode:
        raise ValueError("final installed package query failed")
    current_target(data["target"], installed_final.stdout.splitlines())
    if native_host() != host:
        raise ValueError("target host identity changed during verification")
    return data, fingerprint, hashlib.sha256(signed_bytes).hexdigest(), target_hash, enrolled


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="enumerate gates; execute no probe")
    parser.add_argument("--probe-readonly", action="store_true", help="run bounded identity and doctor probes only")
    parser.add_argument("--observations", type=Path, help="JSON negative/deferred observations; never accepts pass")
    parser.add_argument("--reviewed-record", type=Path, help="human-reviewed, detached-signed native evidence record")
    parser.add_argument("--signature", type=Path, help="detached signature for --reviewed-record")
    args = parser.parse_args()
    if args.dry_run and args.probe_readonly:
        parser.error("choose dry-run or probe-readonly")
    if bool(args.reviewed_record) != bool(args.signature) or args.reviewed_record and (args.observations or args.probe_readonly or args.dry_run):
        parser.error("reviewed record requires signature and cannot combine with other modes")
    try:
        compat = source_check()
        observed = observations(args.observations)
        host = native_host()
        reviewed_info = reviewed_record(args.reviewed_record, args.signature, host) if args.reviewed_record else None
    except (OSError, ValueError, KeyError, TypeError, UnicodeError, json.JSONDecodeError, subprocess.TimeoutExpired) as error:
        print(f"harness input/source error: {error}", file=sys.stderr)
        return 2
    probes = []
    reviewed = reviewed_info[0] if reviewed_info else None
    if args.probe_readonly:
        for command in PROBE_COMMANDS:
            probes.append(probe(command))
    gates = []
    for gate_id, title, action in GATES:
        item = (reviewed["results"] if reviewed else observed).get(gate_id, {"status": "deferred", "reason": "target evidence not recorded"})
        gates.append({"id": gate_id, "title": title, "status": item["status"],
                      "reason": item["reason"], "required": True, "target_action": action,
                      "procedure": f"docs/validation/omarchy-gates.md#{gate_id.lower()}",
                      "issues": ["A19", "A20"]})
    statuses = {g["status"] for g in gates}
    release_status = ("fail" if "fail" in statuses or reviewed and reviewed["decision"] == "reject"
                      else "pass" if reviewed and statuses == {"pass"}
                      else "unknown" if "unknown" in statuses else "deferred")
    report = {"schema_version": 1, "mode": "reviewed" if reviewed else "probe-readonly" if args.probe_readonly else "dry-run",
              "source_revision": reviewed["target"]["candidate_revision"] if reviewed else None,
              "source_revision_status": "signed and checked against current source tree" if reviewed else "not verified by inventory",
              "review": {"record_sha256": reviewed_info[2], "signer_fingerprint": reviewed_info[1],
                         "enrolled_primary_fingerprint": reviewed_info[4],
                         "decision": reviewed["decision"], "target_sha256": reviewed_info[3],
                         "reviewer": reviewed["reviewer"]} if reviewed else None,
              "target_requirement": {"platform": "installed native Omarchy/Arch Linux, not WSL/CI/container",
                                     "omarchy_version": compat["omarchy"]["approved_upstream_version"],
                                     "architecture": "x86_64 reviewed package profile",
                                     "blender_version": compat["backends"]["blender"]["proposed_version"],
                                     "python_ifc_version": compat["backends"]["ifc"]["proposed_python"],
                                     "ifcopenshell_version": compat["backends"]["ifc"]["proposed_ifcopenshell"]},
              "observed_host": host, "probes": probes, "gates": gates,
              "release_status": release_status,
              "target_acceptance": release_status if reviewed else "deferred",
              "note": ("Cryptographically verified reviewer attestation, current target identity and evidence bytes. Gate semantics depend on the independent human review; manifest observations are claims, not automated proof."
                       if reviewed else "Inventory/observations only; no pass or human approval is inferred. GPU and visual review require separate retained target evidence.")}
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if report["release_status"] == "fail" else 0


if __name__ == "__main__":
    sys.exit(main())
