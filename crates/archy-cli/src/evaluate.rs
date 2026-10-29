use crate::{
    EvaluateInput, RunPayload, VariantSelection, WorkflowReceipt, artifact_invalid, publish,
    recorded_manifest_for_key, request_conflict, run_integrity::verify_locked, workflow_receipt,
};
use archy_core::{
    ArchyError, ErrorCode, RUN_REQUEST_SCHEMA_VERSION, RunMetadata, RunRequest, sha256_hex,
};
use archy_eval::{EVALUATION_VERSION, evaluate_variant};

pub fn evaluate(input: EvaluateInput) -> Result<WorkflowReceipt, ArchyError> {
    let store = archy_core::ProjectStore::open(&input.project)?;
    let locked = store.lock()?;
    let context = input.mutation.resolve(&locked)?;
    let recorded = recorded_manifest_for_key(&locked, input.mutation.idempotency_key.as_deref())?;
    if recorded
        .as_ref()
        .is_some_and(|run| run.operation != "evaluate")
    {
        return Err(request_conflict());
    }
    let receipts = locked.receipts()?;
    let replay_source = if input.source_run.is_none() {
        recorded.as_ref().and_then(|run| run.parent_run_id.clone())
    } else {
        None
    };
    let source_id = match input.source_run.or(replay_source) {
        Some(id) => id,
        None => {
            let mut found = None;
            for receipt in receipts.iter().rev() {
                let stored = locked.read_run(&receipt.run_id)?;
                if matches!(stored.manifest().operation.as_str(), "generate" | "edit") {
                    found = Some(receipt.run_id.clone());
                    break;
                }
            }
            found.ok_or_else(|| ArchyError::schema("run", "No accepted generation or edit run"))?
        }
    };
    if let Some(run) = &recorded
        && run.parent_run_id.as_deref() != Some(&source_id)
    {
        return Err(request_conflict());
    }
    if let VariantSelection::One(id) = &input.selection
        && id.len() > archy_core::validation::MAX_INPUT_BYTES.saturating_sub(64)
    {
        return Err(ArchyError::schema(
            "variant",
            "Selector exceeds request byte limit",
        ));
    }
    let selection_bytes = match &input.selection {
        VariantSelection::All => serde_json::json!({"selector":"all"}),
        VariantSelection::One(id) => serde_json::json!({"selector":"one","variant":id}),
    };
    let request_bytes = serde_json::to_vec(&selection_bytes)
        .map_err(|e| ArchyError::new(ErrorCode::InternalError, "request", e.to_string()))?;
    let request = RunRequest {
        schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
        context,
        operation: "evaluate".into(),
        request_bytes,
        input_sha256: match &recorded {
            Some(run) => run.input_sha256.clone(),
            None => locked.read_run(&source_id)?.manifest().input_sha256.clone(),
        },
        source_run: Some(source_id.clone()),
    };
    let pending = match locked.begin(request)? {
        archy_core::BeginResult::Replay(receipt) => {
            let verified = verify_locked(&locked, &receipt.run_id)?;
            return Ok(workflow_receipt(receipt, &verified));
        }
        archy_core::BeginResult::Pending(pending) => pending,
    };
    let source = verify_locked(&locked, &source_id)?;
    if !matches!(source.manifest().operation.as_str(), "generate" | "edit") {
        return Err(artifact_invalid(
            "evaluation source must be a generation or edit run",
        ));
    }
    let variants = match &input.selection {
        VariantSelection::All => source.variants().to_vec(),
        VariantSelection::One(id) => vec![
            source
                .variants()
                .iter()
                .find(|v| v.id == *id)
                .cloned()
                .ok_or_else(|| {
                    ArchyError::schema("variant", "Variant is absent from source run")
                })?,
        ],
    };
    if variants.is_empty() {
        return Err(ArchyError::schema(
            "run",
            "Source run has no variants to evaluate",
        ));
    }
    let evaluations = variants
        .iter()
        .map(|variant| evaluate_variant(source.spec(), variant))
        .collect::<Result<Vec<_>, _>>()?;
    let normalized = serde_json::to_vec(source.spec())
        .map_err(|e| ArchyError::new(ErrorCode::InternalError, "normalized", e.to_string()))?;
    let source_format = source
        .manifest()
        .source_format
        .as_deref()
        .ok_or_else(|| artifact_invalid("missing source format"))?;
    let metadata = RunMetadata {
        project_id: source.spec().project_id.clone(),
        normalized_sha256: Some(sha256_hex(&normalized)),
        source_format: Some(source_format.into()),
        seed: None,
        count: None,
        parent_variant_id: None,
        algorithm_version: EVALUATION_VERSION.into(),
    };
    let receipt = publish(
        pending,
        metadata,
        RunPayload {
            operation: "evaluate",
            parent_run_id: Some(&source_id),
            format: source_format,
            input: source.input(),
            spec: source.spec(),
            generation: None,
            params: None,
            variants: &variants,
            evaluations: &evaluations,
        },
    )?;
    let verified = verify_locked(&locked, &receipt.run_id)?;
    Ok(workflow_receipt(receipt, &verified))
}
