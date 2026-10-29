use archy_core::{InputFormat, parse_project};
use archy_eval::evaluate_variant;
use archy_generate::{ALGORITHM_VERSION, generate_variants};
use archy_report::{ReportContext, render_report, report_schema};

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
    };
    let artifacts = render_report(&spec, &generation.variants, &evaluations, &context).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&artifacts.json).unwrap();
    println!(
        "{}",
        serde_json::json!({"schemas": {"report": report_schema()}, "report": report})
    );
}
