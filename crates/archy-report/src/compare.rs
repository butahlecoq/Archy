use archy_core::{
    ArchyError, CheckResult, ConstraintKind, ErrorCode, EvaluationReport, Metric, NormalizedSpec,
    Variant,
};
use archy_eval::PROFESSIONAL_LIMITS;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub const COMPARISON_SCHEMA_VERSION: &str = "archy-comparison-v1";
pub const COMPARISON_POLICY_VERSION: &str = "absolute-target-gfa-delta-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonSource {
    pub run_id: String,
    pub manifest_sha256: String,
    pub input_sha256: String,
    pub normalized_sha256: String,
    pub variant_sha256: Vec<String>,
    pub evaluation_sha256: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComparisonDocument {
    pub schema_version: String,
    pub policy_version: String,
    pub project_id: String,
    pub observed_revision: u64,
    pub observed_head_sha256: Option<String>,
    pub source_run_id: String,
    pub source_manifest_sha256: String,
    pub source_input_sha256: String,
    pub source_normalized_sha256: String,
    pub objective_description: String,
    pub constraints: Vec<ComparisonConstraint>,
    pub candidates: Vec<ComparisonCandidate>,
    pub subjective_preferences: Vec<String>,
    pub uncomputed: Vec<String>,
    pub professional_limits: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComparisonConstraint {
    pub id: String,
    pub kind: ConstraintKind,
    pub bound: f64,
    pub unit: String,
    pub source: String,
    pub authority: String,
    pub authority_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComparisonCandidate {
    pub variant: Variant,
    pub evaluation: EvaluationReport,
    pub variant_sha256: String,
    pub evaluation_sha256: String,
    pub objective: Option<Metric>,
    pub objective_source_metric: Option<String>,
    pub objective_rank: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonArtifacts {
    pub json: Vec<u8>,
    pub markdown: Vec<u8>,
}

pub fn comparison_schema() -> schemars::Schema {
    schemars::schema_for!(ComparisonDocument)
}

fn invalid(message: &str) -> ArchyError {
    ArchyError::new(ErrorCode::ArtifactInvalid, "comparison", message)
}

pub fn render_comparison(
    spec: &NormalizedSpec,
    variants: &[Variant],
    evaluations: &[EvaluationReport],
    source: &ComparisonSource,
    revision: u64,
    head: Option<String>,
) -> Result<ComparisonArtifacts, ArchyError> {
    if variants.len() != evaluations.len()
        || variants.len() != source.variant_sha256.len()
        || variants.len() != source.evaluation_sha256.len()
        || variants.len() > 1000
    {
        return Err(invalid("candidate evidence counts differ or exceed 1000"));
    }
    let mut candidates = Vec::with_capacity(variants.len());
    for (index, (variant, evaluation)) in variants.iter().zip(evaluations).enumerate() {
        if evaluation.variant_id != variant.id || variant.project_id != spec.project_id {
            return Err(invalid("candidate identity differs from evidence"));
        }
        let delta = evaluation
            .metrics
            .iter()
            .find(|m| m.name == "target_gfa_delta");
        let objective = match delta {
            Some(m) if m.unit == "m2" && m.value.is_finite() => Some(Metric {
                name: "absolute_target_gfa_delta".into(),
                value: if m.value == 0.0 { 0.0 } else { m.value.abs() },
                unit: "m2".into(),
                method_version: COMPARISON_POLICY_VERSION.into(),
            }),
            Some(_) => return Err(invalid("target GFA delta has invalid unit or value")),
            None => None,
        };
        candidates.push(ComparisonCandidate {
            variant: variant.clone(),
            evaluation: evaluation.clone(),
            variant_sha256: source.variant_sha256[index].clone(),
            evaluation_sha256: source.evaluation_sha256[index].clone(),
            objective_source_metric: objective.as_ref().map(|_| "target_gfa_delta".into()),
            objective,
            objective_rank: None,
        });
    }
    candidates.sort_by(|a, b| {
        match (&a.objective, &b.objective) {
            (Some(x), Some(y)) => x.value.total_cmp(&y.value),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| a.variant.id.cmp(&b.variant.id))
        .then_with(|| a.variant_sha256.cmp(&b.variant_sha256))
    });
    let mut previous = None;
    let mut rank = 0;
    for candidate in &mut candidates {
        if let Some(objective) = &candidate.objective {
            if previous != Some(objective.value) {
                rank += 1;
                previous = Some(objective.value);
            }
            candidate.objective_rank = Some(rank);
        }
    }
    let constraints = spec
        .constraints
        .iter()
        .map(|c| ComparisonConstraint {
            id: c.id.clone(),
            kind: c.kind,
            bound: c.value_m,
            unit: "m".into(),
            source: c.source.clone(),
            authority: c.authority.clone(),
            authority_status: "unverified declaration".into(),
        })
        .collect();
    let document = ComparisonDocument {
        schema_version: COMPARISON_SCHEMA_VERSION.into(), policy_version: COMPARISON_POLICY_VERSION.into(),
        project_id: spec.project_id.clone(), observed_revision: revision, observed_head_sha256: head,
        source_run_id: source.run_id.clone(), source_manifest_sha256: source.manifest_sha256.clone(),
        source_input_sha256: source.input_sha256.clone(), source_normalized_sha256: source.normalized_sha256.clone(),
        objective_description: "Ascending absolute value of measured target_gfa_delta in m2; exact ties share a dense rank. Rank does not imply eligibility or compliance. Area tolerance is reported per evaluation.".into(),
        constraints, candidates, subjective_preferences: spec.preferences.clone(),
        uncomputed: vec!["daylight".into(), "cost".into()],
        professional_limits: PROFESSIONAL_LIMITS.into(),
    };
    let json = serde_json::to_vec_pretty(&document).map_err(|e| invalid(&e.to_string()))?;
    let markdown = markdown(&document).into_bytes();
    Ok(ComparisonArtifacts { json, markdown })
}

pub(crate) fn escaped(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            character
                if character.is_control()
                    || matches!(character, '\u{061c}' | '\u{200e}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{206f}') =>
            {
                write!(out, "\\[U+{:04X}\\]", character as u32).unwrap();
            }
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' | '~' => {
                out.push('\\');
                out.push(ch);
            }
            character => out.push(character),
        }
    }
    out
}

