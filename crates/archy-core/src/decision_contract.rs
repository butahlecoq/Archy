//! Untrusted review data. Validation and digest matching never authenticate a decision.
use crate::{
    ArchyError, Check, CheckResult, ConstraintKind, EvaluationReport, NormalizedConstraint,
    NormalizedSpec, Variant, sha256_hex, site_bounds,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const PROPOSAL_SCHEMA_VERSION: &str = "archy-proposal-v1";
pub const DECISION_SCHEMA_VERSION: &str = "archy-decision-record-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConstraintEvidence {
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = r"^[A-Za-z0-9_-]+$(?![\s\S])")
    )]
    pub id: String,
    pub kind: ConstraintKind,
    pub bound: f64,
    pub unit: String,
    pub source: String,
    /// A declaration from input, not proof of legal authority.
    pub authority: String,
}
impl ConstraintEvidence {
    pub fn from_normalized(c: &NormalizedConstraint) -> Self {
        Self {
            id: c.id.clone(),
            kind: c.kind,
            bound: c.value_m,
            unit: "m".into(),
            source: c.source.clone(),
            authority: c.authority.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewEvidence {
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = r"^[A-Za-z0-9_-]+$(?![\s\S])")
    )]
    pub project_id: String,
    #[schemars(regex(pattern = r"^variant-[0-9a-f]{64}$"), length(equal = 72))]
    pub variant_id: String,
    #[schemars(
        regex(
            pattern = r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
        ),
        length(equal = 36)
    )]
    pub source_run_id: String,
    #[schemars(range(min = 1, max = u64::MAX))]
    pub expected_project_revision: u64,
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"), length(equal = 64))]
    pub expected_head_sha256: String,
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"), length(equal = 64))]
    pub input_sha256: String,
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"), length(equal = 64))]
    pub normalized_sha256: String,
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"), length(equal = 64))]
    pub variant_sha256: String,
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"), length(equal = 64))]
    pub evaluation_sha256: String,
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"), length(equal = 64))]
    pub comparison_sha256: String,
    #[schemars(length(max = 4096))]
    pub constraints: Vec<ConstraintEvidence>,
    #[schemars(length(min = 1, max = 4096))]
    pub checks: Vec<Check>,
}

impl ReviewEvidence {
    /// Checks the claim's shape. It does not verify referenced evidence or check status.
    pub fn validate(&self) -> Result<(), ArchyError> {
        crate::validation::identifier(&self.project_id, "project_id")?;
        if !self
            .variant_id
            .strip_prefix("variant-")
            .is_some_and(valid_hash)
        {
            return Err(ArchyError::schema(
                "variant_id",
                "Expected canonical variant identifier",
            ));
        }
        if self.source_run_id.len() != 36 || uuid::Uuid::parse_str(&self.source_run_id).is_err() {
            return Err(ArchyError::schema("source_run_id", "Expected run UUID"));
        }
        if self.expected_project_revision == 0 {
            return Err(ArchyError::schema(
                "expected_project_revision",
                "A source run requires a nonzero revision",
            ));
        }
        for (field, value) in [
            ("expected_head_sha256", &self.expected_head_sha256),
            ("input_sha256", &self.input_sha256),
            ("normalized_sha256", &self.normalized_sha256),
            ("variant_sha256", &self.variant_sha256),
            ("evaluation_sha256", &self.evaluation_sha256),
            ("comparison_sha256", &self.comparison_sha256),
        ] {
            if !valid_hash(value) {
                return Err(ArchyError::schema(field, "Expected lowercase SHA-256 hex"));
            }
        }
        if self.constraints.len() > 4096 || self.checks.len() > 4096 {
            return Err(ArchyError::schema(
                "evidence",
                "Too many constraints or checks",
            ));
        }
        let mut ids = HashSet::new();
        for c in &self.constraints {
            crate::validation::constraint_identifier(&c.id, "constraints.id")?;
            if c.source.trim().is_empty() || c.authority.trim().is_empty() {
                return Err(ArchyError::schema(
                    "constraints",
                    "Constraint provenance is required",
                ));
            }
            if c.unit != "m" || !c.bound.is_finite() || c.bound <= 0.0 {
                return Err(ArchyError::schema(
                    "constraints.bound",
                    "Expected positive finite meter bound",
                ));
            }
            if !ids.insert(c.id.as_str()) {
                return Err(ArchyError::schema("constraints.id", "Duplicate constraint"));
            }
        }
        let mut seen = HashSet::new();
        for check in &self.checks {
            let id = check.constraint_id.as_str();
            if !(ids.contains(id) || matches!(id, "site_containment" | "target_gfa"))
                || !seen.insert(id)
                || !check.required
            {
                return Err(ArchyError::schema(
                    "checks.constraint_id",
                    "Unknown, duplicate or non-required check",
                ));
            }
            if check.explanation.trim().is_empty() {
                return Err(ArchyError::schema(
                    "checks.explanation",
                    "Explanation is required",
                ));
            }
            if check.measured.is_some_and(|n| !n.is_finite())
                || check.limit.is_some_and(|n| !n.is_finite())
            {
                return Err(ArchyError::schema(
                    "checks",
                    "Nonfinite measurement or limit",
                ));
            }
            if let Some(c) = self.constraints.iter().find(|c| c.id == id)
                && check.limit.is_some_and(|v| v != c.bound)
            {
                return Err(ArchyError::schema(
                    "checks.limit",
                    "Limit differs from declared bound",
                ));
            }
            if id == "site_containment" && check.limit.is_some_and(|v| v != 0.0) {
                return Err(ArchyError::schema(
                    "checks.limit",
                    "Site limit must be zero",
                ));
            }
            if check.result == CheckResult::Pass
                && (check.measured.is_none() || check.limit.is_none())
            {
                return Err(ArchyError::schema(
                    "checks.result",
                    "PASS requires measured and limit",
                ));
            }
        }
        if !seen.contains("site_containment") || ids.iter().any(|id| !seen.contains(id)) {
            return Err(ArchyError::schema(
                "checks",
                "Missing site or declared constraint check",
            ));
        }
        Ok(())
    }

