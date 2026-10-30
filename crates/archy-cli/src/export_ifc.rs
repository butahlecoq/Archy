use crate::{
    BoundProjectRead, MutationOptions, artifact_invalid, recorded_manifest_for_key,
    request_conflict, run_integrity::verify_locked,
};
use archy_adapters::{
    CaptureOutcome, CapturedOutputs, IfcRequest, ServiceRequest, ServiceSession, capture_ifc,
};
use archy_core::{
    ArchyError, RUN_REQUEST_SCHEMA_VERSION, RunMetadata, RunReceipt, RunRequest, VariantOutcome,
    sha256_hex,
};
use archy_eval::{evaluate_variant, evaluate_variant_v2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::atomic::AtomicBool};

pub const DESIGN_MANIFEST_VERSION: &str = "archy-design-manifest-v1";
pub(crate) const DESIGN_LIMITATIONS: &str = "Conceptual storey mass proxies only; no walls, slabs, spaces, regulatory or professional approval.";

#[derive(Debug, Clone)]
pub struct ExportIfcInput {
    pub project: PathBuf,
    pub source_run: Option<String>,
    pub variant_id: String,
    pub mutation: MutationOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignManifest {
    pub schema_version: String,
    pub scope: String,
    pub approval_state: String,
    pub project_id: String,
    pub source_run_id: String,
    pub export_run_id: String,
    pub variant_id: String,
    pub source_input_sha256: String,
    pub source_normalized_sha256: String,
    pub variant_sha256: String,
    pub evaluation_sha256: String,
    pub required_checks_passed: bool,
    pub evaluation: archy_core::EvaluationReport,
    pub worker_response_sha256: String,
    pub model_sha256: String,
    pub validation_sha256: String,
    pub worker_version: String,
    pub backend_version: String,
    pub python_version: String,
    pub limitations: String,
}

pub fn design_manifest_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(DesignManifest))
        .expect("design manifest schema serializes");
    schema["$id"] = "https://archy.local/schemas/design-manifest.schema.json".into();
    schema["title"] = "Archy draft schematic IFC design manifest".into();
    for (field, value) in [
        ("schema_version", DESIGN_MANIFEST_VERSION),
        ("scope", "SCHEMATIC_MASSING_ONLY"),
        ("approval_state", "DRAFT"),
        ("limitations", DESIGN_LIMITATIONS),
        ("worker_version", "ifc-worker-v1"),
        ("backend_version", "0.9.0.post1"),
        ("python_version", "3.12.16"),
    ] {
        schema["properties"][field]["const"] = value.into();
    }
    for field in [
        "source_input_sha256",
        "source_normalized_sha256",
        "variant_sha256",
        "evaluation_sha256",
        "worker_response_sha256",
        "model_sha256",
        "validation_sha256",
    ] {
        schema["properties"][field]["pattern"] = "^[0-9a-f]{64}$".into();
    }
    schema
}

fn run_worker(request: &IfcRequest) -> Result<CapturedOutputs, ArchyError> {
    let cancel = AtomicBool::new(false);
    let mut session = ServiceSession::start(ServiceRequest::Ifc(request.clone()))?;
    session.release(&cancel)?;
    let (scratch, exit) = session.finish(&cancel)?;
    let outcome = capture_ifc(scratch.as_file(), request)?;
    match (exit, outcome) {
        (archy_adapters::WorkerExit::Success, CaptureOutcome::Success(outputs)) => Ok(outputs),
        (archy_adapters::WorkerExit::HandledError, CaptureOutcome::WorkerError(error)) => {
            Err(error
                .bound_error()
                .cloned()
                .unwrap_or_else(|| artifact_invalid("unbound IFC worker error")))
        }
        _ => Err(artifact_invalid("worker exit and response disagree")),
    }
}

pub fn export_ifc(input: ExportIfcInput) -> Result<RunReceipt, ArchyError> {
    export_with(input, &run_worker)
}