fn markdown(doc: &ComparisonDocument) -> String {
    let mut out = String::from("# Archy passive comparison\n\n");
    writeln!(out, "Schema: {}  ", escaped(&doc.schema_version)).unwrap();
    writeln!(out, "Project: {}  ", escaped(&doc.project_id)).unwrap();
    writeln!(out, "Source run: {}  ", escaped(&doc.source_run_id)).unwrap();
    writeln!(
        out,
        "Source manifest SHA-256: {}  ",
        escaped(&doc.source_manifest_sha256)
    )
    .unwrap();
    writeln!(
        out,
        "Source input SHA-256: {}  ",
        escaped(&doc.source_input_sha256)
    )
    .unwrap();
    writeln!(
        out,
        "Normalized SHA-256: {}  ",
        escaped(&doc.source_normalized_sha256)
    )
    .unwrap();
    writeln!(out, "Observed revision: {}  ", doc.observed_revision).unwrap();
    writeln!(
        out,
        "Observed head SHA-256: {}\n",
        escaped(doc.observed_head_sha256.as_deref().unwrap_or("none"))
    )
    .unwrap();
    writeln!(out, "Objective: {}\n", escaped(&doc.objective_description)).unwrap();
    out.push_str("## Declared constraints\n\n");
    for c in &doc.constraints {
        writeln!(
            out,
            "- {}: {:?}, bound {} {}; source: {}; authority: {} ({})",
            escaped(&c.id),
            c.kind,
            c.bound,
            escaped(&c.unit),
            escaped(&c.source),
            escaped(&c.authority),
            escaped(&c.authority_status)
        )
        .unwrap();
    }
    out.push_str("\n## Candidates\n\n");
    for c in &doc.candidates {
        writeln!(out, "### {}\n", escaped(&c.variant.id)).unwrap();
        writeln!(
            out,
            "Rank: {}; objective: {} m2; required checks passed: {}  ",
            c.objective_rank
                .map_or_else(|| "uncomputed".into(), |n| n.to_string()),
            c.objective
                .as_ref()
                .map_or_else(|| "uncomputed".into(), |m| m.value.to_string()),
            c.evaluation.required_checks_passed
        )
        .unwrap();
        writeln!(out, "Variant SHA-256: {}  ", escaped(&c.variant_sha256)).unwrap();
        writeln!(
            out,
            "Evaluation SHA-256: {}  ",
            escaped(&c.evaluation_sha256)
        )
        .unwrap();
        writeln!(
            out,
            "Shape: {:?}; storeys: {}; floor-to-floor: {} m  ",
            c.variant.shape, c.variant.storeys, c.variant.floor_to_floor_m
        )
        .unwrap();
        writeln!(
            out,
            "Evaluation: {}; length tolerance: {} m; area tolerance: {} m2\n",
            escaped(&c.evaluation.evaluation_version),
            c.evaluation.tolerance.length_m,
            c.evaluation.tolerance.area_m2
        )
        .unwrap();
        out.push_str("Metrics:\n\n");
        for m in &c.evaluation.metrics {
            writeln!(
                out,
                "- {}: {} {} (method: {})",
                escaped(&m.name),
                m.value,
                escaped(&m.unit),
                escaped(&m.method_version)
            )
            .unwrap();
        }
        out.push_str("\nChecks:\n\n");
        for check in &c.evaluation.checks {
            let unit = if check.constraint_id == "target_gfa" {
                "m2"
            } else {
                "m"
            };
            let fmt =
                |v: Option<f64>| v.map_or_else(|| "uncomputed".into(), |x| format!("{x} {unit}"));
            let status = match check.result {
                CheckResult::Pass => "PASS",
                CheckResult::Fail => "FAIL",
                CheckResult::Warn => "WARN",
                CheckResult::Unknown => "UNKNOWN",
            };
            writeln!(
                out,
                "- {}: {}; measured: {}; limit: {}; required: {}; {}",
                escaped(&check.constraint_id),
                status,
                fmt(check.measured),
                fmt(check.limit),
                check.required,
                escaped(&check.explanation)
            )
            .unwrap();
        }
        out.push('\n');
    }
    out.push_str("## Subjective preferences\n\n");
    for p in &doc.subjective_preferences {
        writeln!(out, "- {}", escaped(p)).unwrap();
    }
    if doc.subjective_preferences.is_empty() {
        out.push_str("None declared.\n");
    }
    out.push_str(
        "\n## Uncomputed\n\nDaylight: uncomputed  \nCost: uncomputed\n\n## Professional limits\n\n",
    );
    out.push_str(&escaped(&doc.professional_limits));
    out.push('\n');
    out
}
