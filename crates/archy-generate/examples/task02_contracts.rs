//! Source-derived Task02 schemas and real deterministic serializer output.
use archy_core::{InputFormat, evaluation_schema, parse_project, variant_schema};
use archy_eval::evaluate_variant;
use archy_generate::{GenerationResult, generate_variants};

fn main() {
    let spec = parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap();
    let result = generate_variants(&spec, 3, 42).unwrap();
    let report = evaluate_variant(&spec, &result.variants[0]).unwrap();
    println!(
        "{}",
        serde_json::json!({
            "schemas": {
                "variant": variant_schema(),
                "evaluation": evaluation_schema(),
                "generation": schemars::schema_for!(GenerationResult)
            },
            "generation": result,
            "evaluation": report
        })
    );
}
