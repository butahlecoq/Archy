use crate::{
    GenerateInput, GenerationMode, MutationOptions, RunPayload, WorkflowReceipt, artifact_invalid,
    begin_with_binding, input_format, publish, recorded_manifest_for_key, request_conflict,
    run_integrity::{migrate_claimed_v1_source, verify_locked},
    workflow_receipt,
};
use archy_core::{
    ArchyError, ErrorCode, InputFormat, LockedProject, RUN_REQUEST_SCHEMA_VERSION, RunMetadata,
    RunRequest, ShapeParameters, Variant, parse_project_current, sha256_hex,
};
use archy_eval::{evaluate_variant_v1, evaluate_variant_v2};
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
    let (format, source_format, _) = input_format(&input.project)?;
    generate_locked(
        &locked,
        input.mode,
        input.mutation,
        format,
        source_format,
        None,
        None,
    )
}

pub(crate) fn generate_locked(
    locked: &LockedProject<'_>,
    mode: GenerationMode,
    mutation: MutationOptions,
    format: InputFormat,
    source_format: &str,
    pinned_source: Option<&[u8]>,
    check: Option<&dyn Fn() -> Result<(), ArchyError>>,
) -> Result<WorkflowReceipt, ArchyError> {
    let context = mutation.resolve(locked)?;
    let recorded = recorded_manifest_for_key(locked, mutation.idempotency_key.as_deref())?;
    match mode {
        GenerationMode::Sample { count, seed } => {
            if recorded.is_none() && !(1..=1000).contains(&count) {
                return Err(ArchyError::schema(
                    "count",
                    "Requested count must be 1..1000",
                ));
            }
            if recorded
                .as_ref()
                .is_some_and(|run| run.operation != "generate")
            {
                return Err(request_conflict());
            }
            let raw = match pinned_source {
                Some(bytes) => bytes.to_vec(),
                None => locked.read_source()?,
            };
            let legacy_replay = match &recorded {
                Some(run) => verify_locked(locked, &run.run_id)?.canonical().is_none(),
                None => false,
            };
            let payload = if legacy_replay {
                serde_json::json!({"mode":"sample","count":count,"seed":seed})
            } else {
                serde_json::json!({"mode":"sample","count":count,"seed":seed,"profile":"si-rectangle-v2"})
            };
            let request_bytes = serde_json::to_vec(&payload)
                .map_err(|e| ArchyError::new(ErrorCode::InternalError, "request", e.to_string()))?;
            let request = RunRequest {
                schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
                context,
                operation: "generate".into(),
                request_bytes,
                input_sha256: sha256_hex(&raw),
                source_run: None,
            };
            let pending = match begin_with_binding(locked, request, check)? {
                archy_core::BeginResult::Replay(receipt) => {
                    let verified = verify_locked(locked, &receipt.run_id)?;
                    return Ok(workflow_receipt(receipt, &verified));
                }
                archy_core::BeginResult::Pending(pending) => pending,
            };
            let source = std::str::from_utf8(&raw)
                .map_err(|_| ArchyError::schema("project", "Project is not UTF-8"))?;
            let current = parse_project_current(source, format)?;
            let spec = &current.numeric;
            let generation = generate_variants(spec, count, seed)?;
            let evaluations = generation
                .variants
                .iter()
                .map(|variant| evaluate_variant_v2(spec, variant))
                .collect::<Result<Vec<_>, _>>()?;
            let normalized = serde_json::to_vec(&current.canonical).map_err(|e| {
                ArchyError::new(ErrorCode::InternalError, "normalized", e.to_string())
            })?;
            let metadata = RunMetadata {
                project_id: spec.project_id.clone(),
                normalized_sha256: Some(sha256_hex(&normalized)),
                source_format: Some(source_format.into()),
                seed: Some(seed),
                count: Some(count),
                parent_variant_id: None,
                algorithm_version: generation.algorithm_version.clone(),
            };
            let receipt = publish(
                pending,
                metadata,
                RunPayload {
                    operation: "generate",
                    parent_run_id: None,
                    format: source_format,
                    input: &raw,
                    spec,
                    canonical: Some(&current.canonical),
                    generation: Some(&generation),
                    params: None,
                    variants: &generation.variants,
                    evaluations: &evaluations,
                },
                check,
            )?;
            let verified = verify_locked(locked, &receipt.run_id)?;
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
            let parent = verify_locked(locked, &from_run)?;
            if !matches!(parent.manifest().operation.as_str(), "generate" | "edit") {
                return Err(artifact_invalid(
                    "edit source must be a generation or edit run",
                ));
            }
            let migration = if recorded.is_none() {
                migrate_claimed_v1_source(&parent)?
            } else {
                None
            };
            let canonical = migration
                .as_ref()
                .map(|current| &current.canonical)
                .or(parent.canonical());
            let spec = migration
                .as_ref()
                .map_or(parent.spec(), |current| &current.numeric);
            let mut request_payload = serde_json::json!({"mode":"edit","from_run":from_run,"variant":variant,"params_sha256":sha256_hex(&params_bytes)});
            let replay_current = match &recorded {
                Some(run) => verify_locked(locked, &run.run_id)?.canonical().is_some(),
                None => false,
            };
            if canonical.is_some() || replay_current {
                request_payload["profile"] = "si-rectangle-v2".into();
            }
            let request_bytes = serde_json::to_vec(&request_payload)
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
            let pending = match begin_with_binding(locked, request, check)? {
                archy_core::BeginResult::Replay(receipt) => {
                    let verified = verify_locked(locked, &receipt.run_id)?;
                    return Ok(workflow_receipt(receipt, &verified));
                }
                archy_core::BeginResult::Pending(pending) => pending,
            };
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
                spec.project_id.clone(),
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
            let evaluations = vec![if canonical.is_some() {
                evaluate_variant_v2(spec, &derived)?
            } else {
                evaluate_variant_v1(spec, &derived)?
            }];
            let variants = vec![derived];
            let normalized = match canonical {
                Some(canonical) => serde_json::to_vec(canonical),
                None => serde_json::to_vec(spec),
            }
            .map_err(|e| ArchyError::new(ErrorCode::InternalError, "normalized", e.to_string()))?;
            let source_format = parent
                .manifest()
                .source_format
                .as_deref()
                .ok_or_else(|| artifact_invalid("missing source format"))?;
            let metadata = RunMetadata {
                project_id: spec.project_id.clone(),
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
                    spec,
                    canonical,
                    generation: None,
                    params: Some(&params_bytes),
                    variants: &variants,
                    evaluations: &evaluations,
                },
                check,
            )?;
            let verified = verify_locked(locked, &receipt.run_id)?;
            Ok(workflow_receipt(receipt, &verified))
        }
    }
}
