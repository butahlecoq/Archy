use archy_core::{MutationContext, ProjectStore, RunMetadata, RunRequest};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::Command,
};

fn fixture() -> PathBuf {
    let p = std::env::temp_dir().join(format!("archy-storage-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&p).unwrap();
    fs::write(p.join("project.json"), b"source").unwrap();
    p
}

#[test]
fn passive_history_does_not_create_state_and_reads_while_writer_locked() {
    let root = fixture();
    let project = root.join("project.json");
    let store = ProjectStore::open(&project).unwrap();
    let history = store.read_history().unwrap();
    assert_eq!(history.len(), 0);
    assert!(!root.join(".archy").exists());
    let locked = store.lock().unwrap();
    assert_eq!(store.read_history().unwrap().len(), 0);
    let mut pending = locked
        .begin(request("passive", 0))
        .unwrap()
        .pending()
        .unwrap();
    pending
        .write("data", b"hello", "text/plain", "test")
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    let snapshot = store.read_history().unwrap();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(
        snapshot.read_run(&receipt.run_id).unwrap().artifact("data"),
        Some(b"hello".as_slice())
    );
    let mut next = locked
        .begin(request("second", 1))
        .unwrap()
        .pending()
        .unwrap();
    next.write("data", b"second", "text/plain", "test").unwrap();
    let second = next.commit(metadata()).unwrap();
    assert_eq!(snapshot.revision(), 1);
    assert_eq!(snapshot.receipts().count(), 1);
    assert!(snapshot.read_run(&second.run_id).is_err());
    assert_eq!(store.read_history().unwrap().revision(), 2);
}

fn request(key: &str, revision: u64) -> RunRequest {
    RunRequest {
        schema_version: archy_core::RUN_REQUEST_SCHEMA_VERSION.into(),
        context: MutationContext {
            idempotency_key: key.into(),
            expected_project_revision: revision,
            destination: "runs".into(),
        },
        operation: "generate".into(),
        request_bytes: b"count=1,seed=42".to_vec(),
        input_sha256: archy_core::sha256_hex(b"source"),
        source_run: None,
    }
}

fn metadata() -> RunMetadata {
    RunMetadata {
        project_id: "fixture".into(),
        normalized_sha256: None,
        source_format: None,
        seed: None,
        count: None,
        parent_variant_id: None,
        algorithm_version: "storage-test-v1".into(),
    }
}

#[test]
fn commit_replay_and_conflict_preserve_prior_bytes() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    pending
        .write("input.json", b"source", "application/json", "test")
        .unwrap();
    let first = pending.commit(metadata()).unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(
        locked.begin(request("one", 0)).unwrap().receipt().unwrap(),
        first
    );
    let mut changed = request("one", 0);
    changed.request_bytes.push(1);
    assert!(locked.begin(changed).is_err());
    assert!(locked.begin(request("two", 0)).is_err());
    let verified = locked.read_run(&first.run_id).unwrap();
    assert_eq!(verified.artifact("input.json").unwrap(), b"source");
    assert_eq!(locked.revision().unwrap(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn accepted_history_rechecks_selected_artifacts_after_chain_snapshot() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked
        .begin(request("history", 0))
        .unwrap()
        .pending()
        .unwrap();
    pending
        .write("data", b"original", "text/plain", "test")
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    let history = locked.accepted_history().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(
        history.read_run(&receipt.run_id).unwrap().artifact("data"),
        Some(b"original".as_slice())
    );
    assert!(history.read_run(&uuid::Uuid::new_v4().to_string()).is_err());
    fs::write(root.join(&receipt.path).join("data"), b"tampered").unwrap();
    assert!(history.read_run(&receipt.run_id).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_traversal_and_occupied_targets_without_overwrite() {
    let root = fixture();
    fs::write(root.join("sentinel"), b"keep").unwrap();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    assert!(
        locked
            .begin(RunRequest {
                context: MutationContext {
                    destination: "../elsewhere".into(),
                    ..request("bad", 0).context
                },
                ..request("bad", 0)
            })
            .is_err()
    );
    let mut p = locked.begin(request("ok", 0)).unwrap().pending().unwrap();
    assert!(
        p.write("../sentinel", b"bad", "text/plain", "test")
            .is_err()
    );
    p.write("data", b"first", "text/plain", "test").unwrap();
    assert!(p.write("data", b"second", "text/plain", "test").is_err());
    assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"keep");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn tampered_artifact_and_broken_revision_refuse_new_mutation() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    pending
        .write("data", b"original", "text/plain", "test")
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    fs::write(root.join(&receipt.path).join("data"), b"forged").unwrap();
    assert!(locked.read_run(&receipt.run_id).is_err());
    assert!(locked.begin(request("two", 1)).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn symlink_and_hardlink_substitution_are_rejected() {
    let root = fixture();
    let outside = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    pending
        .write("data", b"original", "text/plain", "test")
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    let data = root.join(&receipt.path).join("data");
    fs::hard_link(&data, root.join("alias")).unwrap();
    assert!(locked.read_run(&receipt.run_id).is_err());
    fs::remove_file(root.join("alias")).unwrap();
    fs::remove_file(&data).unwrap();
    symlink(outside.join("sentinel"), &data).unwrap();
    assert!(locked.read_run(&receipt.run_id).is_err());
    assert_eq!(fs::read(outside.join("project.json")).unwrap(), b"source");
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn occupied_destination_component_is_preserved() {
    let root = fixture();
    let outside = fixture();
    symlink(&outside, root.join("runs")).unwrap();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let error = locked.begin(request("one", 0)).err().unwrap();
    assert_eq!(error.code, archy_core::ErrorCode::PermissionDenied);
    assert_eq!(locked.revision().unwrap(), 0);
    assert_eq!(fs::read(outside.join("project.json")).unwrap(), b"source");
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn child_lock_probe() {
    let Ok(path) = std::env::var("ARCHY_LOCK_PROBE_PROJECT") else {
        return;
    };
    let store = ProjectStore::open(std::path::Path::new(&path)).unwrap();
    let error = match store.lock() {
        Ok(_) => panic!("competing process acquired lock"),
        Err(e) => e,
    };
    assert_eq!(error.code, archy_core::ErrorCode::RevisionConflict);
    assert!(error.retriable);
}

#[test]
fn persistent_lock_rejects_competing_process() {
    let root = fixture();
    let project = root.join("project.json");
    let store = ProjectStore::open(&project).unwrap();
    let locked = store.lock().unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_lock_probe", "--nocapture"])
        .env("ARCHY_LOCK_PROBE_PROJECT", &project)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    drop(locked);
    assert!(store.lock().is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dropped_lock_releases_when_forked_child_still_has_descriptor() {
    use std::{
        io::{Read, Write},
        os::unix::{net::UnixStream, process::CommandExt},
    };

    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let (mut ready_parent, mut ready_child) = UnixStream::pair().unwrap();
    let (mut release_parent, mut release_child) = UnixStream::pair().unwrap();
    let child = std::thread::spawn(move || {
        let mut command = Command::new("/bin/true");
        // Only async-signal-safe syscalls run between fork and exec.
        unsafe {
            command.pre_exec(move || {
                ready_child.write_all(b"1")?;
                let mut signal = [0];
                release_child.read_exact(&mut signal)?;
                Ok(())
            });
        }
        command.status().unwrap()
    });
    let mut signal = [0];
    ready_parent.read_exact(&mut signal).unwrap();
    let while_held = store.lock().err();
    drop(locked);
    let reacquired = store.lock();
    release_parent.write_all(b"1").unwrap();
    assert!(child.join().unwrap().success());
    assert_eq!(
        while_held.unwrap().code,
        archy_core::ErrorCode::RevisionConflict
    );
    assert!(reacquired.is_ok(), "{:?}", reacquired.err());
}

#[test]
fn revision_gap_refuses_mutation() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut first = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    first.write("data", b"a", "text/plain", "test").unwrap();
    first.commit(metadata()).unwrap();
    let mut second = locked.begin(request("two", 1)).unwrap().pending().unwrap();
    second.write("data", b"b", "text/plain", "test").unwrap();
    second.commit(metadata()).unwrap();
    fs::remove_file(root.join(".archy/revisions/00000000000000000001.json")).unwrap();
    assert_eq!(
        locked.revision().unwrap_err().code,
        archy_core::ErrorCode::ArtifactInvalid
    );
    assert!(locked.begin(request("three", 2)).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pinned_root_survives_path_replacement() {
    let root = fixture();
    let moved = root.with_extension("moved");
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("sentinel"), b"outside").unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    pending
        .write("data", b"inside", "text/plain", "test")
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    assert_eq!(
        fs::read(moved.join(&receipt.path).join("data")).unwrap(),
        b"inside"
    );
    assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"outside");
    assert!(!root.join(".archy").exists());
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(moved).unwrap();
}

#[test]
fn occupied_run_rename_target_is_never_replaced() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    let occupied = root.join("runs").join(pending.run_id());
    fs::create_dir_all(&occupied).unwrap();
    fs::set_permissions(root.join("runs"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&occupied, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(occupied.join("sentinel"), b"keep").unwrap();
    pending.write("data", b"new", "text/plain", "test").unwrap();
    assert!(pending.commit(metadata()).is_err());
    assert_eq!(fs::read(occupied.join("sentinel")).unwrap(), b"keep");
    assert_eq!(locked.revision().unwrap(), 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn occupied_revision_rename_target_is_never_replaced() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    let occupied = root.join(".archy/revisions/00000000000000000001.json");
    fs::write(&occupied, b"sentinel").unwrap();
    pending.write("data", b"new", "text/plain", "test").unwrap();
    let error = pending.commit(metadata()).unwrap_err();
    assert!(error.retriable);
    assert_eq!(fs::read(&occupied).unwrap(), b"sentinel");
    assert_eq!(
        locked.revision().unwrap_err().code,
        archy_core::ErrorCode::ArtifactInvalid
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn request_version_and_source_run_are_bounded_before_fingerprinting() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut supported = request("at-limit", 0);
    supported.source_run = Some(uuid::Uuid::new_v4().to_string());
    assert!(locked.begin(supported.clone()).unwrap().pending().is_some());
    let staged_before = fs::read_dir(root.join(".archy/staging")).unwrap().count();
    let mut oversized = supported.clone();
    oversized.source_run = Some(format!("{}x", supported.source_run.unwrap()));
    assert_eq!(
        locked.begin(oversized).err().unwrap().code,
        archy_core::ErrorCode::InvalidSchema
    );
    assert_eq!(
        fs::read_dir(root.join(".archy/staging")).unwrap().count(),
        staged_before
    );
    let mut unsupported = request("bad-version", 0);
    unsupported.schema_version = "archy-run-request-v2".into();
    assert_eq!(
        locked.begin(unsupported).err().unwrap().code,
        archy_core::ErrorCode::InvalidSchema
    );
    assert_eq!(locked.revision().unwrap(), 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unsupported_request_version_preserves_v1_receipt_and_bytes() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let v1 = request("one", 0);
    let serialized = serde_json::to_value(&v1).unwrap();
    assert_eq!(serialized["schema_version"], "archy-run-request-v1");
    let mut pending = locked.begin(v1.clone()).unwrap().pending().unwrap();
    pending
        .write("data", b"original", "text/plain", "test")
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    let revision_bytes = fs::read(root.join(".archy/revisions/00000000000000000001.json")).unwrap();
    let mut unsupported = v1.clone();
    unsupported.schema_version = "archy-run-request-v2".into();
    assert_eq!(
        locked.begin(unsupported).err().unwrap().code,
        archy_core::ErrorCode::InvalidSchema
    );
    assert_eq!(locked.begin(v1).unwrap().receipt().unwrap(), receipt);
    assert_eq!(
        fs::read(root.join(".archy/revisions/00000000000000000001.json")).unwrap(),
        revision_bytes
    );
    assert_eq!(
        locked.read_run(&receipt.run_id).unwrap().artifact("data"),
        Some(b"original".as_slice())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn input_and_request_byte_limits_accept_exactly_one_mib_and_reject_one_over() {
    let root = fixture();
    let at_limit = vec![b'x'; archy_core::validation::MAX_INPUT_BYTES];
    fs::write(root.join("project.json"), &at_limit).unwrap();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    assert_eq!(locked.read_source().unwrap().len(), at_limit.len());
    let mut request = request("at-limit", 0);
    request.input_sha256 = archy_core::sha256_hex(&at_limit);
    request.request_bytes = at_limit.clone();
    assert!(locked.begin(request.clone()).unwrap().pending().is_some());
    request.context.idempotency_key = "over-limit".into();
    request.request_bytes.push(b'x');
    assert_eq!(
        locked.begin(request).err().unwrap().code,
        archy_core::ErrorCode::InvalidSchema
    );
    fs::write(root.join("project.json"), vec![b'x'; at_limit.len() + 1]).unwrap();
    assert_eq!(
        ProjectStore::open(&root.join("project.json"))
            .err()
            .unwrap()
            .code,
        archy_core::ErrorCode::InvalidSchema
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn artifact_byte_limit_accepts_exactly_sixteen_mib_and_rejects_one_over() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    let at_limit = vec![0_u8; archy_core::MAX_ARTIFACT_BYTES];
    pending
        .write("large.bin", &at_limit, "application/octet-stream", "test")
        .unwrap();
    assert_eq!(
        fs::metadata(
            root.join(".archy/staging")
                .join(pending.run_id())
                .join("large.bin")
        )
        .unwrap()
        .len(),
        at_limit.len() as u64
    );
    let mut over = at_limit;
    over.push(0);
    assert_eq!(
        pending
            .write("over.bin", &over, "application/octet-stream", "test")
            .unwrap_err()
            .code,
        archy_core::ErrorCode::InvalidSchema
    );
    assert!(
        !root
            .join(".archy/staging")
            .join(pending.run_id())
            .join("over.bin")
            .exists()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn locked_reader_lists_accepted_receipts_and_reads_bounded_relative_input() {
    let root = fixture();
    fs::write(root.join("params.json"), br#"{"shape":"rectangle"}"#).unwrap();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    assert_eq!(
        locked.read_project_file("params.json", 1024).unwrap(),
        br#"{"shape":"rectangle"}"#
    );
    assert!(locked.read_project_file("../params.json", 1024).is_err());
    assert!(locked.read_project_file("params.json", 3).is_err());
    assert!(locked.receipts().unwrap().is_empty());
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    pending
        .write("data", b"source", "text/plain", "test")
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    assert_eq!(locked.receipts().unwrap(), vec![receipt]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn optional_manifest_fields_reject_over_limit_before_serialization() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let valid_id = format!("variant-{}", "0".repeat(64));
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    pending
        .write("data", b"source", "text/plain", "test")
        .unwrap();
    let mut md = metadata();
    md.source_format = Some("yaml".into());
    md.parent_variant_id = Some(valid_id.clone());
    let receipt = pending.commit(md).unwrap();
    assert_eq!(
        locked
            .read_run(&receipt.run_id)
            .unwrap()
            .manifest()
            .parent_variant_id
            .as_deref(),
        Some(valid_id.as_str())
    );
    let mut pending = locked.begin(request("two", 1)).unwrap().pending().unwrap();
    pending.write("data", b"x", "text/plain", "test").unwrap();
    let staging_manifest = root
        .join(".archy/staging")
        .join(pending.run_id())
        .join("manifest.json");
    let mut md = metadata();
    md.source_format = Some("x".repeat(17));
    assert_eq!(
        pending.commit(md).unwrap_err().code,
        archy_core::ErrorCode::InvalidSchema
    );
    assert!(!staging_manifest.exists());
    let mut pending = locked
        .begin(request("three", 1))
        .unwrap()
        .pending()
        .unwrap();
    pending.write("data", b"x", "text/plain", "test").unwrap();
    let mut md = metadata();
    md.parent_variant_id = Some(format!("{valid_id}x"));
    assert_eq!(
        pending.commit(md).unwrap_err().code,
        archy_core::ErrorCode::InvalidSchema
    );
    assert_eq!(locked.revision().unwrap(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_receipt_path_and_revision_link_are_artifact_invalid() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("one", 0)).unwrap().pending().unwrap();
    pending
        .write("data", b"source", "text/plain", "test")
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    let revision_path = root.join(".archy/revisions/00000000000000000001.json");
    let original = fs::read(&revision_path).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    value["receipt"]["path"] = serde_json::json!("../escape");
    fs::write(&revision_path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        locked.revision().unwrap_err().code,
        archy_core::ErrorCode::ArtifactInvalid
    );
    assert_eq!(
        locked.read_run(&receipt.run_id).unwrap_err().code,
        archy_core::ErrorCode::ArtifactInvalid
    );
    assert_eq!(
        locked.begin(request("two", 1)).err().unwrap().code,
        archy_core::ErrorCode::ArtifactInvalid
    );
    fs::remove_file(&revision_path).unwrap();
    symlink(root.join("project.json"), &revision_path).unwrap();
    assert_eq!(
        locked.revision().unwrap_err().code,
        archy_core::ErrorCode::ArtifactInvalid
    );
    assert_eq!(
        locked.read_run(&receipt.run_id).unwrap_err().code,
        archy_core::ErrorCode::ArtifactInvalid
    );
    assert_eq!(
        locked.begin(request("two", 1)).err().unwrap().code,
        archy_core::ErrorCode::ArtifactInvalid
    );
    assert_eq!(fs::read(root.join("project.json")).unwrap(), b"source");
    fs::remove_dir_all(root).unwrap();
}
