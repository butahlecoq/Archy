use crate::{BoundProjectRead, artifact_invalid, desktop_open::DesktopOpenKind};
use archy_core::{ArchyError, ErrorCode, sha256_hex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DESKTOP_STATUS_VERSION: &str = "archy-desktop-status-v1";
// A run may contain 4,096 artifacts with 512-byte paths. JSON escaping can
// expand path bytes sixfold, and a desktop page includes 20 render disclosures.
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_outcome: Option<DesktopRenderOutcome>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preview_symbols: Vec<DesktopPreviewSymbol>,
    pub artifact_count: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopPreviewSymbol {
    pub variant_id: String,
    pub views: Vec<String>,
    pub notice: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopRenderOutcome {
    Verified,
    Partial,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopRunOperation {
    Generate,
    Edit,
    Evaluate,
    Render,
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
    Preview,
    Blender,
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
    trusted: &crate::run_integrity::TrustedRun,
    revision: u64,
) -> Result<DesktopRunSummary, ArchyError> {
    let manifest = trusted.manifest();
    let checks = trusted.evaluations();
    let operation = match manifest.operation.as_str() {
        "generate" => DesktopRunOperation::Generate,
        "edit" => DesktopRunOperation::Edit,
        "evaluate" => DesktopRunOperation::Evaluate,
        "render" => DesktopRunOperation::Render,
        _ => return Err(artifact_invalid("unsupported run operation")),
    };
    Ok(DesktopRunSummary {
        run_id: manifest.run_id.clone(),
        revision,
        operation,
        variant_count: manifest.outcomes.len(),
        all_required_checks_passed: !checks.is_empty()
            && checks.iter().all(|e| e.required_checks_passed),
        render_outcome: (operation == DesktopRunOperation::Render).then(|| {
            let verified = manifest
                .outcomes
                .iter()
                .filter(|o| o.evaluation_artifact.is_some())
                .count();
            if verified == manifest.outcomes.len() {
                DesktopRenderOutcome::Verified
            } else if verified == 0 {
                DesktopRenderOutcome::Failed
            } else {
                DesktopRenderOutcome::Partial
            }
        }),
        preview_symbols: trusted
            .render_record()
            .into_iter()
            .flat_map(|record| &record.variants)
            .filter_map(|variant| match &variant.result {
                crate::render::RenderResult::Verified {
                    preview_symbol: Some(symbol),
                    ..
                } => Some(DesktopPreviewSymbol {
                    variant_id: variant.variant_id.clone(),
                    views: symbol.views.clone(),
                    notice: symbol.notice.clone(),
                }),
                _ => None,
            })
            .collect(),
        artifact_count: manifest.artifacts.len(),
    })
}
fn artifact_kind(path: &str) -> DesktopArtifactKind {
    if path == "report.json" || path == "report.md" {
        DesktopArtifactKind::Report
    } else if crate::desktop_open::is_openable_path(DesktopOpenKind::Preview, path) {
        DesktopArtifactKind::Preview
    } else if crate::desktop_open::is_openable_path(DesktopOpenKind::Blender, path) {
        DesktopArtifactKind::Blender
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
    let bound = BoundProjectRead::open(&input.project)?;
    let snapshot = bound.snapshot()?;
    let spec = snapshot.spec();
    let source_sha256 = snapshot.source_sha256();
    let revision = snapshot.revision();
    let binding = sha256_hex(
        format!(
            "archy-desktop-page-v1\0{}:{}:{}",
            snapshot.identity(),
            spec.project_id,
            source_sha256
        )
        .as_bytes(),
    );
    let receipts: Vec<_> = snapshot.receipts().rev().collect();
    let start = match input.cursor.as_deref() {
        Some(value) => parse_cursor(
            value,
            &binding,
            revision,
            snapshot.head_sha256(),
            receipts.len(),
        )?,
        None => 0,
    };
    let end = start
        .saturating_add(input.limit as usize)
        .min(receipts.len());
    let mut runs = Vec::with_capacity(end - start);
    for receipt in &receipts[start..end] {
        let trusted = snapshot.verify_run(&receipt.run_id)?;
        if trusted.manifest().project_id != spec.project_id {
            return Err(artifact_invalid(
                "accepted run belongs to another project ID",
            ));
        }
        runs.push(summary(&trusted, receipt.revision)?);
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
        let trusted = snapshot.verify_run(&receipt.run_id)?;
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
            summary: summary(&trusted, receipt.revision)?,
            artifacts,
        })
    } else {
        None
    };
    let next_cursor = (end < receipts.len())
        .then(|| cursor_token(&binding, revision, snapshot.head_sha256(), end));
    let response = DesktopStatus {
        schema_version: DESKTOP_STATUS_VERSION.into(),
        project: DesktopProject {
            id: spec.project_id.clone(),
            revision,
            source_sha256: source_sha256.to_owned(),
        },
        runs,
        selected_run,
        next_cursor,
        backend_jobs: DesktopJobObservation {
            status: DesktopJobStatus::Unknown,
            reason: "No supervisor job observations are available through this reader".into(),
        },
    };
    let bytes = crate::ascii_json_pretty(&response)
        .map_err(|e| ArchyError::new(ErrorCode::InternalError, "$", e.to_string()))?;
    // The CLI adds one newline after serializing this response.
    if bytes.len() + 1 > MAX_RESPONSE_BYTES {
        return Err(artifact_invalid(
            "Desktop status response exceeds 32 MiB; select a smaller page",
        ));
    }
    Ok(response)
}
pub fn desktop_status_schema() -> schemars::Schema {
    let mut schema = serde_json::to_value(schemars::schema_for!(DesktopStatus)).unwrap();
    schema["$comment"] = serde_json::json!(format!(
        "Desktop CLI ASCII JSON output, including its trailing newline, is limited to {MAX_RESPONSE_BYTES} bytes."
    ));
    schema["properties"]["runs"]["maxItems"] = serde_json::json!(100);
    schema["$defs"]["DesktopRunSummary"]["properties"]["variant_count"]["maximum"] =
        serde_json::json!(1000);
    schema["$defs"]["DesktopRunSummary"]["properties"]["artifact_count"]["maximum"] =
        serde_json::json!(archy_core::MAX_ARTIFACTS);
    schema["$defs"]["DesktopRunSummary"]["properties"]["preview_symbols"]["maxItems"] =
        serde_json::json!(1000);
    schema["$defs"]["DesktopRunSummary"]["$comment"] = serde_json::json!(
        "For render summaries, preview_symbols length must not exceed variant_count. This cross-field cardinality rule is enforced by DesktopModel because standard JSON Schema cannot compare an array length with a sibling value."
    );
    schema["$defs"]["DesktopRunSummary"]["properties"]["render_outcome"] =
        serde_json::json!({"$ref": "#/$defs/DesktopRenderOutcome"});
    schema["$defs"]["DesktopRunSummary"]["allOf"] = serde_json::json!([{
        "if": {"properties": {"operation": {"const": "render"}}, "required": ["operation"]},
        "then": {"required": ["render_outcome"]},
        "else": {"not": {"anyOf": [
            {"required": ["render_outcome"]},
            {"required": ["preview_symbols"]}
        ]}}
    }]);
    schema["$defs"]["DesktopPreviewSymbol"]["properties"]["variant_id"]["pattern"] =
        serde_json::json!("^variant-[0-9a-f]{64}$");
    schema["$defs"]["DesktopPreviewSymbol"]["properties"]["notice"]["const"] =
        serde_json::json!(crate::render::SCREEN_WIDTH_EDGE_NOTICE);
    schema["$defs"]["DesktopPreviewSymbol"]["properties"]["views"]["minItems"] =
        serde_json::json!(1);
    schema["$defs"]["DesktopPreviewSymbol"]["properties"]["views"]["items"]["enum"] =
        serde_json::json!(["top", "front", "iso"]);
    schema["$defs"]["DesktopPreviewSymbol"]["properties"]["views"]["maxItems"] =
        serde_json::json!(3);
    schema["$defs"]["DesktopPreviewSymbol"]["properties"]["views"]["uniqueItems"] =
        serde_json::json!(true);
    schema["$defs"]["DesktopSelectedRun"]["properties"]["artifacts"]["maxItems"] =
        serde_json::json!(archy_core::MAX_ARTIFACTS);
    serde_json::from_value(schema).unwrap()
}
