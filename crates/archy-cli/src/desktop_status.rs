use crate::{artifact_invalid, input_format, run_integrity::verify_history};
use archy_core::{ArchyError, ErrorCode, ProjectStore, RunManifest, parse_project, sha256_hex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DESKTOP_STATUS_VERSION: &str = "archy-desktop-status-v1";
const MAX_RESPONSE_BYTES: usize = 1_048_576;

#[derive(Debug, Clone)]
pub struct DesktopStatusInput {
    pub project: PathBuf,
    pub run: Option<String>,
    pub limit: u32,
    pub cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopProject {
    pub id: String,
    pub revision: u64,
    pub source_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopRunSummary {
    pub run_id: String,
    pub revision: u64,
    pub operation: DesktopRunOperation,
    pub variant_count: usize,
    pub all_required_checks_passed: bool,
    pub artifact_count: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopRunOperation {
    Generate,
    Edit,
    Evaluate,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopArtifactSummary {
    pub id: String,
    pub kind: DesktopArtifactKind,
    pub byte_len: u64,
    pub sha256: String,
    pub integrity: DesktopArtifactIntegrity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopArtifactKind {
    Report,
    Variant,
    Evaluation,
    Source,
    RunData,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopArtifactIntegrity {
    Verified,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopSelectedRun {
    pub summary: DesktopRunSummary,
    pub artifacts: Vec<DesktopArtifactSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopJobObservation {
    pub status: DesktopJobStatus,
    pub reason: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopJobStatus {
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopStatus {
    pub schema_version: String,
    pub project: DesktopProject,
    pub runs: Vec<DesktopRunSummary>,
    pub selected_run: Option<DesktopSelectedRun>,
    pub next_cursor: Option<String>,
    pub backend_jobs: DesktopJobObservation,
}
fn conflict(message: &str) -> ArchyError {
    ArchyError::new(ErrorCode::RevisionConflict, "cursor", message)
}
fn invalid_cursor() -> ArchyError {
    ArchyError::schema("cursor", "Invalid desktop status cursor")
}
fn cursor_token(binding: &str, revision: u64, head: Option<&str>, offset: usize) -> String {
    format!(
        "v1:{binding}:{revision}:{}:{offset}",
        head.unwrap_or("genesis")
    )
}
fn parse_cursor(
    value: &str,
    binding: &str,
    revision: u64,
    head: Option<&str>,
    len: usize,
) -> Result<usize, ArchyError> {
    if value.len() > 200 {
        return Err(invalid_cursor());
    }
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 5 || parts[0] != "v1" || parts[1].len() != 64 || parts[3].len() > 64 {
        return Err(invalid_cursor());
    }
    let recorded_revision = parts[2].parse::<u64>().map_err(|_| invalid_cursor())?;
    let offset = parts[4].parse::<usize>().map_err(|_| invalid_cursor())?;
    if parts[1] != binding || recorded_revision != revision || parts[3] != head.unwrap_or("genesis")
    {
        return Err(conflict(
            "Desktop status page is stale or belongs to another project; restart at first page",
        ));
    }
    if offset == 0 || offset >= len {
        return Err(invalid_cursor());
    }
    Ok(offset)
}
fn summary(
    manifest: &RunManifest,
    revision: u64,
    checks: &[archy_core::EvaluationReport],
) -> Result<DesktopRunSummary, ArchyError> {
    let operation = match manifest.operation.as_str() {
        "generate" => DesktopRunOperation::Generate,
        "edit" => DesktopRunOperation::Edit,
        "evaluate" => DesktopRunOperation::Evaluate,
        _ => return Err(artifact_invalid("unsupported run operation")),
    };
    Ok(DesktopRunSummary {
        run_id: manifest.run_id.clone(),
        revision,
        operation,
        variant_count: manifest.outcomes.len(),
        all_required_checks_passed: !checks.is_empty()
            && checks.iter().all(|e| e.required_checks_passed),
        artifact_count: manifest.artifacts.len(),
    })
}
fn artifact_kind(path: &str) -> DesktopArtifactKind {
    if path == "report.json" || path == "report.md" {
        DesktopArtifactKind::Report
    } else if path.starts_with("variants/") {
        DesktopArtifactKind::Variant
    } else if path.starts_with("evaluations/") {
        DesktopArtifactKind::Evaluation
    } else if path == "input.json" || path == "input.yaml" || path == "normalized.json" {
        DesktopArtifactKind::Source
    } else {
        DesktopArtifactKind::RunData
    }
}
pub fn desktop_status(input: DesktopStatusInput) -> Result<DesktopStatus, ArchyError> {
    if !(1..=100).contains(&input.limit) {
        return Err(ArchyError::schema("limit", "Limit must be 1..=100"));
    }
    if input.run.as_ref().is_some_and(|id| id.len() > 128) {
        return Err(ArchyError::schema("run", "Run ID too long"));
    }
    let (format, _, _) = input_format(&input.project)?;
    let store = ProjectStore::open(&input.project)?;
    let source = store.read_source()?;
    let source_text = std::str::from_utf8(&source)
        .map_err(|_| ArchyError::schema("project", "Project is not UTF-8"))?;
    let spec = parse_project(source_text, format)?;
    let source_sha256 = sha256_hex(&source);
    let history = store.read_history()?;
    let revision = history.revision();
    let binding = sha256_hex(
        format!(
            "archy-desktop-page-v1\0{}:{}:{}",
            store.identity_digest()?,
            spec.project_id,
            source_sha256
        )
        .as_bytes(),
    );
    let receipts: Vec<_> = history.receipts().rev().collect();
    let start = match input.cursor.as_deref() {
        Some(value) => parse_cursor(
            value,
            &binding,
            revision,
            history.head_sha256(),
            receipts.len(),
        )?,
        None => 0,
    };
    let end = start
        .saturating_add(input.limit as usize)
        .min(receipts.len());
    let mut runs = Vec::with_capacity(end - start);
    for receipt in &receipts[start..end] {
        let trusted = verify_history(&history, &receipt.run_id)?;
        if trusted.manifest().project_id != spec.project_id {
            return Err(artifact_invalid(
                "accepted run belongs to another project ID",
            ));
        }
        runs.push(summary(
            trusted.manifest(),
            receipt.revision,
            trusted.evaluations(),
        )?);
    }
    let selected_receipt = match input.run.as_deref() {
        Some(id) => Some(
            *receipts
                .iter()
                .find(|r| r.run_id == id)
                .ok_or_else(|| artifact_invalid("run is not accepted"))?,
        ),
        None => receipts.get(start).copied(),
    };
    let selected_run = if let Some(receipt) = selected_receipt {
        let trusted = verify_history(&history, &receipt.run_id)?;
        if trusted.manifest().project_id != spec.project_id {
            return Err(artifact_invalid(
                "accepted run belongs to another project ID",
            ));
        }
        let artifacts = trusted
            .manifest()
            .artifacts
            .iter()
            .map(|a| DesktopArtifactSummary {
                id: a.path.clone(),
                kind: artifact_kind(&a.path),
                byte_len: a.byte_len,
                sha256: a.sha256.clone(),
                integrity: DesktopArtifactIntegrity::Verified,
            })
            .collect();
        Some(DesktopSelectedRun {
            summary: summary(trusted.manifest(), receipt.revision, trusted.evaluations())?,
            artifacts,
        })
    } else {
        None
    };
    let next_cursor = (end < receipts.len())
        .then(|| cursor_token(&binding, revision, history.head_sha256(), end));
    let response = DesktopStatus {
        schema_version: DESKTOP_STATUS_VERSION.into(),
        project: DesktopProject {
            id: spec.project_id,
            revision,
            source_sha256,
        },
        runs,
        selected_run,
        next_cursor,
        backend_jobs: DesktopJobObservation {
            status: DesktopJobStatus::Unknown,
            reason: "No supervisor job observations are available through this reader".into(),
        },
    };
    let bytes = serde_json::to_vec_pretty(&response)
        .map_err(|e| ArchyError::new(ErrorCode::InternalError, "$", e.to_string()))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(artifact_invalid(
            "Desktop status response exceeds 1 MiB; select a smaller page",
        ));
    }
    Ok(response)
}
pub fn desktop_status_schema() -> schemars::Schema {
    schemars::schema_for!(DesktopStatus)
}
