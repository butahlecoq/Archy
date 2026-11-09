//! Emit source-derived storage schemas and serialized values from a real transaction.
use archy_core::{
    MutationContext, ProjectStore, RUN_REQUEST_SCHEMA_VERSION, RevisionRecord, RunManifest,
    RunMetadata, RunRequest, revision_schema, run_manifest_schema, run_request_schema, sha256_hex,
};
use std::fs;

fn main() {
    let root =
        std::env::temp_dir().join(format!("archy-storage-contracts-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = b"storage schema fixture\n";
    fs::write(root.join("project.txt"), source).unwrap();
    let store = ProjectStore::open(&root.join("project.txt")).unwrap();
    let locked = store.lock().unwrap();
    let source = locked.read_source().unwrap();
    let request = RunRequest {
        schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
        context: MutationContext {
            idempotency_key: "storage-schema-fixture".into(),
            expected_project_revision: 0,
            destination: "runs".into(),
        },
        operation: "storage-schema-fixture".into(),
        request_bytes: b"fixture request".to_vec(),
        input_sha256: sha256_hex(&source),
        source_run: None,
    };
    let request_json = serde_json::to_string(&request).unwrap();
    let mut pending = locked.begin(request).unwrap().pending().unwrap();
    pending
        .write("input.txt", &source, "text/plain", "storage-contracts")
        .unwrap();
    let receipt = pending
        .commit(RunMetadata {
            project_id: "storage-schema-fixture".into(),
            normalized_sha256: None,
            source_format: None,
            seed: None,
            count: None,
            parent_variant_id: None,
            algorithm_version: "storage-fixture-v1".into(),
        })
        .unwrap();
    let verified = locked.read_run(&receipt.run_id).unwrap();
    let manifest_json = fs::read_to_string(root.join(&receipt.path).join("manifest.json")).unwrap();
    let revision_json =
        fs::read_to_string(root.join(format!(".archy/revisions/{:020}.json", receipt.revision)))
            .unwrap();
    let manifest: RunManifest = serde_json::from_str(&manifest_json).unwrap();
    let revision: RevisionRecord = serde_json::from_str(&revision_json).unwrap();
    assert_eq!(&manifest, verified.manifest());
    assert_eq!(revision.receipt, receipt);
    drop(locked);
    drop(store);
    fs::remove_dir_all(&root).unwrap();
    println!(
        "{}",
        serde_json::json!({
            "receipt": receipt,
            "schemas": {
                "run-manifest": run_manifest_schema(),
                "revision": revision_schema(),
                "run-request": run_request_schema(),
            },
            "serialized": {
                "run-request": request_json,
                "run-manifest": manifest_json,
                "revision": revision_json,
            },
        })
    );
}
