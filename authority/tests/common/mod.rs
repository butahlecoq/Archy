use archy_core::decision_contract::{
    ConstraintEvidence, PROPOSAL_SCHEMA_VERSION, Proposal, ReviewEvidence,
};
use archy_core::{Check, CheckResult, ConstraintKind};

pub fn valid() -> Vec<u8> {
    let hash = "a".repeat(64);
    let p = Proposal {
        schema_version: PROPOSAL_SCHEMA_VERSION.into(),
        evidence: ReviewEvidence {
            project_id: "courtyard".into(),
            variant_id: format!("variant-{hash}"),
            source_run_id: "00000000-0000-4000-8000-000000000000".into(),
            expected_project_revision: 4,
            expected_head_sha256: hash.clone(),
            input_sha256: hash.clone(),
            normalized_sha256: hash.clone(),
            variant_sha256: hash.clone(),
            evaluation_sha256: hash.clone(),
            comparison_sha256: hash,
            constraints: vec![ConstraintEvidence {
                id: "height".into(),
                kind: ConstraintKind::MaxHeight,
                bound: 12.0,
                unit: "m".into(),
                source: "user_brief".into(),
                authority: "assumption".into(),
            }],
            checks: vec![
                Check {
                    constraint_id: "site_containment".into(),
                    result: CheckResult::Pass,
                    measured: Some(3.0),
                    limit: Some(0.0),
                    explanation: "site".into(),
                    required: true,
                },
                Check {
                    constraint_id: "height".into(),
                    result: CheckResult::Pass,
                    measured: Some(9.6),
                    limit: Some(12.0),
                    explanation: "height".into(),
                    required: true,
                },
            ],
        },
        suggested_reason: "Review massing".into(),
    };
    serde_json::to_vec(&p).unwrap()
}
