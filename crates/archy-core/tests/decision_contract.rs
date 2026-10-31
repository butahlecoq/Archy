use archy_core::decision_contract::*;
use archy_core::{Check, CheckResult, ConstraintKind, ErrorCode, NormalizedConstraint};

fn hash() -> String {
    "a".repeat(64)
}

#[test]
fn normalized_constraint_projects_si_bound_and_declared_provenance() {
    let normalized = NormalizedConstraint {
        id: "setback".into(),
        kind: ConstraintKind::MinEdgeSetback,
        value_m: 3.0,
        source: "user_brief".into(),
        authority: "assumption".into(),
    };
    let projected = ConstraintEvidence::from_normalized(&normalized);
    assert_eq!(projected.bound, 3.0);
    assert_eq!(projected.unit, "m");
    assert_eq!(projected.source, "user_brief");
    assert_eq!(projected.authority, "assumption");
}

fn evidence() -> ReviewEvidence {
    ReviewEvidence {
        project_id: "courtyard".into(),
        variant_id: format!("variant-{}", hash()),
        source_run_id: "00000000-0000-4000-8000-000000000000".into(),
        expected_project_revision: 4,
        expected_head_sha256: hash(),
        input_sha256: hash(),
        normalized_sha256: hash(),
        variant_sha256: hash(),
        evaluation_sha256: hash(),
        comparison_sha256: hash(),
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
    }
}

#[test]
fn review_evidence_keeps_claimed_authority_unknown_beside_geometry_pass() {
    let mut e = evidence();
    e.constraints[0].source = "claimed_regulation".into();
    e.checks.push(Check {
        constraint_id: "authority:height".into(),
        result: CheckResult::Unknown,
        measured: None,
        limit: None,
        explanation: "Unverified legal authority declaration".into(),
        required: true,
    });
    e.validate().unwrap();
    assert_eq!(e.checks[1].result, CheckResult::Pass);
    assert!(!archy_core::required_checks_pass(&e.checks));
    e.checks[2].result = CheckResult::Pass;
    assert_eq!(e.validate().unwrap_err().code, ErrorCode::InvalidSchema);
}

#[test]
fn proposal_preserves_evidence_but_rejects_actor_and_decision_in_json() {
    let proposal = Proposal {
        schema_version: PROPOSAL_SCHEMA_VERSION.into(),
        evidence: evidence(),
        suggested_reason: "Review massing".into(),
    };
    proposal.validate().unwrap();
    let mut value = serde_json::to_value(&proposal).unwrap();
    assert_eq!(
        value["evidence"]["constraints"][0]["authority"],
        "assumption"
    );
    value["actor"] = "human".into();
    assert!(serde_json::from_value::<Proposal>(value).is_err());
    let mut value = serde_json::to_value(&proposal).unwrap();
    value["decision"] = "select".into();
    assert!(serde_json::from_value::<Proposal>(value).is_err());
}

#[test]
fn review_evidence_rejects_missing_binding_and_unknown_check_remains_nonpassing() {
    let mut e = evidence();
    e.variant_sha256 = "bad".into();
    assert_eq!(e.validate().unwrap_err().code, ErrorCode::InvalidSchema);
    let mut e = evidence();
    e.checks[0].result = CheckResult::Unknown;
    e.validate().unwrap();
    e.constraints[0].source.clear();
    assert_eq!(e.validate().unwrap_err().code, ErrorCode::InvalidSchema);
    let mut e = evidence();
    e.constraints[0].authority.clear();
    assert_eq!(e.validate().unwrap_err().code, ErrorCode::InvalidSchema);
    let mut e = evidence();
    e.checks[0].constraint_id = "invented".into();
    assert_eq!(e.validate().unwrap_err().code, ErrorCode::InvalidSchema);
}

#[test]
fn serialized_decision_is_only_a_claimed_record_and_must_bind_evidence() {
    let record = DecisionRecord {
        schema_version: DECISION_SCHEMA_VERSION.into(),
        evidence: evidence(),
        actor: ActorRecord {
            principal: "alice".into(),
            mechanism: "unverified".into(),
        },
        timestamp: UnixTimestamp {
            seconds: 1_800_000_000,
            nanos: 0,
        },
        decision: DecisionKind::Select,
        reason: "Chosen for review".into(),
    };
    record.validate().unwrap();
    let json = serde_json::to_string(&record).unwrap();
    let parsed: DecisionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.evidence.variant_id, format!("variant-{}", hash()));
    let mut invalid = parsed;
    invalid.timestamp.nanos = 1_000_000_000;
    assert_eq!(
        invalid.validate().unwrap_err().code,
        ErrorCode::InvalidSchema
    );
    invalid.timestamp.nanos = 0;
    invalid.evidence.checks[0].result = CheckResult::Unknown;
    invalid.validate().unwrap();
    invalid.decision = DecisionKind::Withdraw;
    invalid.validate().unwrap();
}
