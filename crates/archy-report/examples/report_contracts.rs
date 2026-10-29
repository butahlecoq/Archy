use archy_core::{InputFormat, parse_project, parse_project_current};
use archy_eval::{evaluate_variant, evaluate_variant_v2};
use archy_generate::{ALGORITHM_VERSION, generate_variants};
use archy_report::{
    ReportContext, render_report, render_report_v2, report_schema, report_schema_v2,
};

fn main() {
    let spec = parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap();
    let generation = generate_variants(&spec, 1, 42).unwrap();
    let evaluations: Vec<_> = generation
        .variants
        .iter()
        .map(|variant| evaluate_variant(&spec, variant).unwrap())
        .collect();
    let context = ReportContext {
        schema_version: "archy-report-context-v1".into(),
        run_id: "run-report-fixture".into(),
        parent_run_id: None,
        parent_variant_id: None,
        operation: "generate".into(),
        input_sha256: "a".repeat(64),
        normalized_sha256: "b".repeat(64),
        algorithm_version: ALGORITHM_VERSION.into(),
        build_version: "fixture-build".into(),
        requested_count: Some(generation.requested_count),
        attempts: Some(generation.attempts),
        exhaustion_explanations: generation.exhaustion_explanations.clone(),
        attempt_diagnostics: generation.attempt_diagnostics.clone(),
    };
    let artifacts = render_report(&spec, &generation.variants, &evaluations, &context).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&artifacts.json).unwrap();
    let current = parse_project_current(
        include_str!("../../../tests/fixtures/briefs/courtyard-v2.json"),
        InputFormat::Json,
    )
    .unwrap();
    let current_evaluations: Vec<_> = generation
        .variants
        .iter()
        .map(|variant| evaluate_variant_v2(&current.numeric, variant).unwrap())
        .collect();
    let current_artifacts = render_report_v2(
        &current.numeric,
        &current.canonical,
        &generation.variants,
        &current_evaluations,
        &context,
    )
    .unwrap();
    let current_report: serde_json::Value =
        serde_json::from_slice(&current_artifacts.json).unwrap();
    println!(
        "{}",
        serde_json::json!({"schemas": {"report": report_schema_v2(), "report-v1": report_schema()}, "report": current_report, "report_v1": report})
    );
}
