use archy_authority::hex_digest;
use archy_authority::protected_project::ProtectedProject;
use archy_core::ProjectStore;
use archy_core::decision_contract::{
    ConstraintEvidence, PROPOSAL_SCHEMA_VERSION, Proposal, ReviewEvidence,
};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    base: PathBuf,
    root: PathBuf,
    project: PathBuf,
    proposal: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        Self::new_with_source(include_bytes!("../../tests/fixtures/briefs/courtyard.json"))
    }

    fn new_with_source(source_bytes: &[u8]) -> Self {
        let base = PathBuf::from(std::env::var_os("HOME").unwrap()).join(format!(
            "archy-protected-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        let root = base.join("project");
        let project = root.join("project.json");
        let receipt = ProtectedProject::initialize(&base, unsafe { libc::geteuid() }, source_bytes)
            .unwrap()
            .generate_accepted(3, 42, "fixture-generated-run".into(), 0)
            .unwrap();
        let store = ProjectStore::open(&project).unwrap();
        let history = store.read_history().unwrap();
        let run = archy_cli::verify_history(&history, &receipt.run_id).unwrap();
        let source = history.read_run(&receipt.run_id).unwrap();
        let selected = run
            .variants()
            .iter()
            .zip(run.evaluations())
            .find(|(_, e)| e.required_checks_passed)
            .unwrap();
        let variant = selected.0;
        let evaluation = selected.1;
        let variant_bytes = source
            .artifact(&format!("variants/{}.json", variant.id))
            .unwrap();
        let evaluation_bytes = source
            .artifact(&format!("evaluations/{}.json", variant.id))
            .unwrap();
        let manifest_hash = history
            .receipts()
            .find(|r| r.run_id == receipt.run_id)
            .unwrap()
            .manifest_sha256
            .clone();
        let comparison = archy_cli::render_verified_comparison(
            &run,
            &manifest_hash,
            history.revision(),
            history.head_sha256().map(str::to_owned),
        )
        .unwrap();
        let normalized = source.artifact("normalized.json").unwrap();
        let proposal = Proposal {
            schema_version: PROPOSAL_SCHEMA_VERSION.into(),
            evidence: ReviewEvidence {
                project_id: run.spec().project_id.clone(),
                variant_id: variant.id.clone(),
                source_run_id: receipt.run_id,
                expected_project_revision: history.revision(),
                expected_head_sha256: history.head_sha256().unwrap().into(),
                input_sha256: hex_digest(run.input()),
                normalized_sha256: hex_digest(normalized),
                variant_sha256: hex_digest(variant_bytes),
                evaluation_sha256: hex_digest(evaluation_bytes),
                comparison_sha256: hex_digest(&comparison.json),
                constraints: run
                    .spec()
                    .constraints
                    .iter()
                    .map(ConstraintEvidence::from_normalized)
                    .collect(),
                checks: evaluation.checks.clone(),
            },
            suggested_reason: "Review candidate".into(),
        };
        Self {
            base,
            root,
            project,
            proposal: serde_json::to_vec(&proposal).unwrap(),
        }
    }
    fn open(&self) -> ProtectedProject {
        ProtectedProject::open(&self.project, unsafe { libc::geteuid() }).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}

#[test]
fn real_generated_history_produces_exact_bounded_review_snapshot() {
    let f = Fixture::new();
    let snapshot = f.open().snapshot(&f.proposal).unwrap();
    assert_eq!(snapshot.digest(), hex_digest(snapshot.bytes()));
    assert_eq!(snapshot.revision(), 1);
    assert_eq!(snapshot.head().len(), 64);
    let display: serde_json::Value = serde_json::from_slice(snapshot.bytes()).unwrap();
    assert_eq!(display["schema_version"], "archy-broker-review-v1");
    assert_eq!(display["input_provenance"], "accepted_run_artifact");
    assert_eq!(display["current_source_differs_from_run"], false);
    assert_eq!(display["proposal_sha256"], hex_digest(&f.proposal));
    let comparison: serde_json::Value =
        serde_json::from_str(display["comparison_json"].as_str().unwrap()).unwrap();
    assert_eq!(comparison["source_run_id"], display["source_run_id"]);
    assert_eq!(
        hex_digest(display["comparison_json"].as_str().unwrap().as_bytes()),
        display["comparison_sha256"].as_str().unwrap()
    );
    assert!(
        display["input_utf8"]
            .as_str()
            .unwrap()
            .contains("courtyard-demo")
    );
}

#[test]
fn self_asserted_and_stale_evidence_cannot_form_a_snapshot() {
    let f = Fixture::new();
    let project = f.open();
    for key in [
        "expected_head_sha256",
        "variant_sha256",
        "evaluation_sha256",
        "comparison_sha256",
    ] {
        let mut claim: serde_json::Value = serde_json::from_slice(&f.proposal).unwrap();
        claim["evidence"][key] = "a".repeat(64).into();
        assert!(
            project
                .snapshot(&serde_json::to_vec(&claim).unwrap())
                .is_err(),
            "{key}"
        );
    }
    let mut claim: serde_json::Value = serde_json::from_slice(&f.proposal).unwrap();
    claim["evidence"]["source_run_id"] = "00000000-0000-4000-8000-000000000000".into();
    assert!(
        project
            .snapshot(&serde_json::to_vec(&claim).unwrap())
            .is_err()
    );
    let mut claim: serde_json::Value = serde_json::from_slice(&f.proposal).unwrap();
    claim["evidence"]["checks"][0]["explanation"] = "forged PASS".into();
    assert!(
        project
            .snapshot(&serde_json::to_vec(&claim).unwrap())
            .is_err()
    );
}

#[test]
fn agent_writable_or_swapped_root_is_rejected_even_for_real_evidence() {
    let f = Fixture::new();
    let uid = unsafe { libc::geteuid() };
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(ProtectedProject::open(&f.project, uid).is_err());
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
    let protected = f.open();
    fs::set_permissions(&f.project, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(protected.snapshot(&f.proposal).is_err());
    fs::set_permissions(&f.project, fs::Permissions::from_mode(0o600)).unwrap();
    let link = f.root.join("linked.json");
    symlink(&f.project, &link).unwrap();
    assert!(ProtectedProject::open(&link, uid).is_err());
    fs::remove_file(&link).unwrap();
    fs::hard_link(&f.project, &link).unwrap();
    assert!(ProtectedProject::open(&f.project, uid).is_err());
}

#[test]
fn changed_stored_evaluation_and_new_revision_invalidate_review() {
    let f = Fixture::new();
    let project = f.open();
    let original: serde_json::Value = serde_json::from_slice(&f.proposal).unwrap();
    let run = original["evidence"]["source_run_id"].as_str().unwrap();
    let variant = original["evidence"]["variant_id"].as_str().unwrap();
    let evaluation = f
        .root
        .join("runs")
        .join(run)
        .join("evaluations")
        .join(format!("{variant}.json"));
    let bytes = fs::read(&evaluation).unwrap();
    fs::write(&evaluation, b"{}").unwrap();
    assert!(project.snapshot(&f.proposal).is_err());
    assert!(
        project
            .generate_accepted(3, 43, "tampered-head".into(), 1)
            .is_err()
    );
    assert_eq!(
        fs::read_dir(f.root.join(".archy/revisions"))
            .unwrap()
            .count(),
        1
    );
    fs::write(&evaluation, bytes).unwrap();
    assert!(project.snapshot(&f.proposal).is_ok());
    project
        .generate_accepted(3, 43, "next-protected-run".into(), 1)
        .unwrap();
    assert!(project.snapshot(&f.proposal).is_err());
}

#[test]
fn protected_writer_replays_exact_run_and_refuses_changed_request_or_source() {
    let f = Fixture::new();
    let writer = f.open();
    let first = writer
        .generate_accepted(3, 42, "fixture-generated-run".into(), 0)
        .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(
        ProjectStore::open(&f.project)
            .unwrap()
            .read_history()
            .unwrap()
            .len(),
        1
    );
    assert!(
        writer
            .generate_accepted(3, 43, "fixture-generated-run".into(), 0)
            .is_err()
    );
    assert!(
        writer
            .generate_accepted(33, 42, "too-large".into(), 1)
            .is_err()
    );
    assert!(writer.generate_accepted(3, 43, "stale".into(), 0).is_err());
    let replacement = f.root.join("replacement.json");
    fs::copy(&f.project, &replacement).unwrap();
    fs::rename(replacement, &f.project).unwrap();
    assert!(
        writer
            .generate_accepted(3, 43, "changed-source".into(), 1)
            .is_err()
    );
}

#[test]
fn initializer_is_exclusive_and_rejects_untrusted_state_or_invalid_source() {
    let f = Fixture::new();
    let owner = unsafe { libc::geteuid() };
    let original = fs::read(&f.project).unwrap();
    assert!(ProtectedProject::initialize(&f.base, owner, &original).is_err());
    assert_eq!(fs::read(&f.project).unwrap(), original);
    let other = f.base.join("other");
    fs::create_dir(&other).unwrap();
    fs::set_permissions(&other, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(ProtectedProject::initialize(&other, owner, b"{}").is_err());
    assert!(!other.join("project").exists());
    fs::set_permissions(&other, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(ProtectedProject::initialize(&other, owner, &original).is_err());
    assert!(!other.join("project").exists());
}

#[test]
fn writable_and_symlink_ancestors_reject_real_generated_run() {
    let f = Fixture::new();
    assert!(f.open().snapshot(&f.proposal).is_ok());
    fs::set_permissions(&f.base, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(ProtectedProject::open(&f.project, unsafe { libc::geteuid() }).is_err());
    fs::set_permissions(&f.base, fs::Permissions::from_mode(0o700)).unwrap();
    let alias = f.base.with_extension("alias");
    symlink(&f.base, &alias).unwrap();
    assert!(
        ProtectedProject::open(&alias.join("project/project.json"), unsafe {
            libc::geteuid()
        })
        .is_err()
    );
    fs::remove_file(alias).unwrap();
}

#[test]
fn ancestor_rename_and_source_replacement_invalidate_open_reviewer() {
    let f = Fixture::new();
    let reviewer = f.open();
    let moved = f.base.with_extension("moved");
    fs::rename(&f.base, &moved).unwrap();
    fs::create_dir(&f.base).unwrap();
    fs::set_permissions(&f.base, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(moved.join("project"), f.base.join("project")).unwrap();
    assert!(reviewer.snapshot(&f.proposal).is_err());
    fs::remove_dir_all(&f.base).unwrap();
    fs::rename(&moved, &f.base).unwrap();
    assert!(reviewer.snapshot(&f.proposal).is_ok());
    let replacement = f.root.join("replacement.json");
    fs::copy(&f.project, &replacement).unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(replacement, &f.project).unwrap();
    assert!(reviewer.snapshot(&f.proposal).is_err());
    let later_source = f.root.join("later.json");
    fs::write(&later_source, b"{\"later draft\":true}").unwrap();
    fs::set_permissions(&later_source, fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(later_source, &f.project).unwrap();
    let reopened = f.open();
    let display: serde_json::Value =
        serde_json::from_slice(reopened.snapshot(&f.proposal).unwrap().bytes()).unwrap();
    assert_eq!(display["current_source_differs_from_run"], true);
    assert!(
        display["input_utf8"]
            .as_str()
            .unwrap()
            .contains("courtyard-demo")
    );
}

#[test]
fn bounded_history_rejects_excess_before_verification() {
    let f = Fixture::new();
    let store = ProjectStore::open(&f.project).unwrap();
    let locked = store.lock().unwrap();
    assert!(locked.accepted_history_bounded(16, 1).is_err());
    drop(locked);
    let revisions = f.root.join(".archy/revisions");
    for n in 2..=17 {
        fs::write(revisions.join(format!("{n:020}.json")), b"{}").unwrap();
    }
    assert!(f.open().snapshot(&f.proposal).is_err());
}

#[test]
fn current_v2_source_publishes_a_verified_protected_run() {
    let f = Fixture::new_with_source(include_bytes!(
        "../../tests/fixtures/briefs/courtyard-v2.json"
    ));
    let snapshot = f.open().snapshot(&f.proposal).unwrap();
    let display: serde_json::Value = serde_json::from_slice(snapshot.bytes()).unwrap();
    assert_eq!(display["proposal_sha256"], hex_digest(&f.proposal));
    let normalized: serde_json::Value =
        serde_json::from_str(display["normalized_json"].as_str().unwrap()).unwrap();
    assert_eq!(normalized["schema_version"], "0.2.0");
}
