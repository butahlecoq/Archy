use archy_core::{InputFormat, ShapeParameters, Variant, parse_project};
use archy_eval::evaluate_variant;
use archy_report::{ComparisonSource, render_comparison};

#[test]
fn ranking_ties_failures_and_text_are_stable() {
    let mut spec = parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap();
    spec.preferences.push("<script>\n| fake |".into());
    let make = |x, width| {
        Variant::new(
            spec.project_id.clone(),
            42,
            archy_generate::ALGORITHM_VERSION.into(),
            ShapeParameters::Rectangle {
                origin_xy_m: [x, 3.0],
                width_m: width,
                depth_m: 20.0,
            },
            3,
            3.2,
        )
        .unwrap()
    };
    let variants = vec![make(3.0, 10.0), make(4.0, 10.0), make(0.0, 5.0)];
    let evaluations = variants
        .iter()
        .map(|v| evaluate_variant(&spec, v).unwrap())
        .collect::<Vec<_>>();
    let source = ComparisonSource {
        run_id: "run-1".into(),
        manifest_sha256: "a".repeat(64),
        input_sha256: "b".repeat(64),
        normalized_sha256: "c".repeat(64),
        variant_sha256: variants.iter().map(|_| "d".repeat(64)).collect(),
        evaluation_sha256: variants.iter().map(|_| "e".repeat(64)).collect(),
    };
    let first = render_comparison(
        &spec,
        &variants,
        &evaluations,
        &source,
        1,
        Some("f".repeat(64)),
    )
    .unwrap();
    let mut reversed = variants.clone();
    reversed.reverse();
    let mut reversed_evals = evaluations.clone();
    reversed_evals.reverse();
    let mut reversed_source = source.clone();
    reversed_source.variant_sha256.reverse();
    reversed_source.evaluation_sha256.reverse();
    let second = render_comparison(
        &spec,
        &reversed,
        &reversed_evals,
        &reversed_source,
        1,
        Some("f".repeat(64)),
    )
    .unwrap();
    assert_eq!(first, second);
    let doc: serde_json::Value = serde_json::from_slice(&first.json).unwrap();
    assert_eq!(doc["candidates"].as_array().unwrap().len(), 3);
    assert_eq!(doc["candidates"][0]["objective_rank"], 1);
    assert_eq!(doc["candidates"][1]["objective_rank"], 1);
    assert_eq!(doc["candidates"][2]["objective_rank"], 2);
    assert_eq!(
        doc["candidates"][2]["evaluation"]["required_checks_passed"],
        false
    );
    assert_eq!(doc["uncomputed"], serde_json::json!(["daylight", "cost"]));
    assert!(doc["constraints"][0]["source"].is_string());
    assert_eq!(doc["constraints"][0]["unit"], "m");
    assert_eq!(
        doc["constraints"][0]["authority_status"],
        "unverified declaration"
    );
    assert_eq!(doc["candidates"][0]["objective"]["unit"], "m2");
    assert_eq!(
        doc["candidates"][0]["objective"]["method_version"],
        archy_report::COMPARISON_POLICY_VERSION
    );
    assert_eq!(
        doc["candidates"][0]["objective_source_metric"],
        "target_gfa_delta"
    );
    assert!(doc["candidates"][0]["evaluation_sha256"].is_string());
    let md = String::from_utf8(first.markdown).unwrap();
    assert!(!md.contains("<script>"));
    assert!(md.contains("&lt;script&gt;"));
}

#[test]
fn missing_objective_is_uncomputed_and_ranked_last() {
    let spec = parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap();
    let variant = Variant::new(
        spec.project_id.clone(),
        42,
        archy_generate::ALGORITHM_VERSION.into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [3.0, 3.0],
            width_m: 10.0,
            depth_m: 20.0,
        },
        3,
        3.2,
    )
    .unwrap();
    let mut evaluation = evaluate_variant(&spec, &variant).unwrap();
    evaluation.metrics.retain(|m| m.name != "target_gfa_delta");
    let source = ComparisonSource {
        run_id: "run-1".into(),
        manifest_sha256: "a".repeat(64),
        input_sha256: "b".repeat(64),
        normalized_sha256: "c".repeat(64),
        variant_sha256: vec!["d".repeat(64)],
        evaluation_sha256: vec!["e".repeat(64)],
    };
    let artifact = render_comparison(&spec, &[variant], &[evaluation], &source, 1, None).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&artifact.json).unwrap();
    assert!(value["candidates"][0]["objective"].is_null());
    assert!(value["candidates"][0]["objective_rank"].is_null());
}

#[test]
fn checked_in_schema_matches_derived_document() {
    let derived = serde_json::to_value(archy_report::comparison_schema()).unwrap();
    let checked_in: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/comparison.schema.json")).unwrap();
    assert_eq!(checked_in, derived);
}

#[test]
fn markdown_makes_directional_and_control_characters_visible_but_json_preserves_them() {
    let mut spec = parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap();
    let text = "café العربية 👩\u{200d}💻 \u{061c}\u{200e}\u{200f}\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}\u{2066}\u{2067}\u{2068}\u{2069}\u{206a}\u{206f}\u{2028}\u{2029}\u{001b}\t\n [link](url)|<tag>";
    spec.constraints[0].authority = text.into();
    spec.constraints[0].source = text.into();
    spec.preferences.push(text.into());
    let variant = Variant::new(
        spec.project_id.clone(),
        42,
        archy_generate::ALGORITHM_VERSION.into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [3.0, 3.0],
            width_m: 10.0,
            depth_m: 20.0,
        },
        3,
        3.2,
    )
    .unwrap();
    let evaluation = evaluate_variant(&spec, &variant).unwrap();
    let source = ComparisonSource {
        run_id: "run-1".into(),
        manifest_sha256: text.into(),
        input_sha256: "b".repeat(64),
        normalized_sha256: "c".repeat(64),
        variant_sha256: vec![text.into()],
        evaluation_sha256: vec![text.into()],
    };
    let result = render_comparison(
        &spec,
        &[variant],
        &[evaluation],
        &source,
        1,
        Some(text.into()),
    )
    .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&result.json).unwrap();
    assert_eq!(json["constraints"][0]["authority"], text);
    assert_eq!(json["constraints"][0]["source"], text);
    assert_eq!(json["subjective_preferences"][0], text);
    assert_eq!(json["source_manifest_sha256"], text);
    let md = String::from_utf8(result.markdown).unwrap();
    for code in [
        0x061c, 0x200e, 0x200f, 0x202a, 0x202b, 0x202c, 0x202d, 0x202e, 0x2066, 0x2067, 0x2068,
        0x2069, 0x206a, 0x206f, 0x2028, 0x2029, 0x001b, 0x0009, 0x000a,
    ] {
        if code != 0x000a {
            assert!(
                !md.contains(char::from_u32(code).unwrap()),
                "raw U+{code:04X}"
            );
        }
        assert!(
            md.contains(&format!("\\[U+{code:04X}\\]")),
            "missing visible U+{code:04X}"
        );
    }
    assert!(md.contains("café العربية 👩\u{200d}💻"));
    assert!(md.contains("\\[link\\]\\(url\\)\\|&lt;tag&gt;"));
}
