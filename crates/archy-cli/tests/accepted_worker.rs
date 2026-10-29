#![cfg(target_os = "linux")]
use archy_adapters::ServiceRequest;
use archy_cli::{
    AcceptedWorkerBackend, GenerateInput, GenerationMode, MutationOptions,
    accepted_service_request, run_accepted_service, start_accepted_service,
};
use archy_core::{ErrorCode, ProjectStore, RevisionRecord, RunManifest, sha256_hex};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn temp_project() -> PathBuf {
    let root = std::env::temp_dir().join(format!("archy-worker-bind-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.json");
    fs::write(
        &project,
        include_bytes!("../../../tests/fixtures/briefs/courtyard.json"),
    )
    .unwrap();
    project
}

fn generate(project: &Path, seed: u64) -> archy_cli::WorkflowReceipt {
    archy_cli::generate(GenerateInput {
        project: project.to_path_buf(),
        mode: GenerationMode::Sample { count: 1, seed },
        mutation: MutationOptions::default(),
    })
    .unwrap()
}

fn wire(request: ServiceRequest) -> Value {
    let bytes = match request {
        ServiceRequest::Blender(request) => request.to_bytes().unwrap(),
        ServiceRequest::Ifc(request) => request.to_bytes().unwrap(),
    };
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn current_and_historical_runs_bind_owned_wire_to_verified_input() {
    let project = temp_project();
    let old = generate(&project, 42);
    let current = generate(&project, 43);
    let old_input = fs::read(project.parent().unwrap().join(&old.path).join("input.json")).unwrap();
    for receipt in [&old, &current] {
        for backend in [AcceptedWorkerBackend::Blender, AcceptedWorkerBackend::Ifc] {
            let request = accepted_service_request(
                &project,
                &receipt.run_id,
                &receipt.variant_ids[0],
                "job-1",
                backend,
            )
            .unwrap();
            let wire = wire(request);
            assert_eq!(wire["run_id"], receipt.run_id);
            assert_eq!(wire["variant_id"], receipt.variant_ids[0]);
            assert_eq!(wire["project_id"], "courtyard-demo");
            let digest = sha256_hex(&old_input);
            let field = if backend == AcceptedWorkerBackend::Blender {
                assert_eq!(wire["output_dir"], "output");
                "input_digest"
            } else {
                assert_eq!(wire["output_dir"], "output/ifc");
                assert_eq!(wire["source_artifacts"], serde_json::json!([]));
                "source_digest"
            };
            assert_eq!(wire[field], digest);
        }
    }
    let mut changed_source: Value = serde_json::from_slice(&fs::read(&project).unwrap()).unwrap();
    changed_source["generation"]["seed"] = serde_json::json!(9001);
    fs::write(&project, serde_json::to_vec(&changed_source).unwrap()).unwrap();
    let historical = wire(
        accepted_service_request(
            &project,
            &old.run_id,
            &old.variant_ids[0],
            "job-3",
            AcceptedWorkerBackend::Blender,
        )
        .unwrap(),
    );
    assert_eq!(historical["input_digest"], sha256_hex(&old_input));
    assert_eq!(historical["run_id"], old.run_id);
    if !std::path::Path::new("/usr/lib/archy/archy-service-gate").exists() {
        let error = start_accepted_service(
            &project,
            &current.run_id,
            &current.variant_ids[0],
            "job-2",
            AcceptedWorkerBackend::Blender,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::BackendUnavailable);
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let error = run_accepted_service(
            &project,
            &current.run_id,
            &current.variant_ids[0],
            "job-2",
            AcceptedWorkerBackend::Blender,
            &cancelled,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::BackendUnavailable);
    }
    fs::remove_dir_all(project.parent().unwrap()).unwrap();
}

#[test]
fn absent_foreign_stale_and_tampered_evidence_is_rejected() {
    let project = temp_project();
    let receipt = generate(&project, 42);
    let variant = &receipt.variant_ids[0];
    let reject = |project: &PathBuf, run: &str, variant: &str| {
        accepted_service_request(
            project,
            run,
            variant,
            "job-1",
            AcceptedWorkerBackend::Blender,
        )
        .err()
        .expect("untrusted evidence must fail")
    };
    assert_eq!(
        reject(&project, &uuid::Uuid::new_v4().to_string(), variant).code,
        ErrorCode::ArtifactInvalid
    );
    assert_eq!(
        reject(&project, &receipt.run_id, "variant-absent").code,
        ErrorCode::ArtifactInvalid
    );
    let orphan_id = uuid::Uuid::new_v4().to_string();
    let orphan = project.parent().unwrap().join("runs").join(&orphan_id);
    fs::create_dir(&orphan).unwrap();
    fs::copy(
        project
            .parent()
            .unwrap()
            .join(&receipt.path)
            .join("manifest.json"),
        orphan.join("manifest.json"),
    )
    .unwrap();
    assert_eq!(
        reject(&project, &orphan_id, variant).code,
        ErrorCode::ArtifactInvalid
    );
    let foreign = temp_project();
    let foreign_receipt = generate(&foreign, 43);
    assert_eq!(
        reject(
            &project,
            &foreign_receipt.run_id,
            &foreign_receipt.variant_ids[0]
        )
        .code,
        ErrorCode::ArtifactInvalid
    );
    fs::remove_dir_all(foreign.parent().unwrap()).unwrap();

    let original = fs::read(&project).unwrap();
    fs::write(&project, b"not a project").unwrap();
    assert!(reject(&project, &receipt.run_id, variant).code != ErrorCode::BackendUnavailable);
    fs::write(&project, &original).unwrap();
    let mut other_project: Value = serde_json::from_slice(&original).unwrap();
    other_project["project_id"] = serde_json::json!("other-project");
    fs::write(&project, serde_json::to_vec(&other_project).unwrap()).unwrap();
    assert_eq!(
        reject(&project, &receipt.run_id, variant).code,
        ErrorCode::ArtifactInvalid
    );
    fs::write(&project, &original).unwrap();
    let run_dir = project.parent().unwrap().join(&receipt.path);
    let variant_file = run_dir.join(format!("variants/{variant}.json"));
    fs::write(&variant_file, b"{}").unwrap();
    assert_eq!(
        reject(&project, &receipt.run_id, variant).code,
        ErrorCode::ArtifactInvalid
    );
    fs::remove_dir_all(project.parent().unwrap()).unwrap();
}

#[test]
fn rehashed_semantic_forgery_and_concurrent_lock_fail_closed() {
    let project = temp_project();
    let receipt = generate(&project, 42);
    let variant_id = &receipt.variant_ids[0];
    let run_dir = project.parent().unwrap().join(&receipt.path);
    let variant_path = format!("variants/{variant_id}.json");
    let path = run_dir.join(&variant_path);
    let mut variant: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    variant["footprint_xy_m"][0][0] = serde_json::json!(500.0);
    let bytes = serde_json::to_vec(&variant).unwrap();
    fs::write(path, &bytes).unwrap();
    let manifest_path = run_dir.join("manifest.json");
    let mut manifest: RunManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let artifact = manifest
        .artifacts
        .iter_mut()
        .find(|a| a.path == variant_path)
        .unwrap();
    artifact.sha256 = sha256_hex(&bytes);
    artifact.byte_len = bytes.len() as u64;
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(manifest_path, &manifest_bytes).unwrap();
    let revision_path = project
        .parent()
        .unwrap()
        .join(".archy/revisions/00000000000000000001.json");
    let mut revision: RevisionRecord =
        serde_json::from_slice(&fs::read(&revision_path).unwrap()).unwrap();
    revision.receipt.manifest_sha256 = sha256_hex(&manifest_bytes);
    fs::write(revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();
    let store = ProjectStore::open(&project).unwrap();
    assert!(store.lock().unwrap().read_run(&receipt.run_id).is_ok());
    let error = accepted_service_request(
        &project,
        &receipt.run_id,
        variant_id,
        "job-1",
        AcceptedWorkerBackend::Ifc,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::ArtifactInvalid);

    let lock = store.lock().unwrap();
    let error = accepted_service_request(
        &project,
        &receipt.run_id,
        variant_id,
        "job-1",
        AcceptedWorkerBackend::Ifc,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::RevisionConflict);
    drop(lock);
    fs::remove_dir_all(project.parent().unwrap()).unwrap();
}

#[test]
fn concurrent_writer_cannot_mix_a_request_snapshot() {
    let project = temp_project();
    let first = generate(&project, 42);
    let before = wire(
        accepted_service_request(
            &project,
            &first.run_id,
            &first.variant_ids[0],
            "job-1",
            AcceptedWorkerBackend::Blender,
        )
        .unwrap(),
    );
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let writer_project = project.clone();
    let writer = std::thread::spawn(move || {
        let store = ProjectStore::open(&writer_project).unwrap();
        let lock = store.lock().unwrap();
        locked_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        drop(lock);
        generate(&writer_project, 43)
    });
    locked_rx.recv().unwrap();
    let error = accepted_service_request(
        &project,
        &first.run_id,
        &first.variant_ids[0],
        "job-2",
        AcceptedWorkerBackend::Blender,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::RevisionConflict);
    release_tx.send(()).unwrap();
    let second = writer.join().unwrap();
    let after = wire(
        accepted_service_request(
            &project,
            &first.run_id,
            &first.variant_ids[0],
            "job-1",
            AcceptedWorkerBackend::Blender,
        )
        .unwrap(),
    );
    assert_eq!(before, after);
    assert_ne!(second.run_id, first.run_id);
    fs::remove_dir_all(project.parent().unwrap()).unwrap();
}
