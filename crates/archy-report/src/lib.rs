mod templates;

use archy_core::{ArchyError, ErrorCode, EvaluationReport, NormalizedSpec, Variant, site_bounds};
use archy_eval::evaluate_variant;
use archy_generate::ALGORITHM_VERSION;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const REPORT_SCHEMA_VERSION: &str = "archy-report-v1";
pub const REPORT_CONTEXT_VERSION: &str = "archy-report-context-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportContext {
    pub schema_version: String,
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub parent_variant_id: Option<String>,
    pub operation: String,
    pub input_sha256: String,
    pub normalized_sha256: String,
    pub algorithm_version: String,
    pub build_version: String,
    pub requested_count: Option<u32>,
    pub attempts: Option<u32>,
    pub exhaustion_explanations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportArtifacts {
    pub json: Vec<u8>,
    pub markdown: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportDocument {
    pub schema_version: String,
    pub context: ReportContext,
    pub spec: NormalizedSpec,
    pub variants: Vec<ReportVariant>,
    pub uncomputed: Vec<String>,
    pub professional_limits: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportVariant {
    pub variant: Variant,
    pub evaluation: EvaluationReport,
}

pub fn report_schema() -> schemars::Schema {
    schemars::schema_for!(ReportDocument)
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_context(context: &ReportContext, variants: &[Variant]) -> Result<(), ArchyError> {
    if context.schema_version != REPORT_CONTEXT_VERSION {
        return Err(ArchyError::schema(
            "context.schema_version",
            "Unsupported report context version",
        ));
    }
    for (field, value) in [
        ("run_id", Some(context.run_id.as_str())),
        ("parent_run_id", context.parent_run_id.as_deref()),
    ] {
        if let Some(value) = value
            && !valid_id(value)
        {
            return Err(ArchyError::schema(
                format!("context.{field}"),
                "Invalid report identifier",
            ));
        }
    }
    if let Some(parent_variant_id) = &context.parent_variant_id
        && !(parent_variant_id.starts_with("variant-") && valid_digest(&parent_variant_id[8..]))
    {
        return Err(ArchyError::schema(
            "context.parent_variant_id",
            "Expected canonical variant identifier syntax",
        ));
    }
    if context.parent_variant_id.is_some() && context.parent_run_id.is_none() {
        return Err(ArchyError::schema(
            "context.parent_variant_id",
            "Parent variant needs parent run",
        ));
    }
    for (field, value) in [
        ("input_sha256", &context.input_sha256),
        ("normalized_sha256", &context.normalized_sha256),
    ] {
        if !valid_digest(value) {
            return Err(ArchyError::schema(
                format!("context.{field}"),
                "Expected lowercase SHA-256",
            ));
        }
    }
    if context.build_version.trim().is_empty() || context.build_version.len() > 128 {
        return Err(ArchyError::schema(
            "context",
            "Build provenance version is required",
        ));
    }
    let invalid =
        |field: &str, message: &str| ArchyError::schema(format!("context.{field}"), message);
    match context.operation.as_str() {
        "generate" => {
            if context.parent_run_id.is_some() || context.parent_variant_id.is_some() {
                return Err(invalid(
                    "parent_run_id",
                    "Generation must not claim a parent",
                ));
            }
            let requested = context
                .requested_count
                .ok_or_else(|| invalid("requested_count", "Generation needs requested count"))?;
            let attempts = context
                .attempts
                .ok_or_else(|| invalid("attempts", "Generation needs attempt count"))?;
            if !(1..=1000).contains(&requested)
                || attempts > requested.saturating_mul(100)
                || variants.len() > requested as usize
                || attempts < variants.len() as u32
            {
                return Err(invalid(
                    "requested_count",
                    "Generation count or attempts contradict results",
                ));
            }
            let incomplete = variants.len() < requested as usize;
            let has_reason = context
                .exhaustion_explanations
                .iter()
                .any(|explanation| !explanation.trim().is_empty());
            if incomplete != has_reason
                || context
                    .exhaustion_explanations
                    .iter()
                    .any(|explanation| explanation.trim().is_empty())
            {
                return Err(invalid(
                    "exhaustion_explanations",
                    "Partial or empty generation needs a reason; complete generation has none",
                ));
            }
            if context.algorithm_version != ALGORITHM_VERSION {
                return Err(invalid(
                    "algorithm_version",
                    "Unsupported generation algorithm",
                ));
            }
        }
        "edit" => {
            if context.parent_run_id.is_none()
                || context.parent_variant_id.is_none()
                || context.requested_count != Some(1)
                || context.attempts.is_some()
                || !context.exhaustion_explanations.is_empty()
                || variants.len() != 1
            {
                return Err(invalid(
                    "operation",
                    "Edit needs one result, parent run and variant, and no generation attempts",
                ));
            }
            if context.algorithm_version != ALGORITHM_VERSION {
                return Err(invalid(
                    "algorithm_version",
                    "Unsupported edit generation algorithm",
                ));
            }
        }
        "evaluate" => {
            if context.parent_run_id.is_none()
                || context.parent_variant_id.is_some()
                || context.requested_count.is_some()
                || context.attempts.is_some()
                || !context.exhaustion_explanations.is_empty()
                || variants.is_empty()
            {
                return Err(invalid(
                    "operation",
                    "Evaluation needs a parent run and selected variants, without generation counts",
                ));
            }
            if context.algorithm_version != archy_eval::EVALUATION_VERSION {
                return Err(invalid(
                    "algorithm_version",
                    "Unsupported evaluation algorithm",
                ));
            }
        }
        _ => return Err(invalid("operation", "Unsupported report operation")),
    }
    for variant in variants {
        if variant.generation_algorithm != ALGORITHM_VERSION {
            return Err(ArchyError::new(
                ErrorCode::ArtifactInvalid,
                "variants.generation_algorithm",
                "Unsupported variant generation algorithm",
            ));
        }
    }
    Ok(())
}

pub fn render_report(
    spec: &NormalizedSpec,
    variants: &[Variant],
    evaluations: &[EvaluationReport],
    context: &ReportContext,
) -> Result<ReportArtifacts, ArchyError> {
    validate_context(context, variants)?;
    site_bounds(spec)?;
    if variants.len() != evaluations.len() {
        return Err(ArchyError::new(
            ErrorCode::ArtifactInvalid,
            "evaluations",
            "Every variant needs one evaluation",
        ));
    }
    let mut by_id = BTreeMap::new();
    for variant in variants {
        variant.validate()?;
        if variant.project_id != spec.project_id {
            return Err(ArchyError::schema(
                "variants.project_id",
                "Variant belongs to another project",
            ));
        }
        if by_id.insert(variant.id.as_str(), variant).is_some() {
            return Err(ArchyError::new(
                ErrorCode::ArtifactInvalid,
                "variants",
                "Duplicate variant ID",
            ));
        }
    }
    let mut reports = BTreeMap::new();
    for evaluation in evaluations {
        let variant = by_id.get(evaluation.variant_id.as_str()).ok_or_else(|| {
            ArchyError::new(
                ErrorCode::ArtifactInvalid,
                "evaluations.variant_id",
                "Evaluation has no canonical variant",
            )
        })?;
        let fresh = evaluate_variant(spec, variant)?;
        if evaluation != &fresh
            || reports
                .insert(evaluation.variant_id.as_str(), fresh)
                .is_some()
        {
            return Err(ArchyError::new(
                ErrorCode::ArtifactInvalid,
                "evaluations",
                "Stored evaluation differs from fresh recomputation or is duplicated",
            ));
        }
    }
    let variants = by_id
        .into_iter()
        .map(|(id, variant)| ReportVariant {
            variant: variant.clone(),
            evaluation: reports.remove(id).expect("count and associations checked"),
        })
        .collect();
    let document = ReportDocument {
        schema_version: REPORT_SCHEMA_VERSION.into(),
        context: context.clone(),
        spec: spec.clone(),
        variants,
        uncomputed: vec!["daylight".into(), "cost".into()],
        professional_limits: archy_eval::PROFESSIONAL_LIMITS.into(),
    };
    let json = serde_json::to_vec_pretty(&document).map_err(|error| {
        ArchyError::new(ErrorCode::InternalError, "report.json", error.to_string())
    })?;
    let markdown = templates::markdown(&document).into_bytes();
    Ok(ReportArtifacts { json, markdown })
}
