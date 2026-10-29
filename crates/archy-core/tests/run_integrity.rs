use archy_core::{
    MutationContext, ProjectStore, RevisionRecord, RunManifest, RunMetadata, RunRequest, sha256_hex,
};
use std::{fs, path::PathBuf};

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!("archy-integrity-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("project.json"), b"source").unwrap();
    root
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
fn rehashed_evaluation_summary_remains_untrusted_bytes() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let request = RunRequest {
        schema_version: archy_core::RUN_REQUEST_SCHEMA_VERSION.into(),
        context: MutationContext {
            idempotency_key: "original".into(),
            expected_project_revision: 0,
            destination: "runs".into(),
        },
        operation: "generate".into(),
        request_bytes: b"seed=42".to_vec(),
        input_sha256: sha256_hex(b"source"),
        source_run: None,
    };
    let mut pending = locked.begin(request).unwrap().pending().unwrap();
    pending
        .write("input.json", b"source", "application/json", "test")
        .unwrap();
    pending
        .write(
            "evaluation.json",
            br#"{"required_checks_passed":false}"#,
            "application/json",
            "test",
        )
        .unwrap();
    let receipt = pending.commit(metadata()).unwrap();

    let run_dir = root.join(&receipt.path);
    let forged = br#"{"required_checks_passed":true}"#;
    fs::write(run_dir.join("evaluation.json"), forged).unwrap();
    let mut manifest: RunManifest =
        serde_json::from_slice(&fs::read(run_dir.join("manifest.json")).unwrap()).unwrap();
    let evaluation = manifest
        .artifacts
        .iter_mut()
        .find(|a| a.path == "evaluation.json")
        .unwrap();
    evaluation.sha256 = sha256_hex(forged);
    evaluation.byte_len = forged.len() as u64;
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(run_dir.join("manifest.json"), &manifest_bytes).unwrap();
    let revision_path = root.join(".archy/revisions/00000000000000000001.json");
    let mut revision: RevisionRecord =
        serde_json::from_slice(&fs::read(&revision_path).unwrap()).unwrap();
    revision.receipt.manifest_sha256 = sha256_hex(&manifest_bytes);
    fs::write(&revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();

    let inventory = locked.read_run(&receipt.run_id).unwrap();
    assert_eq!(inventory.artifact("evaluation.json").unwrap(), forged);
    assert_eq!(inventory.manifest().run_id, receipt.run_id);
    // No trusted evaluation status is exported by the core reader.
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn strict_manifest_and_revision_schemas_reject_unknown_fields() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let request = RunRequest {
        schema_version: archy_core::RUN_REQUEST_SCHEMA_VERSION.into(),
        context: MutationContext {
            idempotency_key: "one".into(),
            expected_project_revision: 0,
            destination: "runs".into(),
        },
        operation: "generate".into(),
        request_bytes: vec![],
        input_sha256: sha256_hex(b"source"),
        source_run: None,
    };
    let mut pending = locked.begin(request).unwrap().pending().unwrap();
    pending.write("data", b"x", "text/plain", "test").unwrap();
    let receipt = pending.commit(metadata()).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(&receipt.path).join("manifest.json")).unwrap())
            .unwrap();
    value["forged_authority"] = serde_json::json!(true);
    assert!(serde_json::from_value::<RunManifest>(value).is_err());
    let mut value: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(".archy/revisions/00000000000000000001.json")).unwrap(),
    )
    .unwrap();
    value["forged_authority"] = serde_json::json!(true);
    assert!(serde_json::from_value::<RevisionRecord>(value).is_err());
    fs::remove_dir_all(root).unwrap();
}
