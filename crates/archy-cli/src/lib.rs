//! Reusable local workflow service. The binary and later typed tools call the same API.
#[cfg(target_os = "linux")]
mod accepted_worker;
mod ascii_json;
mod compare;
mod desktop_capabilities;
mod desktop_status;
mod evaluate;
mod generate;
mod project_read;
mod run_integrity;

#[cfg(target_os = "linux")]
pub use accepted_worker::{
    AcceptedWorkerBackend, accepted_service_request, run_accepted_service, start_accepted_service,
};
pub use ascii_json::{ascii_json_compact, ascii_json_pretty};
pub use compare::{compare, render_verified_comparison};
pub use desktop_capabilities::{
    DesktopCapabilities, desktop_capabilities, desktop_capabilities_schema,
};
pub use desktop_status::{
    DesktopStatus, DesktopStatusInput, desktop_status, desktop_status_schema,
};
pub use evaluate::evaluate;
pub use generate::generate;
pub use project_read::{BoundProjectRead, BoundReadSnapshot};
pub use run_integrity::{TrustedRun, verify_run};

use archy_core::{
    ArchyError, ErrorCode, EvaluationReport, InputFormat, LockedProject, MutationContext,
    NormalizedSpec, PendingRun, RunManifest, RunMetadata, RunReceipt, Variant, VariantOutcome,
    sha256_hex,
};
use archy_generate::GenerationResult;
use archy_report::{REPORT_CONTEXT_VERSION, ReportContext, render_report};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationOptions {
    pub idempotency_key: Option<String>,
    pub expected_project_revision: Option<u64>,
    pub destination: String,
}
impl Default for MutationOptions {
    fn default() -> Self {
        Self {
            idempotency_key: None,
            expected_project_revision: None,
            destination: "runs".into(),
        }
    }
}
impl MutationOptions {
    pub(crate) fn resolve(
        &self,
        locked: &LockedProject<'_>,
    ) -> Result<MutationContext, ArchyError> {
        if self.idempotency_key.is_some() && self.expected_project_revision.is_none() {
            return Err(ArchyError::schema(
                "expected_project_revision",
                "An explicit idempotency key requires an explicit expected revision",
            ));
        }
        Ok(MutationContext {
            idempotency_key: self
                .idempotency_key
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            expected_project_revision: match self.expected_project_revision {
                Some(value) => value,
                None => locked.revision()?,
            },
            destination: self.destination.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationMode {
    Sample {
        count: u32,
        seed: u64,
    },
    Edit {
        from_run: String,
        variant: String,
        params: String,
    },
}
#[derive(Debug, Clone)]
pub struct GenerateInput {
    pub project: PathBuf,
    pub mode: GenerationMode,
    pub mutation: MutationOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantSelection {
    One(String),
    All,
}
#[derive(Debug, Clone)]
pub struct EvaluateInput {
    pub project: PathBuf,
    pub source_run: Option<String>,
    pub selection: VariantSelection,
    pub mutation: MutationOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReceipt {
    pub run_id: String,
    pub revision: u64,
    pub path: String,
    pub variant_ids: Vec<String>,
    pub all_required_checks_passed: bool,
    pub exhaustion_explanations: Vec<String>,
}

pub(crate) fn artifact_invalid(message: impl Into<String>) -> ArchyError {
    ArchyError::new(ErrorCode::ArtifactInvalid, "run", message)
}

pub(crate) fn request_conflict() -> ArchyError {
    ArchyError::new(
        ErrorCode::RevisionConflict,
        "storage",
        "idempotency key reused with different request",
    )
}

pub(crate) fn recorded_manifest_for_key(
    locked: &LockedProject<'_>,
    explicit_key: Option<&str>,
) -> Result<Option<RunManifest>, ArchyError> {
    let Some(key) = explicit_key else {
        return Ok(None);
    };
    for receipt in locked.receipts()?.iter().rev() {
        let stored = locked.read_run(&receipt.run_id)?;
        if stored.manifest().idempotency_key == key {
            return Ok(Some(stored.manifest().clone()));
        }
    }
    Ok(None)
}

pub(crate) fn input_format(
    project: &Path,
) -> Result<(InputFormat, &'static str, &'static str), ArchyError> {
    match project.extension().and_then(|s| s.to_str()) {
        Some("json") => Ok((InputFormat::Json, "json", "input.json")),
        Some("yaml" | "yml") => Ok((InputFormat::Yaml, "yaml", "input.yaml")),
        _ => Err(ArchyError::schema(
            "project",
            "Expected .json, .yaml or .yml project file",
        )),
    }
}

pub(crate) fn workflow_receipt(receipt: RunReceipt, run: &TrustedRun) -> WorkflowReceipt {
    WorkflowReceipt {
        run_id: receipt.run_id,
        revision: receipt.revision,
        path: receipt.path,
        variant_ids: run.variants().iter().map(|v| v.id.clone()).collect(),
        all_required_checks_passed: !run.variants().is_empty()
            && run.evaluations().iter().all(|e| e.required_checks_passed),
        exhaustion_explanations: run.exhaustion().to_vec(),
    }
}

pub(crate) struct RunPayload<'a> {
    pub operation: &'a str,
    pub parent_run_id: Option<&'a str>,
    pub format: &'a str,
    pub input: &'a [u8],
    pub spec: &'a NormalizedSpec,
    pub generation: Option<&'a GenerationResult>,
    pub params: Option<&'a [u8]>,
    pub variants: &'a [Variant],
    pub evaluations: &'a [EvaluationReport],
}

pub(crate) fn publish(
    mut pending: Box<PendingRun<'_>>,
    metadata: RunMetadata,
    payload: RunPayload<'_>,
    check: Option<&dyn Fn() -> Result<(), ArchyError>>,
) -> Result<RunReceipt, ArchyError> {
    let normalized = serde_json::to_vec(payload.spec)
        .map_err(|e| ArchyError::new(ErrorCode::InternalError, "normalized", e.to_string()))?;
    let input_sha256 = sha256_hex(payload.input);
    let normalized_sha256 = sha256_hex(&normalized);
    let context = ReportContext {
        schema_version: REPORT_CONTEXT_VERSION.into(),
        run_id: pending.run_id().into(),
        parent_run_id: payload.parent_run_id.map(str::to_owned),
        parent_variant_id: metadata.parent_variant_id.clone(),
        operation: payload.operation.into(),
        input_sha256,
        normalized_sha256,
        algorithm_version: metadata.algorithm_version.clone(),
        build_version: env!("CARGO_PKG_VERSION").into(),
        requested_count: metadata.count,
        attempts: payload.generation.map(|g| g.attempts),
        exhaustion_explanations: payload
            .generation
            .map_or_else(Vec::new, |g| g.exhaustion_explanations.clone()),
        attempt_diagnostics: payload
            .generation
            .and_then(|g| g.attempt_diagnostics.clone()),
    };
    let report = render_report(
        payload.spec,
        payload.variants,
        payload.evaluations,
        &context,
    )?;
    let input_path = match payload.format {
        "json" => "input.json",
        "yaml" => "input.yaml",
        _ => {
            return Err(ArchyError::schema(
                "source_format",
                "Unsupported source format",
            ));
        }
    };
    pending.write(
        input_path,
        payload.input,
        "application/octet-stream",
        "archy-cli",
    )?;
    pending.write(
        "normalized.json",
        &normalized,
        "application/json",
        "archy-core",
    )?;
    if let Some(generation) = payload.generation {
        let bytes = serde_json::to_vec(generation)
            .map_err(|e| ArchyError::new(ErrorCode::InternalError, "generation", e.to_string()))?;
        pending.write(
            "generation.json",
            &bytes,
            "application/json",
            "archy-generate",
        )?;
    }
    if let Some(params) = payload.params {
        pending.write("params.json", params, "application/json", "archy-cli")?;
    }
    for (variant, evaluation) in payload.variants.iter().zip(payload.evaluations) {
        let variant_path = format!("variants/{}.json", variant.id);
        let evaluation_path = format!("evaluations/{}.json", variant.id);
        let variant_bytes = serde_json::to_vec(variant)
            .map_err(|e| ArchyError::new(ErrorCode::InternalError, "variant", e.to_string()))?;
        let evaluation_bytes = serde_json::to_vec(evaluation)
            .map_err(|e| ArchyError::new(ErrorCode::InternalError, "evaluation", e.to_string()))?;
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
        pending.record_outcome(VariantOutcome {
            variant_id: variant.id.clone(),
            variant_artifact: Some(variant_path),
            evaluation_artifact: Some(evaluation_path),
            error_artifact: None,
        })?;
    }
    pending.write(
        "report.json",
        &report.json,
        "application/json",
        "archy-report",
    )?;
    pending.write(
        "report.md",
        &report.markdown,
        "text/markdown",
        "archy-report",
    )?;
    pending.write(
        "events.jsonl",
        &run_integrity::event_bytes(payload.operation, payload.variants, payload.evaluations)?,
        "application/x-ndjson",
        "archy-cli",
    )?;
    match check {
        Some(check) => pending.commit_checked(metadata, check),
        None => pending.commit(metadata),
    }
}