    /// Bind the projection to exact normalized, variant and evaluation bytes and their inventory.
    /// This does not prove their origin, recompute the evaluator, verify history, or authorize a decision.
    pub fn claimed_checks_passed_against(
        &self,
        normalized: &[u8],
        variant: &[u8],
        evaluation: &[u8],
    ) -> Result<bool, ArchyError> {
        self.validate()?;
        for (field, bytes, digest) in [
            ("normalized_sha256", normalized, &self.normalized_sha256),
            ("variant_sha256", variant, &self.variant_sha256),
            ("evaluation_sha256", evaluation, &self.evaluation_sha256),
        ] {
            if sha256_hex(bytes) != *digest {
                return Err(ArchyError::schema(
                    field,
                    "Evidence bytes do not match digest",
                ));
            }
        }
        let spec: NormalizedSpec = serde_json::from_slice(normalized)
            .map_err(|_| ArchyError::schema("normalized", "Invalid normalized project"))?;
        let variant: Variant = serde_json::from_slice(variant)
            .map_err(|_| ArchyError::schema("variant", "Invalid variant"))?;
        let evaluation: EvaluationReport = serde_json::from_slice(evaluation)
            .map_err(|_| ArchyError::schema("evaluation", "Invalid evaluation"))?;
        site_bounds(&spec)?;
        if evaluation.evaluation_version != "planar-massing-v1" {
            return Err(ArchyError::schema(
                "evaluation_version",
                "Unsupported evaluation version",
            ));
        }
        if spec.project_id != self.project_id
            || variant.project_id != self.project_id
            || variant.id != self.variant_id
            || evaluation.variant_id != self.variant_id
        {
            return Err(ArchyError::schema(
                "evidence",
                "Project or variant identity mismatch",
            ));
        }
        let projected: Vec<_> = spec
            .constraints
            .iter()
            .map(ConstraintEvidence::from_normalized)
            .collect();
        if projected != self.constraints || evaluation.checks != self.checks {
            return Err(ArchyError::schema(
                "evidence",
                "Projection differs from bound evidence",
            ));
        }
        let has_target = self.checks.iter().any(|c| c.constraint_id == "target_gfa");
        if has_target != spec.brief.target_gfa_hard {
            return Err(ArchyError::schema(
                "checks",
                "Hard target check inventory differs from brief",
            ));
        }
        if has_target
            && self
                .checks
                .iter()
                .find(|c| c.constraint_id == "target_gfa")
                .and_then(|c| c.limit)
                .is_some_and(|v| v != spec.brief.target_gfa_m2)
        {
            return Err(ArchyError::schema(
                "checks.limit",
                "Target limit differs from brief",
            ));
        }
        Ok(self.checks.iter().all(|c| c.result == CheckResult::Pass))
    }
}

