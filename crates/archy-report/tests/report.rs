use archy_core::{CheckResult, ErrorCode, InputFormat, ShapeParameters, Variant, parse_project};
use archy_eval::evaluate_variant;
use archy_generate::generate_variants;
use archy_report::{ReportContext, render_report, report_schema};

fn data() -> (
    archy_core::NormalizedSpec,
    Vec<Variant>,
    Vec<archy_core::EvaluationReport>,
    ReportContext,
) {
    let spec = parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap();
    let rectangle = Variant::new(
        spec.project_id.clone(),
        42,
        "chacha8-u53-massing-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [3.0, 3.0],
            width_m: 10.0,
            depth_m: 20.0,
        },
        3,
        3.2,
    )
    .unwrap();
    let l_shape = Variant::new(
        spec.project_id.clone(),
        42,
        "chacha8-u53-massing-v1".into(),
        ShapeParameters::LShape {
            origin_xy_m: [3.0, 3.0],
            width_m: 10.0,
            depth_m: 10.0,
            notch_width_m: 5.0,
            notch_depth_m: 5.0,
        },
        3,
        3.2,
    )
    .unwrap();
    let evaluations = vec![
        evaluate_variant(&spec, &rectangle).unwrap(),
        evaluate_variant(&spec, &l_shape).unwrap(),
    ];
    let context = ReportContext {
        schema_version: "archy-report-context-v1".into(),
        run_id: "run-1".into(),
        parent_run_id: None,
        parent_variant_id: None,
        operation: "generate".into(),
        input_sha256: "a".repeat(64),
        normalized_sha256: "b".repeat(64),
        algorithm_version: "chacha8-u53-massing-v1".into(),
        build_version: "test-build".into(),
        requested_count: Some(3),
        attempts: Some(300),
        exhaustion_explanations: vec!["Only 2 distinct designs".into()],
    };
    (spec, vec![rectangle, l_shape], evaluations, context)
}

#[test]
fn json_and_markdown_preserve_declared_evidence_and_stable_order() {
    let (spec, variants, evaluations, context) = data();
    let first = render_report(&spec, &variants, &evaluations, &context).unwrap();
    let reversed = render_report(
        &spec,
        &variants.iter().rev().cloned().collect::<Vec<_>>(),
        &evaluations.iter().rev().cloned().collect::<Vec<_>>(),
        &context,
    )
    .unwrap();
    assert_eq!(first, reversed);
    let doc: serde_json::Value = serde_json::from_slice(&first.json).unwrap();
    assert_eq!(doc["schema_version"], "archy-report-v1");
    assert_eq!(doc["context"]["run_id"], "run-1");
    assert_eq!(
        doc["context"]["exhaustion_explanations"][0],
        "Only 2 distinct designs"
    );
    assert_eq!(doc["variants"].as_array().unwrap().len(), 2);
    assert_eq!(
        doc["variants"][0]["variant"]["id"].as_str().unwrap(),
        variants
            .iter()
            .map(|variant| variant.id.as_str())
            .min()
            .unwrap()
    );
    assert!(
        doc["variants"][0]["evaluation"]["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|metric| metric["name"] == "gross_floor_area"
                && metric["unit"] == "m2"
                && metric["method_version"] == "planar-massing-v1")
    );
    let combined_area: f64 = doc["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            entry["evaluation"]["metrics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|metric| metric["name"] == "footprint_area")
                .unwrap()["value"]
                .as_f64()
                .unwrap()
        })
        .sum();
    assert_eq!(combined_area, 275.0);
    assert_eq!(doc["spec"]["constraints"][0]["source"], "user_brief");
    assert_eq!(doc["spec"]["constraints"][0]["authority"], "assumption");
    assert!(doc.get("approval").is_none());
    let markdown = std::str::from_utf8(&first.markdown).unwrap();
    for term in [
        "Declared constraints",
        "Preferences",
        "Professional limits",
        "Daylight: uncomputed",
        "Cost: uncomputed",
        "Only 2 distinct designs",
        "gross\\_floor\\_area",
        "planar\\-massing\\-v1",
        "user\\_brief",
        "assumption",
    ] {
        assert!(markdown.contains(term), "missing {term}");
    }
}

