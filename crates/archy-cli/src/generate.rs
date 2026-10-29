use crate::{
    GenerateInput, GenerationMode, RunPayload, WorkflowReceipt, artifact_invalid, input_format,
    publish, recorded_manifest_for_key, request_conflict, run_integrity::verify_locked,
    workflow_receipt,
};
use archy_core::{
    ArchyError, ErrorCode, RUN_REQUEST_SCHEMA_VERSION, RunMetadata, RunRequest, ShapeParameters,
    Variant, parse_project, sha256_hex,
};
use archy_eval::evaluate_variant;
use archy_generate::{ALGORITHM_VERSION, generate_variants};
use serde::{Deserialize, Serialize};

/// Editable geometry and storey inputs only; IDs, footprints and check flags are derived.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditParameters {
    pub parameters: ShapeParameters,
    pub storeys: u32,
    pub floor_to_floor_m: f64,
}

pub fn generate(input: GenerateInput) -> Result<WorkflowReceipt, ArchyError> {
    let store = archy_core::ProjectStore::open(&input.project)?;
    let locked = store.lock()?;
    let context = input.mutation.resolve(&locked)?;
    let recorded = recorded_manifest_for_key(&locked, input.mutation.idempotency_key.as_deref())?;
    match input.mode {
        GenerationMode::Sample { count, seed } => {
            if recorded
                .as_ref()
                .is_some_and(|run| run.operation != "generate")
            {
                return Err(request_conflict());
            }
            let (format, source_format, _) = input_format(&input.project)?;
            let raw = locked.read_source()?;
            let request_bytes =
                serde_json::to_vec(&serde_json::json!({"mode":"sample","count":count,"seed":seed}))
                    .map_err(|e| {
                        ArchyError::new(ErrorCode::InternalError, "request", e.to_string())
                    })?;
            let request = RunRequest {
                schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
                context,
                operation: "generate".into(),
                request_bytes,
                input_sha256: sha256_hex(&raw),
                source_run: None,
            };
            let pending = match locked.begin(request)? {
                archy_core::BeginResult::Replay(receipt) => {
                    let verified = verify_locked(&locked, &receipt.run_id)?;
                    return Ok(workflow_receipt(receipt, &verified));
                }
                archy_core::BeginResult::Pending(pending) => pending,
            };
            let source = std::str::from_utf8(&raw)
                .map_err(|_| ArchyError::schema("project", "Project is not UTF-8"))?;
            let spec = parse_project(source, format)?;
            let generation = generate_variants(&spec, count, seed)?;
            let evaluations = generation
                .variants
                .iter()
                .map(|variant| evaluate_variant(&spec, variant))
                .collect::<Result<Vec<_>, _>>()?;
            let normalized = serde_json::to_vec(&spec).map_err(|e| {
                ArchyError::new(ErrorCode::InternalError, "normalized", e.to_string())
            })?;
            let metadata = RunMetadata {
                project_id: spec.project_id.clone(),
                normalized_sha256: Some(sha256_hex(&normalized)),
                source_format: Some(source_format.into()),
                seed: Some(seed),
                count: Some(count),
                parent_variant_id: None,
                algorithm_version: ALGORITHM_VERSION.into(),
            };
            let receipt = publish(
                pending,
                metadata,
                RunPayload {
                    operation: "generate",
                    parent_run_id: None,
                    format: source_format,
                    input: &raw,
                    spec: &spec,
                    generation: Some(&generation),
                    params: None,
                    variants: &generation.variants,
                    evaluations: &evaluations,
                },
            )?;
            let verified = verify_locked(&locked, &receipt.run_id)?;
            Ok(workflow_receipt(receipt, &verified))
        }
        GenerationMode::Edit {
            from_run,
            variant,
            params,
        } => {
            if let Some(run) = &recorded
                && (run.operation != "edit"
                    || run.parent_run_id.as_deref() != Some(&from_run)
                    || run.parent_variant_id.as_deref() != Some(&variant))
            {
                return Err(request_conflict());
            }
            let params_bytes =
                locked.read_project_file(&params, archy_core::validation::MAX_INPUT_BYTES)?;
            let request_bytes = serde_json::to_vec(&serde_json::json!({"mode":"edit","from_run":from_run,"variant":variant,"params_sha256":sha256_hex(&params_bytes)}))
                .map_err(|e| ArchyError::new(ErrorCode::InternalError, "request", e.to_string()))?;
            let request = RunRequest {
                schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
                context,
                operation: "edit".into(),
                request_bytes,
                input_sha256: match &recorded {
                    Some(run) => run.input_sha256.clone(),
                    None => locked.read_run(&from_run)?.manifest().input_sha256.clone(),
                },
                source_run: Some(from_run.clone()),
            };
            let pending = match locked.begin(request)? {
                archy_core::BeginResult::Replay(receipt) => {
                    let verified = verify_locked(&locked, &receipt.run_id)?;
                    return Ok(workflow_receipt(receipt, &verified));
                }
                archy_core::BeginResult::Pending(pending) => pending,
            };
            let parent = verify_locked(&locked, &from_run)?;
            if !matches!(parent.manifest().operation.as_str(), "generate" | "edit") {
                return Err(artifact_invalid(
                    "edit source must be a generation or edit run",
                ));
            }
            let parent_variant = parent
                .variants()
                .iter()
                .find(|v| v.id == variant)
                .ok_or_else(|| {
                    ArchyError::schema("variant", "Variant is absent from source run")
                })?;
            let edit: EditParameters = serde_json::from_slice(&params_bytes)
                .map_err(|e| ArchyError::schema("params", e.to_string()))?;
            let derived = Variant::new(
                parent.spec().project_id.clone(),
                parent_variant.seed,
                ALGORITHM_VERSION.into(),
                edit.parameters,
                edit.storeys,
                edit.floor_to_floor_m,
            )?;
            if derived.id == parent_variant.id {
                return Err(ArchyError::schema(
                    "params",
                    "Edit must change canonical geometry or storey inputs",
                ));
            }
            let evaluations = vec![evaluate_variant(parent.spec(), &derived)?];
            let variants = vec![derived];
            let normalized = serde_json::to_vec(parent.spec()).map_err(|e| {
                ArchyError::new(ErrorCode::InternalError, "normalized", e.to_string())
            })?;
            let source_format = parent
                .manifest()
                .source_format
                .as_deref()
                .ok_or_else(|| artifact_invalid("missing source format"))?;
            let metadata = RunMetadata {
                project_id: parent.spec().project_id.clone(),
                normalized_sha256: Some(sha256_hex(&normalized)),
                source_format: Some(source_format.into()),
                seed: Some(parent_variant.seed),
                count: Some(1),
                parent_variant_id: Some(variant.clone()),
                algorithm_version: ALGORITHM_VERSION.into(),
            };
            let receipt = publish(
                pending,
                metadata,
                RunPayload {
                    operation: "edit",
                    parent_run_id: Some(&from_run),
                    format: source_format,
                    input: parent.input(),
                    spec: parent.spec(),
                    generation: None,
                    params: Some(&params_bytes),
                    variants: &variants,
                    evaluations: &evaluations,
                },
            )?;
            let verified = verify_locked(&locked, &receipt.run_id)?;
            Ok(workflow_receipt(receipt, &verified))
        }
    }
}