/// Schema validation is structural; callers must also call `validate` and bind evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    #[schemars(regex(pattern = r"^archy-proposal-v1$"), length(equal = 17))]
    pub schema_version: String,
    pub evidence: ReviewEvidence,
    #[schemars(
        length(min = 1, max = 4096),
        regex(
            pattern = r"[^\s\x00-\x1f\x7f-\x9f\u00ad\u0300-\u036f\u061c\u180e\u1ab0-\u1aff\u1dc0-\u1dff\u200b-\u200f\u202a-\u202e\u2060-\u206f\u20d0-\u20ff\ufe00-\ufe0f\ufe20-\ufe2f\ufeff]"
        )
    )]
    pub suggested_reason: String,
}
impl Proposal {
    pub fn validate(&self) -> Result<(), ArchyError> {
        version(&self.schema_version, PROPOSAL_SCHEMA_VERSION)?;
        self.evidence.validate()?;
        text("suggested_reason", &self.suggested_reason)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorRecord {
    #[schemars(
        length(min = 1, max = 4096),
        regex(
            pattern = r"[^\s\x00-\x1f\x7f-\x9f\u00ad\u0300-\u036f\u061c\u180e\u1ab0-\u1aff\u1dc0-\u1dff\u200b-\u200f\u202a-\u202e\u2060-\u206f\u20d0-\u20ff\ufe00-\ufe0f\ufe20-\ufe2f\ufeff]"
        )
    )]
    pub principal: String,
    #[schemars(
        length(min = 1, max = 4096),
        regex(
            pattern = r"[^\s\x00-\x1f\x7f-\x9f\u00ad\u0300-\u036f\u061c\u180e\u1ab0-\u1aff\u1dc0-\u1dff\u200b-\u200f\u202a-\u202e\u2060-\u206f\u20d0-\u20ff\ufe00-\ufe0f\ufe20-\ufe2f\ufeff]"
        )
    )]
    pub mechanism: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnixTimestamp {
    #[schemars(range(max = u64::MAX))]
    pub seconds: u64,
    #[schemars(range(max = 999999999))]
    pub nanos: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Select,
    Withdraw,
}

/// A serialized claim, never an authorization receipt or approved-state token. Schema
/// validation is structural; callers must also call `validate` and bind evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionRecord {
    #[schemars(regex(pattern = r"^archy-decision-record-v1$"), length(equal = 24))]
    pub schema_version: String,
    pub evidence: ReviewEvidence,
    pub actor: ActorRecord,
    pub timestamp: UnixTimestamp,
    pub decision: DecisionKind,
    #[schemars(
        length(min = 1, max = 4096),
        regex(
            pattern = r"[^\s\x00-\x1f\x7f-\x9f\u00ad\u0300-\u036f\u061c\u180e\u1ab0-\u1aff\u1dc0-\u1dff\u200b-\u200f\u202a-\u202e\u2060-\u206f\u20d0-\u20ff\ufe00-\ufe0f\ufe20-\ufe2f\ufeff]"
        )
    )]
    pub reason: String,
}
impl DecisionRecord {
    pub fn validate(&self) -> Result<(), ArchyError> {
        version(&self.schema_version, DECISION_SCHEMA_VERSION)?;
        self.evidence.validate()?;
        text("actor.principal", &self.actor.principal)?;
        text("actor.mechanism", &self.actor.mechanism)?;
        text("reason", &self.reason)?;
        if self.timestamp.nanos >= 1_000_000_000 {
            return Err(ArchyError::schema("timestamp", "Nanoseconds out of range"));
        }
        Ok(())
    }
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn text(field: &str, s: &str) -> Result<(), ArchyError> {
    let visible = s.chars().any(|c| !c.is_whitespace() && !c.is_control() && !matches!(c, '\u{00ad}' | '\u{0300}'..='\u{036f}' | '\u{1ab0}'..='\u{1aff}' | '\u{1dc0}'..='\u{1dff}' | '\u{20d0}'..='\u{20ff}' | '\u{fe20}'..='\u{fe2f}' | '\u{061c}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}' | '\u{e0100}'..='\u{e01ef}'));
    if !visible || s.chars().count() > 4096 {
        Err(ArchyError::schema(
            field,
            "Expected visible text of at most 4096 Unicode characters",
        ))
    } else {
        Ok(())
    }
}
fn version(actual: &str, expected: &str) -> Result<(), ArchyError> {
    if actual != expected {
        Err(ArchyError::schema(
            "schema_version",
            "Unsupported contract version",
        ))
    } else {
        Ok(())
    }
}