#[test]
fn forged_evaluation_and_canonical_variant_are_rejected() {
    let (spec, variants, evaluations, context) = data();
    let mut cases = vec![];
    let mut forged = evaluations.clone();
    forged[0].required_checks_passed = !forged[0].required_checks_passed;
    cases.push(forged);
    let mut forged = evaluations.clone();
    forged[0].metrics[0].value += 1.0;
    cases.push(forged);
    let mut forged = evaluations.clone();
    forged[0].checks[0].result = CheckResult::Unknown;
    cases.push(forged);
    let mut forged = evaluations.clone();
    forged[0].professional_limits = "approved".into();
    cases.push(forged);
    for forged in cases {
        assert_eq!(
            render_report(&spec, &variants, &forged, &context)
                .unwrap_err()
                .code,
            ErrorCode::ArtifactInvalid
        );
    }
    let mut bad = variants.clone();
    bad[0].id = "variant-forged".into();
    assert!(render_report(&spec, &bad, &evaluations, &context).is_err());
    assert_eq!(
        render_report(&spec, &variants, &evaluations[..1], &context)
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    let mut duplicated = variants.clone();
    duplicated.push(variants[0].clone());
    let mut duplicate_context = context.clone();
    duplicate_context.requested_count = Some(4);
    assert_eq!(
        render_report(&spec, &duplicated, &evaluations, &duplicate_context)
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
}

#[test]
fn invalid_context_and_empty_exhausted_generation_are_distinguished() {
    let (spec, variants, evaluations, mut context) = data();
    context.run_id = "../escape".into();
    assert_eq!(
        render_report(&spec, &variants, &evaluations, &context)
            .unwrap_err()
            .code,
        ErrorCode::InvalidSchema
    );
    context.run_id = "run-1".into();
    context.input_sha256 = "not-a-digest".into();
    assert_eq!(
        render_report(&spec, &variants, &evaluations, &context)
            .unwrap_err()
            .code,
        ErrorCode::InvalidSchema
    );
    context.input_sha256 = "a".repeat(64);
    context.parent_run_id = Some("run-source".into());
    context.parent_variant_id = Some("notcanonical".into());
    assert_eq!(
        render_report(&spec, &variants, &evaluations, &context)
            .unwrap_err()
            .code,
        ErrorCode::InvalidSchema
    );
    context.parent_variant_id = None;
    context.parent_run_id = None;
    let empty = render_report(&spec, &[], &[], &context).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&empty.json).unwrap();
    assert!(document["variants"].as_array().unwrap().is_empty());
    assert_eq!(
        document["context"]["exhaustion_explanations"][0],
        "Only 2 distinct designs"
    );
}

#[test]
fn markdown_untrusted_text_cannot_create_html_headings_tables_or_fences() {
    let (mut spec, variants, _, mut context) = data();
    spec.preferences
        .push("</td>\n# Forged heading\n```rust\n| forged | <script>alert(1)</script>".into());
    spec.constraints[0].source = "<img src=x onerror=alert(1)>\n|cell|".into();
    let evaluations: Vec<_> = variants
        .iter()
        .map(|variant| evaluate_variant(&spec, variant).unwrap())
        .collect();
    context.exhaustion_explanations = vec!["```\n## forged\n<script>".into()];
    let artifacts = render_report(&spec, &variants, &evaluations, &context).unwrap();
    let markdown = String::from_utf8(artifacts.markdown).unwrap();
    assert!(!markdown.contains("<script>"));
    assert!(!markdown.contains("<img"));
    assert!(!markdown.contains("\n# Forged heading"));
    assert!(!markdown.contains("\n```"));
    assert!(!markdown.contains("|cell|"));
    assert!(markdown.contains("&lt;script&gt;"));
    let json: serde_json::Value = serde_json::from_slice(&artifacts.json).unwrap();
    assert_eq!(
        json["spec"]["preferences"]
            .as_array()
            .unwrap()
            .last()
            .unwrap(),
        "</td>\n# Forged heading\n```rust\n| forged | <script>alert(1)</script>"
    );
}

#[test]
fn source_derived_schema_and_serializer_match_golden_fixture() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/report-contracts.json")).unwrap();
    let (spec, _, _, mut context) = data();
    let generation = generate_variants(&spec, 1, 42).unwrap();
    let evaluations: Vec<_> = generation
        .variants
        .iter()
        .map(|variant| evaluate_variant(&spec, variant).unwrap())
        .collect();
    context.run_id = "run-report-fixture".into();
    context.parent_run_id = None;
    context.build_version = "fixture-build".into();
    context.requested_count = Some(generation.requested_count);
    context.attempts = Some(generation.attempts);
    context.exhaustion_explanations = generation.exhaustion_explanations.clone();
    let artifacts = render_report(&spec, &generation.variants, &evaluations, &context).unwrap();
    let actual_report: serde_json::Value = serde_json::from_slice(&artifacts.json).unwrap();
    assert_eq!(actual_report, expected["report"]);
    assert_eq!(
        serde_json::to_value(report_schema()).unwrap(),
        expected["schemas"]["report"]
    );
}