pub(crate) fn export_with(
    input: ExportIfcInput,
    runner: &dyn Fn(&IfcRequest) -> Result<CapturedOutputs, ArchyError>,
) -> Result<RunReceipt, ArchyError> {
    let reader = BoundProjectRead::open(&input.project)?;
    let snapshot = reader.snapshot()?;
    snapshot.verify_head()?;
    let source_id = match &input.source_run {
        Some(id) => id.clone(),
        None => {
            let mut found = None;
            for receipt in snapshot.receipts().rev() {
                let run = snapshot.verify_run(&receipt.run_id)?;
                if matches!(run.manifest().operation.as_str(), "generate" | "edit") {
                    found = Some(receipt.run_id.clone());
                    break;
                }
            }
            found.ok_or_else(|| ArchyError::schema("run", "No accepted generation or edit run"))?
        }
    };
    let source = snapshot.verify_run(&source_id)?;
    if !matches!(source.manifest().operation.as_str(), "generate" | "edit") {
        return Err(artifact_invalid("IFC source must be generation or edit"));
    }
    let variant = source
        .variants()
        .iter()
        .find(|v| v.id == input.variant_id)
        .ok_or_else(|| ArchyError::schema("variant", "Variant is absent from source run"))?;
    let evaluation = if source.canonical().is_some() {
        evaluate_variant_v2(source.spec(), variant)?
    } else {
        evaluate_variant(source.spec(), variant)?
    };
    let source_digest = sha256_hex(source.input());
    let source_normalized = source
        .manifest()
        .normalized_sha256
        .clone()
        .ok_or_else(|| artifact_invalid("missing normalized digest"))?;
    let variant_bytes = serde_json::to_vec(variant).map_err(|e| artifact_invalid(e.to_string()))?;
    let evaluation_bytes =
        serde_json::to_vec(&evaluation).map_err(|e| artifact_invalid(e.to_string()))?;
    let request_selector = serde_json::to_vec(
        &serde_json::json!({"source_run":source_id,"variant_id":input.variant_id}),
    )
    .map_err(|e| artifact_invalid(e.to_string()))?;
    drop(snapshot);
    let locked = reader.lock()?;
    let context = input.mutation.resolve(&locked)?;
    let recorded = recorded_manifest_for_key(&locked, input.mutation.idempotency_key.as_deref())?;
    if recorded.as_ref().is_some_and(|m| {
        m.operation != "export-ifc"
            || m.parent_run_id.as_deref() != Some(&source_id)
            || m.parent_variant_id.as_deref() != Some(&input.variant_id)
    }) {
        return Err(request_conflict());
    }
    let run_request = RunRequest {
        schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
        context,
        operation: "export-ifc".into(),
        request_bytes: request_selector,
        input_sha256: source_digest.clone(),
        source_run: Some(source_id.clone()),
    };
    let validate_orphan = |history: &archy_core::AcceptedHistory<'_>, run_id: &str| {
        let trusted = crate::run_integrity::verify_history(history, run_id)?;
        if trusted.manifest().operation != "export-ifc"
            || trusted.manifest().parent_run_id.as_deref() != Some(source_id.as_str())
            || trusted.manifest().parent_variant_id.as_deref() != Some(input.variant_id.as_str())
            || trusted.input() != source.input()
            || trusted.variants().len() != 1
            || trusted.variants()[0].id != input.variant_id
        {
            return Err(artifact_invalid(
                "recovered IFC run differs from selected source",
            ));
        }
        Ok(())
    };
    if recorded.is_some() {
        return match locked.begin_checked_recovery(
            run_request,
            &|| reader.check_binding(),
            &validate_orphan,
        )? {
            archy_core::BeginResult::Replay(receipt) => {
                verify_locked(&locked, &receipt.run_id)?;
                reader.check_binding()?;
                Ok(receipt)
            }
            archy_core::BeginResult::Pending(_) => {
                Err(artifact_invalid("recorded export unexpectedly pending"))
            }
        };
    }
    // Reverify under the publication lock; the worker request is bound to accepted bytes.
    let current = verify_locked(&locked, &source_id)?;
    if current.input() != source.input() || !current.variants().contains(variant) {
        return Err(artifact_invalid("source changed before export"));
    }
    let source_profile_version = current.source_schema_version();
    let request_id = uuid::Uuid::new_v4().to_string();
    let request = IfcRequest::new(
        source.spec(),
        variant,
        &request_id,
        &source_id,
        &source_digest,
        "output/ifc",
        Vec::new(),
    )?;
    let mut pending = match locked.begin_checked_recovery(
        run_request,
        &|| reader.check_binding(),
        &validate_orphan,
    )? {
        archy_core::BeginResult::Pending(pending) => pending,
        archy_core::BeginResult::Replay(receipt) => {
            verify_locked(&locked, &receipt.run_id)?;
            reader.check_binding()?;
            return Ok(receipt);
        }
    };
    let outputs = match runner(&request) {
        Ok(outputs) => outputs,
        Err(error) => {
            pending.discard_empty()?;
            return Err(error);
        }
    };
    reader.check_binding()?;
    let model = outputs
        .artifacts()
        .iter()
        .find(|a| a.path() == "model.ifc")
        .ok_or_else(|| artifact_invalid("missing IFC model"))?;
    let validation = outputs
        .artifacts()
        .iter()
        .find(|a| a.path() == "validation.json")
        .ok_or_else(|| artifact_invalid("missing IFC validation"))?;
    let response: serde_json::Value = serde_json::from_slice(outputs.response_bytes())
        .map_err(|e| artifact_invalid(format!("invalid worker response: {e}")))?;
    let validation_json: serde_json::Value = serde_json::from_slice(validation.bytes())
        .map_err(|e| artifact_invalid(format!("invalid validation artifact: {e}")))?;
    if response["status"] != "ok"
        || sha256_hex(model.bytes()) != model.sha256()
        || sha256_hex(validation.bytes()) != validation.sha256()
        || response["validation"] != validation_json
    {
        return Err(artifact_invalid("worker output byte claim mismatch"));
    }
    if let Err(error) = crate::ifc_verifier::verify(
        model.bytes(),
        source.input(),
        source.spec(),
        source.canonical(),
        source
            .manifest()
            .source_format
            .as_deref()
            .unwrap_or_default(),
        variant,
        &source_id,
    ) {
        pending.discard_empty()?;
        return Err(error);
    }
    let design = DesignManifest {
        schema_version: DESIGN_MANIFEST_VERSION.into(),
        scope: "SCHEMATIC_MASSING_ONLY".into(),
        approval_state: "DRAFT".into(),
        project_id: source.spec().project_id.clone(),
        source_run_id: source_id.clone(),
        export_run_id: pending.run_id().into(),
        variant_id: input.variant_id.clone(),
        source_input_sha256: source_digest,
        source_normalized_sha256: source_normalized,
        variant_sha256: sha256_hex(&variant_bytes),
        evaluation_sha256: sha256_hex(&evaluation_bytes),
        required_checks_passed: evaluation.required_checks_passed,
        evaluation,
        worker_response_sha256: outputs.response_sha256().into(),
        model_sha256: model.sha256().into(),
        validation_sha256: validation.sha256().into(),
        worker_version: response["worker_version"]
            .as_str()
            .unwrap_or_default()
            .into(),
        backend_version: response["backend_version"]
            .as_str()
            .unwrap_or_default()
            .into(),
        python_version: response["python_version"]
            .as_str()
            .unwrap_or_default()
            .into(),
        limitations: DESIGN_LIMITATIONS.into(),
    };
    let design_bytes =
        serde_json::to_vec_pretty(&design).map_err(|e| artifact_invalid(e.to_string()))?;
    let normalized = match source.canonical() {
        Some(canonical) => serde_json::to_vec(canonical),
        None => serde_json::to_vec(source.spec()),
    }
    .map_err(|e| artifact_invalid(e.to_string()))?;
    let input_path = match source.manifest().source_format.as_deref() {
        Some("json") => "input.json",
        Some("yaml") => "input.yaml",
        _ => return Err(artifact_invalid("unsupported source format")),
    };
    pending.write(
        input_path,
        source.input(),
        "application/octet-stream",
        "archy-cli",
    )?;
    pending.write(
        "normalized.json",
        &normalized,
        "application/json",
        "archy-core",
    )?;
    if let Some(source_version) = source_profile_version {
        let profile = crate::run_integrity::profile_bytes(
            "export-ifc",
            DESIGN_MANIFEST_VERSION,
            source_version,
        )?;
        pending.write("profile.json", &profile, "application/json", "archy-core")?;
    }
    let variant_path = format!("variants/{}.json", variant.id);
    let evaluation_path = format!("evaluations/{}.json", variant.id);
    pending.write(
        &variant_path,
        &variant_bytes,
        "application/json",
        "archy-core",
    )?;
    pending.write(
        &evaluation_path,
        &evaluation_bytes,
        "application/json",
        "archy-eval",
    )?;
    pending.write(
        "ifc/model.ifc",
        model.bytes(),
        model.media_type(),
        model.producer(),
    )?;
    pending.write(
        "ifc/validation.json",
        validation.bytes(),
        validation.media_type(),
        validation.producer(),
    )?;
    pending.write(
        "ifc/response.json",
        outputs.response_bytes(),
        "application/json",
        "ifc-worker-v1",
    )?;
    pending.write(
        "design-manifest.json",
        &design_bytes,
        "application/json",
        "archy-cli",
    )?;
    pending.record_outcome(VariantOutcome {
        variant_id: variant.id.clone(),
        variant_artifact: Some(variant_path),
        evaluation_artifact: Some(evaluation_path),
        error_artifact: None,
    })?;
    let metadata = RunMetadata {
        project_id: source.spec().project_id.clone(),
        normalized_sha256: Some(sha256_hex(&normalized)),
        source_format: source.manifest().source_format.clone(),
        seed: None,
        count: None,
        parent_variant_id: Some(variant.id.clone()),
        algorithm_version: DESIGN_MANIFEST_VERSION.into(),
    };
    let receipt = pending.commit_checked(metadata, &|| reader.check_binding())?;
    verify_locked(&locked, &receipt.run_id)?;
    reader.check_binding().map_err(|_| {
        let mut error = ArchyError::new(
            archy_core::ErrorCode::RevisionConflict,
            "project",
            "Configured project changed during IFC export; commit may have occurred on the pinned root; retry the same idempotency key after restoring the project binding",
        );
        error.retriable = true;
        error
    })?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GenerateInput, GenerationMode, generate};
    use archy_core::ProjectStore;
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};

    fn copy_private_tree(source: &std::path::Path, destination: &std::path::Path) {
        fs::create_dir(destination).unwrap();
        fs::set_permissions(destination, fs::Permissions::from_mode(0o700)).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let dest = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_private_tree(&entry.path(), &dest);
            } else {
                fs::copy(entry.path(), dest).unwrap();
            }
        }
    }

    #[test]
    fn malformed_orphan_never_consumes_revision_or_retry_key() {
        use archy_core::{Artifact, MutationContext, ProjectStore, RunManifest, digest_request};
        let root = std::env::temp_dir().join(format!("archy-a12-orphan-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let project = root.join("project.json");
        let source = include_bytes!("../../../tests/fixtures/briefs/courtyard-v2.json");
        fs::write(&project, source).unwrap();
        let generation = generate(GenerateInput {
            project: project.clone(),
            mode: GenerationMode::Sample { count: 1, seed: 42 },
            mutation: MutationOptions::default(),
        })
        .unwrap();
        let variant = generation.variant_ids[0].clone();
        let key = "forged-orphan-approval";
        let request = RunRequest {
            schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
            context: MutationContext {
                idempotency_key: key.into(),
                expected_project_revision: generation.revision,
                destination: "runs".into(),
            },
            operation: "export-ifc".into(),
            request_bytes: serde_json::to_vec(&serde_json::json!({
                "source_run": generation.run_id, "variant_id": variant
            }))
            .unwrap(),
            input_sha256: sha256_hex(source),
            source_run: Some(generation.run_id.clone()),
        };
        let orphan_id = uuid::Uuid::new_v4().to_string();
        let orphan = root.join("runs").join(&orphan_id);
        copy_private_tree(&root.join("runs").join(&generation.run_id), &orphan);
        let mut manifest: RunManifest =
            serde_json::from_slice(&fs::read(orphan.join("manifest.json")).unwrap()).unwrap();
        let forged = br#"{"approval_state":"APPROVED","certified_for_construction":true}"#;
        fs::write(orphan.join("design-manifest.json"), forged).unwrap();
        manifest.artifacts.push(Artifact {
            path: "design-manifest.json".into(),
            sha256: sha256_hex(forged),
            media_type: "application/json".into(),
            producer: "forged-worker".into(),
            byte_len: forged.len() as u64,
        });
        manifest.run_id = orphan_id.clone();
        manifest.idempotency_key = key.into();
        manifest.request_sha256 = digest_request(&request).unwrap();
        manifest.request_schema_version = RUN_REQUEST_SCHEMA_VERSION.into();
        manifest.expected_project_revision = generation.revision;
        manifest.operation = "export-ifc".into();
        manifest.parent_run_id = Some(generation.run_id.clone());
        manifest.parent_variant_id = Some(variant.clone());
        manifest.algorithm_version = DESIGN_MANIFEST_VERSION.into();
        fs::write(
            orphan.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let store = ProjectStore::open(&project).unwrap();
        let before = store.lock().unwrap().receipts().unwrap();
        let input = ExportIfcInput {
            project: project.clone(),
            source_run: Some(generation.run_id),
            variant_id: variant,
            mutation: MutationOptions {
                idempotency_key: Some(key.into()),
                expected_project_revision: Some(generation.revision),
                destination: "runs".into(),
            },
        };
        let rejected =
            export_with(input.clone(), &|_| panic!("forged orphan reached worker")).unwrap_err();
        assert_eq!(rejected.code, archy_core::ErrorCode::ArtifactInvalid);
        assert_eq!(store.lock().unwrap().receipts().unwrap(), before);
        fs::remove_dir_all(&orphan).unwrap();
        let called = std::cell::Cell::new(false);
        let retry = export_with(input, &|_| {
            called.set(true);
            Err(ArchyError::new(
                archy_core::ErrorCode::BackendUnavailable,
                "worker",
                "retry reached worker",
            ))
        })
        .unwrap_err();
        assert!(called.get());
        assert_eq!(retry.code, archy_core::ErrorCode::BackendUnavailable);
        assert_eq!(store.lock().unwrap().receipts().unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_valid_export_orphan_replays_exact_key() {
        let Ok(python) = std::env::var("ARCHY_IFC_TEST_PYTHON") else {
            return;
        };
        let root =
            std::env::temp_dir().join(format!("archy-a12-valid-orphan-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let project = root.join("project.json");
        fs::write(
            &project,
            include_bytes!("../../../tests/fixtures/briefs/courtyard-v2.json"),
        )
        .unwrap();
        let generated = generate(GenerateInput {
            project: project.clone(),
            mode: GenerationMode::Sample { count: 1, seed: 42 },
            mutation: MutationOptions::default(),
        })
        .unwrap();
        let input = ExportIfcInput {
            project: project.clone(),
            source_run: Some(generated.run_id),
            variant_id: generated.variant_ids[0].clone(),
            mutation: MutationOptions {
                idempotency_key: Some("valid-orphan".into()),
                expected_project_revision: Some(generated.revision),
                destination: "runs".into(),
            },
        };
        let job = root.join("worker");
        fs::create_dir(&job).unwrap();
        let receipt = export_with(input.clone(), &|request| {
            native(request, &job, &python, None)
        })
        .unwrap();
        let run_dir = root.join("runs").join(&receipt.run_id);
        let original_manifest = fs::read(run_dir.join("manifest.json")).unwrap();
        let original_design = fs::read(run_dir.join("design-manifest.json")).unwrap();
        let revision_file = root.join(format!(".archy/revisions/{:020}.json", receipt.revision));
        fs::remove_file(revision_file).unwrap();
        let store = ProjectStore::open(&project).unwrap();
        assert_eq!(
            store.lock().unwrap().revision().unwrap(),
            generated.revision
        );
        let accepted = store.lock().unwrap().receipts().unwrap();
        let mut design: serde_json::Value = serde_json::from_slice(&original_design).unwrap();
        design["approval_state"] = "APPROVED".into();
        let forged_design = serde_json::to_vec(&design).unwrap();
        fs::write(run_dir.join("design-manifest.json"), &forged_design).unwrap();
        let mut manifest: archy_core::RunManifest =
            serde_json::from_slice(&original_manifest).unwrap();
        let claim = manifest
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.path == "design-manifest.json")
            .unwrap();
        claim.sha256 = sha256_hex(&forged_design);
        claim.byte_len = forged_design.len() as u64;
        fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let rejected =
            export_with(input.clone(), &|_| panic!("forged orphan reached worker")).unwrap_err();
        assert_eq!(rejected.code, archy_core::ErrorCode::ArtifactInvalid);
        assert_eq!(
            store.lock().unwrap().revision().unwrap(),
            generated.revision
        );
        assert_eq!(store.lock().unwrap().receipts().unwrap(), accepted);
        fs::write(run_dir.join("design-manifest.json"), original_design).unwrap();
        fs::write(run_dir.join("manifest.json"), original_manifest).unwrap();
        let replay =
            export_with(input.clone(), &|_| panic!("orphan replay reached worker")).unwrap();
        assert_eq!(replay, receipt);
        assert_eq!(store.lock().unwrap().revision().unwrap(), receipt.revision);
        assert_eq!(
            export_with(input, &|_| panic!("exact retry reached worker")).unwrap(),
            receipt
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canonical_v2_export_reaches_worker_without_publication_on_failure() {
        let root = std::env::temp_dir().join(format!("archy-a12-v2-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let project = root.join("project.json");
        fs::write(
            &project,
            include_bytes!("../../../tests/fixtures/briefs/courtyard-v2.json"),
        )
        .unwrap();
        let generation = generate(GenerateInput {
            project: project.clone(),
            mode: GenerationMode::Sample { count: 1, seed: 42 },
            mutation: MutationOptions::default(),
        })
        .unwrap();
        let verified_source = crate::verify_run(&project, &generation.run_id).unwrap();
        assert!(verified_source.canonical().is_some());
        assert_eq!(verified_source.source_schema_version(), Some("0.2.0"));
        let called = std::cell::Cell::new(false);
        let error = export_with(
            ExportIfcInput {
                project: project.clone(),
                source_run: Some(generation.run_id),
                variant_id: generation.variant_ids[0].clone(),
                mutation: MutationOptions::default(),
            },
            &|_| {
                called.set(true);
                Err(ArchyError::new(
                    archy_core::ErrorCode::BackendUnavailable,
                    "worker",
                    "fixture worker unavailable",
                ))
            },
        )
        .unwrap_err();
        assert!(called.get());
        assert_eq!(error.code, archy_core::ErrorCode::BackendUnavailable);
        assert_eq!(
            ProjectStore::open(&project)
                .unwrap()
                .lock()
                .unwrap()
                .revision()
                .unwrap(),
            generation.revision
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepted_historical_v1_export_reaches_worker_without_publication_on_failure() {
        let root = std::env::temp_dir().join(format!("archy-a12-v1-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let project = root.join("project.json");
        fs::write(
            &project,
            include_bytes!("../../../tests/fixtures/briefs/legacy-adversarial-preferences.json"),
        )
        .unwrap();
        let capture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/legacy-preference-run");
        copy_private_tree(&capture.join(".archy"), &root.join(".archy"));
        copy_private_tree(&capture.join("archived-runs"), &root.join("runs"));
        let generation: crate::WorkflowReceipt =
            serde_json::from_slice(&fs::read(capture.join("receipt.json")).unwrap()).unwrap();
        let called = std::cell::Cell::new(false);
        let error = export_with(
            ExportIfcInput {
                project: project.clone(),
                source_run: Some(generation.run_id),
                variant_id: generation.variant_ids[0].clone(),
                mutation: MutationOptions::default(),
            },
            &|_| {
                called.set(true);
                Err(ArchyError::new(
                    archy_core::ErrorCode::BackendUnavailable,
                    "worker",
                    "fixture worker unavailable",
                ))
            },
        )
        .unwrap_err();
        assert!(called.get());
        assert_eq!(error.code, archy_core::ErrorCode::BackendUnavailable);
        assert_eq!(
            ProjectStore::open(&project)
                .unwrap()
                .lock()
                .unwrap()
                .revision()
                .unwrap(),
            generation.revision
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn native(
        request: &IfcRequest,
        root: &std::path::Path,
        python: &str,
        tamper: Option<&str>,
    ) -> Result<CapturedOutputs, ArchyError> {
        fs::create_dir(root.join("output")).unwrap();
        let source = root.join("request.json");
        fs::write(&source, request.to_bytes()?).unwrap();
        let process = Command::new(python)
            .args(["-I", "-c", "import sys; sys.path.insert(0, sys.argv.pop(1)); from archy_ifc_worker.worker import main; raise SystemExit(main())"])
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workers/ifc"))
            .arg("--project-root")
            .arg(root)
            .arg("--request")
            .arg(&source)
            .output()
            .unwrap();
        assert_eq!(
            process.status.code(),
            Some(0),
            "{} {}",
            String::from_utf8_lossy(&process.stdout),
            String::from_utf8_lossy(&process.stderr)
        );
        if let Some(kind) = tamper {
            let output = root.join("output/ifc");
            let original = String::from_utf8(fs::read(output.join("model.ifc")).unwrap()).unwrap();
            let model = match kind {
                "invalid" => b"captured:model.ifc".to_vec(),
                "approval" => original.replacen("IFCLABEL('DRAFT')", "IFCLABEL('APPROVED')", 1).into_bytes(),
                "polyglot" => [original.as_bytes(), b"\n<script>alert(1)</script>\n"].concat(),
                "external" => original.replacen("ENDSEC;\nEND-ISO-10303-21;", "#999=IFCEXTERNALREFERENCE('https://attacker.example/ifc',$,$);\nENDSEC;\nEND-ISO-10303-21;", 1).into_bytes(),
                "header_comment" => original.replacen("HEADER;", "HEADER;/* Approved for construction */", 1).into_bytes(),
                "data_comment" => original.replacen("DATA;", "DATA;/* Certified */", 1).into_bytes(),
                "post_terminator_comment" => [original.as_bytes(), b"/* <script>alert(1)</script> */"].concat(),
                "unused_allowed_entity" => original.replacen("ENDSEC;\nEND-ISO-10303-21;", "#999=IFCCARTESIANPOINT((1.,2.,3.));\nENDSEC;\nEND-ISO-10303-21;", 1).into_bytes(),
                "duplicate_approval_first" => original.replacen("#95,#96,#97", "#999,#95,#96,#97", 1).replacen("ENDSEC;\nEND-ISO-10303-21;", "#999=IFCPROPERTYSINGLEVALUE('ApprovalState',$,IFCLABEL('APPROVED'),$);\nENDSEC;\nEND-ISO-10303-21;", 1).into_bytes(),
                "duplicate_approval_same" => original.replacen("#95,#96,#97", "#999,#95,#96,#97", 1).replacen("ENDSEC;\nEND-ISO-10303-21;", "#999=IFCPROPERTYSINGLEVALUE('ApprovalState',$,IFCLABEL('DRAFT'),$);\nENDSEC;\nEND-ISO-10303-21;", 1).into_bytes(),
                "duplicate_area_first" => original.replacen("(#110,#111,#112)", "(#999,#110,#111,#112)", 1).replacen("ENDSEC;\nEND-ISO-10303-21;", "#999=IFCQUANTITYAREA('FootprintArea',$,#7,999.,'wrong');\nENDSEC;\nEND-ISO-10303-21;", 1).into_bytes(),
                "extra_context" => original.replacen("(#14),#9", "(#14,#999),#9", 1).replacen("ENDSEC;\nEND-ISO-10303-21;", "#999=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#13,$);\nENDSEC;\nEND-ISO-10303-21;", 1).into_bytes(),
                "context_true_north" => original.replacen("1.E-05,#13,$);", "1.E-05,#13,#999);", 1).replacen("ENDSEC;\nEND-ISO-10303-21;", "#999=IFCDIRECTION((1.,0.));\nENDSEC;\nEND-ISO-10303-21;", 1).into_bytes(),
                _ => panic!("unknown tamper kind"),
            };
            assert_ne!(model, original.as_bytes(), "tamper did not change model");
            fs::write(output.join("model.ifc"), &model).unwrap();
            let response_path = output.join("response.json");
            let mut response: serde_json::Value =
                serde_json::from_slice(&fs::read(&response_path).unwrap()).unwrap();
            let claim = response["artifacts"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|item| item["path"] == "model.ifc")
                .unwrap();
            claim["sha256"] = sha256_hex(&model).into();
            claim["size_bytes"] = model.len().into();
            fs::write(response_path, serde_json::to_vec(&response).unwrap()).unwrap();
        }
        let scratch = fs::File::open(root.join("output")).unwrap();
        match capture_ifc(&scratch, request)? {
            CaptureOutcome::Success(outputs) => Ok(outputs),
            CaptureOutcome::WorkerError(error) => Err(error.diagnostic_error().clone()),
        }
    }

    #[test]
    fn native_canonical_v2_json_yaml_rectangle_l_export_and_replay() {
        let Ok(python) = std::env::var("ARCHY_IFC_TEST_PYTHON") else {
            return;
        };
        for format in ["json", "yaml"] {
            for shape in ["rectangle", "l_shape"] {
                let root = std::env::temp_dir()
                    .join(format!("archy-a12-current-{}", uuid::Uuid::new_v4()));
                fs::create_dir(&root).unwrap();
                let project = root.join(format!("project.{format}"));
                let source = if format == "json" {
                    let mut value: serde_json::Value = serde_json::from_slice(include_bytes!(
                        "../../../tests/fixtures/briefs/courtyard-v2.json"
                    ))
                    .unwrap();
                    value["generation"]["shapes"] = serde_json::json!([shape]);
                    serde_json::to_vec(&value).unwrap()
                } else {
                    include_str!("../../../tests/fixtures/briefs/courtyard-v2.yaml")
                        .replace("[rectangle, l_shape]", &format!("[{shape}]"))
                        .into_bytes()
                };
                fs::write(&project, &source).unwrap();
                let generation = generate(GenerateInput {
                    project: project.clone(),
                    mode: GenerationMode::Sample { count: 2, seed: 42 },
                    mutation: MutationOptions::default(),
                })
                .unwrap();
                assert_eq!(generation.variant_ids.len(), 2);
                let variant_id = generation.variant_ids[1].clone();
                let input = ExportIfcInput {
                    project: project.clone(),
                    source_run: Some(generation.run_id.clone()),
                    variant_id: variant_id.clone(),
                    mutation: MutationOptions {
                        idempotency_key: Some(format!("v2-{format}-{shape}")),
                        expected_project_revision: Some(generation.revision),
                        destination: "runs".into(),
                    },
                };
                if format == "yaml" && shape == "l_shape" {
                    let job = root.join("tamper");
                    fs::create_dir(&job).unwrap();
                    let error = export_with(input.clone(), &|request| {
                        native(request, &job, &python, Some("approval"))
                    })
                    .unwrap_err();
                    assert_eq!(error.code, archy_core::ErrorCode::ArtifactInvalid);
                    assert_eq!(
                        fs::read_dir(root.join(".archy/staging")).unwrap().count(),
                        0
                    );
                    assert_eq!(
                        ProjectStore::open(&project)
                            .unwrap()
                            .lock()
                            .unwrap()
                            .revision()
                            .unwrap(),
                        generation.revision
                    );
                }
                let job = root.join("valid");
                fs::create_dir(&job).unwrap();
                let receipt = export_with(input.clone(), &|request| {
                    native(request, &job, &python, None)
                })
                .unwrap();
                let verified = crate::verify_run(&project, &receipt.run_id).unwrap();
                assert!(verified.canonical().is_some());
                assert_eq!(verified.input(), source);
                assert_eq!(verified.variants()[0].id, variant_id);
                let normalized_digest = crate::verify_run(&project, &generation.run_id)
                    .unwrap()
                    .manifest()
                    .normalized_sha256
                    .clone()
                    .unwrap();
                let stored = ProjectStore::open(&project).unwrap();
                let locked = stored.lock().unwrap();
                let run = locked.read_run(&receipt.run_id).unwrap();
                let design: DesignManifest =
                    serde_json::from_slice(run.artifact("design-manifest.json").unwrap()).unwrap();
                assert_eq!(design.approval_state, "DRAFT");
                assert_eq!(design.source_input_sha256, sha256_hex(&source));
                assert_eq!(design.source_normalized_sha256, normalized_digest);
                assert_eq!(
                    design.model_sha256,
                    sha256_hex(run.artifact("ifc/model.ifc").unwrap())
                );
                assert!(run.artifact("profile.json").is_some());
                drop(locked);
                assert_eq!(
                    export_with(input, &|_| panic!("replay reached worker")).unwrap(),
                    receipt
                );
                fs::remove_dir_all(root).unwrap();
            }
        }
    }

    #[test]
    fn native_historical_v1_json_yaml_rectangle_l_preserves_source_bytes() {
        let Ok(python) = std::env::var("ARCHY_IFC_TEST_PYTHON") else {
            return;
        };
        for format in ["json", "yaml"] {
            for shape in ["rectangle", "l_shape"] {
                let root =
                    std::env::temp_dir().join(format!("archy-a12-legacy-{}", uuid::Uuid::new_v4()));
                fs::create_dir(&root).unwrap();
                let project = root.join(format!("project.{format}"));
                let source = if format == "json" {
                    let mut value: serde_json::Value = serde_json::from_slice(include_bytes!(
                        "../../../tests/fixtures/briefs/courtyard.json"
                    ))
                    .unwrap();
                    value["generation"]["shapes"] = serde_json::json!([shape]);
                    serde_json::to_vec(&value).unwrap()
                } else {
                    include_str!("../../../tests/fixtures/briefs/courtyard-v1.yaml")
                        .replace("[rectangle, l_shape]", &format!("[{shape}]"))
                        .into_bytes()
                };
                fs::write(&project, &source).unwrap();
                let spec = archy_core::parse_project_v1(
                    std::str::from_utf8(&source).unwrap(),
                    if format == "json" {
                        archy_core::InputFormat::Json
                    } else {
                        archy_core::InputFormat::Yaml
                    },
                )
                .unwrap();
                let generation = archy_generate::generate_variants_v1(&spec, 2, 42).unwrap();
                assert_eq!(generation.variants.len(), 2);
                let evaluations = generation
                    .variants
                    .iter()
                    .map(|variant| archy_eval::evaluate_variant(&spec, variant).unwrap())
                    .collect::<Vec<_>>();
                let store = ProjectStore::open(&project).unwrap();
                let locked = store.lock().unwrap();
                let request = RunRequest {
                    schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
                    context: MutationOptions::default().resolve(&locked).unwrap(),
                    operation: "generate".into(),
                    request_bytes: serde_json::to_vec(&serde_json::json!({
                        "mode":"sample", "count":2, "seed":42,
                    }))
                    .unwrap(),
                    input_sha256: sha256_hex(&source),
                    source_run: None,
                };
                let archy_core::BeginResult::Pending(pending) = locked.begin(request).unwrap()
                else {
                    panic!("new historical run replayed")
                };
                let receipt = crate::publish(
                    pending,
                    RunMetadata {
                        project_id: spec.project_id.clone(),
                        normalized_sha256: Some(sha256_hex(&serde_json::to_vec(&spec).unwrap())),
                        source_format: Some(format.into()),
                        seed: Some(42),
                        count: Some(2),
                        parent_variant_id: None,
                        algorithm_version: generation.algorithm_version.clone(),
                    },
                    crate::RunPayload {
                        operation: "generate",
                        parent_run_id: None,
                        format,
                        input: &source,
                        spec: &spec,
                        canonical: None,
                        generation: Some(&generation),
                        params: None,
                        variants: &generation.variants,
                        evaluations: &evaluations,
                    },
                    None,
                )
                .unwrap();
                let revision = receipt.revision;
                drop(locked);
                let verified_source = crate::verify_run(&project, &receipt.run_id).unwrap();
                assert!(verified_source.canonical().is_none());
                assert_eq!(verified_source.source_schema_version(), None);
                let variant_id = generation.variants[1].id.clone();
                let input = ExportIfcInput {
                    project: project.clone(),
                    source_run: Some(receipt.run_id),
                    variant_id: variant_id.clone(),
                    mutation: MutationOptions {
                        idempotency_key: Some(format!("v1-{format}-{shape}")),
                        expected_project_revision: Some(revision),
                        destination: "runs".into(),
                    },
                };
                let job = root.join("valid");
                fs::create_dir(&job).unwrap();
                let exported = export_with(input.clone(), &|request| {
                    native(request, &job, &python, None)
                })
                .unwrap();
                let verified = crate::verify_run(&project, &exported.run_id).unwrap();
                assert!(verified.canonical().is_none());
                assert_eq!(verified.input(), source);
                assert_eq!(verified.variants()[0].id, variant_id);
                let locked = store.lock().unwrap();
                let stored = locked.read_run(&exported.run_id).unwrap();
                assert_eq!(
                    stored.artifact(if format == "json" {
                        "input.json"
                    } else {
                        "input.yaml"
                    }),
                    Some(source.as_slice())
                );
                assert!(stored.artifact("profile.json").is_none());
                drop(locked);
                assert_eq!(
                    export_with(input, &|_| panic!("replay reached worker")).unwrap(),
                    exported
                );
                fs::remove_dir_all(root).unwrap();
            }
        }
    }

    #[test]
    fn native_selected_export_publishes_verified_draft_and_replays() {
        let Ok(python) = std::env::var("ARCHY_IFC_TEST_PYTHON") else {
            return;
        };
        for shape in ["l_shape"] {
            let root = std::env::temp_dir().join(format!("archy-a12-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let project = root.join("project.json");
            fs::write(
                &project,
                include_bytes!(
                    "../../../tests/fixtures/briefs/legacy-adversarial-preferences.json"
                ),
            )
            .unwrap();
            let capture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/legacy-preference-run");
            copy_private_tree(&capture.join(".archy"), &root.join(".archy"));
            copy_private_tree(&capture.join("archived-runs"), &root.join("runs"));
            let generation: crate::WorkflowReceipt =
                serde_json::from_slice(&fs::read(capture.join("receipt.json")).unwrap()).unwrap();
            assert!(
                crate::verify_run(&project, &generation.run_id)
                    .unwrap()
                    .canonical()
                    .is_none()
            );
            let variant_id = generation.variant_ids[0].clone();
            let export_input = ExportIfcInput {
                project: project.clone(),
                source_run: Some(generation.run_id.clone()),
                variant_id: variant_id.clone(),
                mutation: MutationOptions {
                    idempotency_key: Some("native-ifc-1".into()),
                    expected_project_revision: Some(generation.revision),
                    destination: "runs".into(),
                },
            };
            let failed = export_with(export_input.clone(), &|_| {
                Err(ArchyError::new(
                    archy_core::ErrorCode::BackendUnavailable,
                    "worker",
                    "development failure",
                ))
            })
            .unwrap_err();
            assert_eq!(failed.code, archy_core::ErrorCode::BackendUnavailable);
            assert_eq!(
                archy_core::ProjectStore::open(&project)
                    .unwrap()
                    .lock()
                    .unwrap()
                    .revision()
                    .unwrap(),
                generation.revision
            );
            let mut bad_path = export_input.clone();
            bad_path.mutation.destination = "../outside".into();
            assert!(
                export_with(bad_path, &|_| panic!("unsafe destination reached worker")).is_err()
            );
            let fake_job = root.join("fake-success");
            fs::create_dir(&fake_job).unwrap();
            let fake = export_with(export_input.clone(), &|request| {
                native(request, &fake_job, &python, Some("invalid"))
            })
            .unwrap_err();
            assert_eq!(fake.code, archy_core::ErrorCode::ArtifactInvalid);
            assert_eq!(
                fs::read_dir(root.join(".archy/staging")).unwrap().count(),
                0
            );
            assert_eq!(
                archy_core::ProjectStore::open(&project)
                    .unwrap()
                    .lock()
                    .unwrap()
                    .revision()
                    .unwrap(),
                generation.revision
            );
            for kind in [
                "approval",
                "polyglot",
                "external",
                "header_comment",
                "data_comment",
                "post_terminator_comment",
                "unused_allowed_entity",
                "duplicate_approval_first",
                "duplicate_approval_same",
                "duplicate_area_first",
                "extra_context",
                "context_true_north",
            ] {
                let job = root.join(format!("fake-{kind}"));
                fs::create_dir(&job).unwrap();
                let error = export_with(export_input.clone(), &|request| {
                    native(request, &job, &python, Some(kind))
                })
                .unwrap_err();
                assert_eq!(error.code, archy_core::ErrorCode::ArtifactInvalid, "{kind}");
                assert_eq!(
                    fs::read_dir(root.join(".archy/staging")).unwrap().count(),
                    0,
                    "{kind} left staging"
                );
                assert_eq!(
                    archy_core::ProjectStore::open(&project)
                        .unwrap()
                        .lock()
                        .unwrap()
                        .revision()
                        .unwrap(),
                    generation.revision,
                    "{kind} changed accepted revision"
                );
            }
            let attempts = std::cell::Cell::new(0);
            let result = export_with(export_input.clone(), &|request| {
                attempts.set(attempts.get() + 1);
                let job = root.join(format!("job-{}", attempts.get()));
                fs::create_dir(&job).unwrap();
                native(request, &job, &python, None)
            });
            let receipt = result.unwrap();
            assert_eq!(attempts.get(), 1);
            let stored = archy_core::ProjectStore::open(&project).unwrap();
            let locked = stored.lock().unwrap();
            let run = locked.read_run(&receipt.run_id).unwrap();
            let design: DesignManifest =
                serde_json::from_slice(run.artifact("design-manifest.json").unwrap()).unwrap();
            assert_eq!(design.approval_state, "DRAFT");
            assert_eq!(design.scope, "SCHEMATIC_MASSING_ONLY");
            assert_eq!(
                design.model_sha256,
                sha256_hex(run.artifact("ifc/model.ifc").unwrap())
            );
            assert_eq!(design.variant_id, variant_id);
            let exported_variant: serde_json::Value = serde_json::from_slice(
                run.artifact(&format!("variants/{variant_id}.json"))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(exported_variant["shape"], shape);
            if let Ok(node_path) = std::env::var("ARCHY_IFC_READER_NODE_PATH") {
                let source = locked.read_run(&generation.run_id).unwrap();
                let source_input: serde_json::Value =
                    serde_json::from_slice(source.artifact("input.json").unwrap()).unwrap();
                let variant = source
                    .artifact(&format!("variants/{}.json", variant_id))
                    .unwrap();
                let variant: serde_json::Value = serde_json::from_slice(variant).unwrap();
                let points = variant["footprint_xy_m"].as_array().unwrap();
                let mut twice_area = 0.0;
                for pair in points.windows(2) {
                    twice_area += pair[0][0].as_f64().unwrap() * pair[1][1].as_f64().unwrap()
                        - pair[1][0].as_f64().unwrap() * pair[0][1].as_f64().unwrap();
                }
                let area = twice_area.abs() / 2.0;
                let storeys = variant["storeys"].as_u64().unwrap();
                let floor = variant["floor_to_floor_m"].as_f64().unwrap();
                let expected = serde_json::json!({"schema_version":"ifc-reader-fixture-v1", "cases":[{
                    "file":"stored.ifc", "source_file":"input.json", "source_mode":"accepted_project_json_v1",
                    "shape": variant["shape"], "project_id": design.project_id,
                    "variant_id": variant_id, "run_id": generation.run_id,
                    "source_digest": design.source_input_sha256,
                    "origin_xy_m": variant["parameters"]["origin_xy_m"],
                    "width_m": variant["parameters"]["width_m"],
                    "depth_m": variant["parameters"]["depth_m"],
                    "notch_width_m": variant["parameters"]["notch_width_m"],
                    "notch_depth_m": variant["parameters"]["notch_depth_m"],
                    "north_angle_deg": source_input["site"]["north_deg"],
                    "footprint_xy_m": variant["footprint_xy_m"],
                    "footprint_area_m2": area, "approximate_gfa_m2": area * storeys as f64,
                    "mass_volume_m3": area * storeys as f64 * floor,
                    "storeys": storeys, "floor_to_floor_m": floor, "height_m": storeys as f64 * floor
                }]});
                let expected_path = root.join("expected.json");
                let ifc_path = root.join("stored.ifc");
                let source_path = root.join("input.json");
                fs::write(&expected_path, serde_json::to_vec(&expected).unwrap()).unwrap();
                fs::write(&ifc_path, run.artifact("ifc/model.ifc").unwrap()).unwrap();
                fs::write(&source_path, source.artifact("input.json").unwrap()).unwrap();
                let reader = concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/backend/ifc-reader/verify.cjs"
                );
                let output = Command::new("node")
                    .env("NODE_PATH", node_path)
                    .args([
                        reader,
                        ifc_path.to_str().unwrap(),
                        expected_path.to_str().unwrap(),
                        "stored.ifc",
                        source_path.to_str().unwrap(),
                    ])
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "independent reader: {} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["status"],
                    "ok"
                );
            }
            drop(locked);
            assert_eq!(
                export_with(export_input, &|_| panic!("replay must not run worker")).unwrap(),
                receipt
            );
            assert!(stored.lock().unwrap().read_run(&generation.run_id).is_ok());
            fs::remove_dir_all(root).unwrap();
        }
    }
}
