use archy_core::decision_contract::*;
use archy_core::{CheckResult, InputFormat, ShapeParameters, Variant, parse_project, sha256_hex};

fn evidence(
    x: f64,
    hard: bool,
    no_constraints: bool,
    long_source: bool,
) -> (ReviewEvidence, Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.json"
    ))
    .unwrap();
    source["brief"]["target_gfa_hard"] = hard.into();
    if no_constraints {
        source["constraints"] = serde_json::json!([]);
    } else if long_source {
        source["constraints"][0]["source"] = "é".repeat(5000).into();
    }
    let spec = parse_project(&source.to_string(), InputFormat::Json).unwrap();
    let variant = Variant::new(
        spec.project_id.clone(),
        42,
        "chacha8-u53-massing-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [x, 3.0],
            width_m: 10.0,
            depth_m: 20.0,
        },
        3,
        3.2,
    )
    .unwrap();
    let evaluation = archy_eval::evaluate_variant(&spec, &variant).unwrap();
    let normalized = serde_json::to_vec(&spec).unwrap();
    let variant_bytes = serde_json::to_vec(&variant).unwrap();
    let evaluation_bytes = serde_json::to_vec(&evaluation).unwrap();
    let evidence = ReviewEvidence {
        project_id: spec.project_id.clone(),
        variant_id: variant.id.clone(),
        source_run_id: "00000000-0000-4000-8000-000000000000".into(),
        expected_project_revision: 1,
        expected_head_sha256: "a".repeat(64),
        input_sha256: "b".repeat(64),
        normalized_sha256: sha256_hex(&normalized),
        variant_sha256: sha256_hex(&variant_bytes),
        evaluation_sha256: sha256_hex(&evaluation_bytes),
        comparison_sha256: "e".repeat(64),
        constraints: spec
            .constraints
            .iter()
            .map(ConstraintEvidence::from_normalized)
            .collect(),
        checks: evaluation.checks,
    };
    (evidence, normalized, variant_bytes, evaluation_bytes)
}

#[test]
fn actual_evaluator_checks_bind_and_missing_or_modified_checks_are_rejected() {
    for (x, pass) in [(3.0, true), (-1.0, false)] {
        let (e, n, v, evaluation) = evidence(x, false, true, false);
        e.validate().unwrap();
        assert_eq!(
            e.claimed_checks_passed_against(&n, &v, &evaluation)
                .unwrap(),
            pass
        );
        let mut omitted = e.clone();
        omitted
            .checks
            .retain(|c| c.constraint_id != "site_containment");
        assert!(
            omitted
                .claimed_checks_passed_against(&n, &v, &evaluation)
                .is_err()
        );
        let mut changed = e.clone();
        changed.checks[0].result = CheckResult::Pass;
        if !pass {
            assert!(
                changed
                    .claimed_checks_passed_against(&n, &v, &evaluation)
                    .is_err()
            );
        }
        let mut changed = e.clone();
        changed.checks[0].required = false;
        assert!(
            changed
                .claimed_checks_passed_against(&n, &v, &evaluation)
                .is_err()
        );
    }
}

#[test]
fn hard_target_is_required_and_cannot_be_removed_even_from_rehashed_evaluation() {
    let (mut e, n, v, evaluation) = evidence(3.0, true, false, false);
    assert!(e.checks.iter().any(|c| c.constraint_id == "target_gfa"));
    assert!(
        !e.claimed_checks_passed_against(&n, &v, &evaluation)
            .unwrap()
    );
    e.checks.retain(|c| c.constraint_id != "target_gfa");
    let mut altered: serde_json::Value = serde_json::from_slice(&evaluation).unwrap();
    altered["checks"] = serde_json::to_value(&e.checks).unwrap();
    altered["required_checks_passed"] = true.into();
    let bytes = serde_json::to_vec(&altered).unwrap();
    e.evaluation_sha256 = sha256_hex(&bytes);
    assert!(e.claimed_checks_passed_against(&n, &v, &bytes).is_err());
}

#[test]
fn bound_required_unknown_and_warn_never_report_a_pass() {
    for result in ["UNKNOWN", "WARN", "FAIL"] {
        let (mut e, n, v, evaluation) = evidence(3.0, false, false, false);
        let mut altered: serde_json::Value = serde_json::from_slice(&evaluation).unwrap();
        altered["checks"][0]["result"] = result.into();
        altered["required_checks_passed"] = false.into();
        let bytes = serde_json::to_vec(&altered).unwrap();
        e.checks[0].result = match result {
            "UNKNOWN" => CheckResult::Unknown,
            "WARN" => CheckResult::Warn,
            _ => CheckResult::Fail,
        };
        e.evaluation_sha256 = sha256_hex(&bytes);
        assert!(!e.claimed_checks_passed_against(&n, &v, &bytes).unwrap());
    }
}

#[test]
fn evaluator_explanation_preserves_long_valid_upstream_source() {
    let (e, n, v, evaluation) = evidence(3.0, false, false, true);
    assert!(
        e.claimed_checks_passed_against(&n, &v, &evaluation)
            .unwrap()
    );
    assert_eq!(e.constraints[0].source.chars().count(), 5000);
}