#[test]
fn rejects_unsupported_or_contradictory_operation_provenance() {
    let (spec, variants, evaluations, context) = data();
    for operation in ["approved", "unknown"] {
        let mut candidate = context.clone();
        candidate.operation = operation.into();
        assert!(render_report(&spec, &variants, &evaluations, &candidate).is_err());
    }
    let mut candidate = context.clone();
    candidate.algorithm_version = "future-v9".into();
    assert!(render_report(&spec, &variants, &evaluations, &candidate).is_err());
    let unsupported = Variant::new(
        spec.project_id.clone(),
        42,
        "analytic-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [3.0, 3.0],
            width_m: 10.0,
            depth_m: 20.0,
        },
        3,
        3.2,
    )
    .unwrap();
    let unsupported_evaluation = evaluate_variant(&spec, &unsupported).unwrap();
    assert!(render_report(&spec, &[unsupported], &[unsupported_evaluation], &context).is_err());
}

#[test]
fn generation_counts_and_exhaustion_must_explain_missing_results() {
    let (spec, variants, evaluations, context) = data();
    let mut candidate = context.clone();
    candidate.requested_count = Some(1);
    assert!(render_report(&spec, &variants, &evaluations, &candidate).is_err());
    let mut candidate = context.clone();
    candidate.exhaustion_explanations.clear();
    assert!(render_report(&spec, &variants, &evaluations, &candidate).is_err());
    assert!(render_report(&spec, &[], &[], &candidate).is_err());
    candidate.exhaustion_explanations.push("  ".into());
    assert!(render_report(&spec, &[], &[], &candidate).is_err());
    let mut candidate = context.clone();
    candidate.attempts = None;
    assert!(render_report(&spec, &variants, &evaluations, &candidate).is_err());
    let mut candidate = context.clone();
    candidate.attempts = Some(1);
    assert!(render_report(&spec, &variants, &evaluations, &candidate).is_err());
    let mut candidate = context;
    candidate.requested_count = Some(2);
    assert!(render_report(&spec, &variants, &evaluations, &candidate).is_err());
}

#[test]
fn edit_and_evaluate_contexts_match_cli_semantics() {
    let (spec, variants, evaluations, context) = data();
    let mut edit = context.clone();
    edit.operation = "edit".into();
    edit.parent_run_id = Some("run-source".into());
    edit.parent_variant_id = Some(variants[1].id.clone());
    edit.requested_count = Some(1);
    edit.attempts = None;
    edit.exhaustion_explanations.clear();
    assert!(render_report(&spec, &variants[..1], &evaluations[..1], &edit).is_ok());
    assert!(render_report(&spec, &variants, &evaluations, &edit).is_err());
    let mut missing_parent = edit.clone();
    missing_parent.parent_variant_id = None;
    assert!(render_report(&spec, &variants[..1], &evaluations[..1], &missing_parent).is_err());
    let mut evaluate = context;
    evaluate.operation = "evaluate".into();
    evaluate.parent_run_id = Some("run-source".into());
    evaluate.algorithm_version = "planar-massing-v1".into();
    evaluate.requested_count = None;
    evaluate.attempts = None;
    evaluate.exhaustion_explanations.clear();
    assert!(render_report(&spec, &variants, &evaluations, &evaluate).is_ok());
    evaluate.requested_count = Some(2);
    assert!(render_report(&spec, &variants, &evaluations, &evaluate).is_err());
}

#[test]
fn markdown_check_measurements_have_declared_length_or_area_units() {
    let (mut spec, variants, _, context) = data();
    spec.brief.target_gfa_hard = true;
    let evaluations: Vec<_> = variants
        .iter()
        .map(|variant| evaluate_variant(&spec, variant).unwrap())
        .collect();
    let markdown = String::from_utf8(
        render_report(&spec, &variants, &evaluations, &context)
            .unwrap()
            .markdown,
    )
    .unwrap();
    let site_line = markdown
        .lines()
        .find(|line| line.starts_with("- site\\_containment:"))
        .unwrap();
    assert!(
        site_line.contains("measured: 3 m; limit: 0 m"),
        "{site_line}"
    );
    let height_line = markdown
        .lines()
        .find(|line| line.starts_with("- height:") && line.contains("measured:"))
        .unwrap();
    assert!(height_line.contains(" m; limit: 12 m"), "{height_line}");
    let area_line = markdown
        .lines()
        .find(|line| line.starts_with("- target\\_gfa:"))
        .unwrap();
    assert!(
        area_line.contains("measured: 225 m2; limit: 900 m2"),
        "{area_line}"
    );
}
