use archy_core::*;
use archy_eval::evaluate_variant;
use archy_generate::generate_variants;

fn spec() -> NormalizedSpec {
    parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap()
}

#[test]
fn golden_is_reproducible_distinct_and_geometrically_feasible() {
    let s = spec();
    let a = generate_variants(&s, 3, 42).unwrap();
    let b = generate_variants(&s, 3, 42).unwrap();
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.variants.len(), 3);
    let mut footprints = std::collections::BTreeSet::new();
    for (v, w) in a.variants.iter().zip(&b.variants) {
        assert!(footprints.insert(serde_json::to_string(&v.footprint_xy_m).unwrap()));
        let report = evaluate_variant(&s, v).unwrap();
        assert!(report.required_checks_passed);
        assert_eq!(report, evaluate_variant(&s, w).unwrap());
        assert_eq!(
            *v,
            serde_json::from_str::<Variant>(&serde_json::to_string(v).unwrap()).unwrap()
        );
    }
    assert_ne!(a.variants, generate_variants(&s, 3, 43).unwrap().variants);
}

#[test]
fn restrictions_exhaustion_invalid_counts_and_failing_candidates() {
    let mut s = spec();
    for shape in [Shape::Rectangle, Shape::LShape] {
        s.generation.shapes = vec![shape];
        assert!(
            generate_variants(&s, 12, 42)
                .unwrap()
                .variants
                .iter()
                .all(|v| v.shape == shape)
        );
    }
    for count in [0, 1001] {
        assert!(generate_variants(&s, count, 42).is_err());
    }
    s.brief.floor_to_floor_m = 10.0;
    let result = generate_variants(&s, 3, 42).unwrap();
    assert_eq!(result.variants.len(), 3);
    assert!(
        result
            .variants
            .iter()
            .all(|v| !evaluate_variant(&s, v).unwrap().required_checks_passed)
    );
    s.site.polygon_xy_m = [[0., 0.], [6., 0.], [6., 35.], [0., 35.], [0., 0.]];
    let result = generate_variants(&s, 3, 42).unwrap();
    assert!(result.variants.is_empty());
    assert!(!result.exhaustion_explanations.is_empty());
    assert!(result.attempts <= 300);
    s.site.polygon_xy_m = [
        [0., 0.],
        [6.00000001, 0.],
        [6.00000001, 35.],
        [0., 35.],
        [0., 0.],
    ];
    assert!(generate_variants(&s, 3, 42).unwrap().variants.is_empty());
}

#[test]
fn changed_geometry_changes_canonical_id() {
    let project = spec();
    let first = Variant::new(
        project.project_id.clone(),
        42,
        "analytic-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [3., 3.],
            width_m: 10.,
            depth_m: 20.,
        },
        3,
        3.2,
    )
    .unwrap();
    let changed = Variant::new(
        project.project_id,
        42,
        "analytic-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [3., 3.],
            width_m: 11.,
            depth_m: 20.,
        },
        3,
        3.2,
    )
    .unwrap();
    assert_ne!(first.id, changed.id);
    assert_ne!(first.footprint_xy_m, changed.footprint_xy_m);
}

#[test]
fn translated_generation_stays_within_translated_bounds() {
    let original = spec();
    let mut translated = original.clone();
    for point in &mut translated.site.polygon_xy_m {
        point[0] += 10.;
        point[1] -= 5.;
    }
    let baseline = generate_variants(&original, 3, 42).unwrap();
    let moved = generate_variants(&translated, 3, 42).unwrap();
    assert_eq!(baseline.variants.len(), 3);
    assert_eq!(moved.variants.len(), 3);
    for (reference, shifted) in baseline.variants.iter().zip(&moved.variants) {
        assert_ne!(reference.id, shifted.id);
        assert!(
            evaluate_variant(&translated, shifted)
                .unwrap()
                .required_checks_passed
        );
        for (before, after) in reference.footprint_xy_m.iter().zip(&shifted.footprint_xy_m) {
            assert!((after[0] - before[0] - 10.).abs() < 1e-12);
            assert!((after[1] - before[1] + 5.).abs() < 1e-12);
            assert!(after[0] >= 13. - 1e-6 && after[0] <= 27. + 1e-6);
            assert!(after[1] >= -2. - 1e-6 && after[1] <= 27. + 1e-6);
        }
    }
}

#[test]
fn legal_narrow_l_site_explains_unrepresentable_samples() {
    let mut input: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.json"
    ))
    .unwrap();
    input["site"]["width_m"] = serde_json::json!(0.0000021);
    input["constraints"] = serde_json::json!([]);
    input["generation"]["shapes"] = serde_json::json!(["l_shape"]);
    let project = parse_project(&input.to_string(), InputFormat::Json).unwrap();
    let result = generate_variants(&project, 1, 42).unwrap();
    assert_eq!(
        serde_json::to_vec(&result).unwrap(),
        serde_json::to_vec(&generate_variants(&project, 1, 42).unwrap()).unwrap()
    );
    assert_eq!(result.attempts, 100);
    assert!(result.variants.is_empty());
    let rejected: u32 = result
        .attempt_diagnostics
        .unwrap()
        .rejections
        .iter()
        .map(|entry| entry.count)
        .sum();
    assert_eq!(rejected, result.attempts);
}

#[test]
fn public_spec_overflow_explains_variant_construction_failures() {
    let mut project = spec();
    project.site.polygon_xy_m = [[0., 0.], [1e153, 0.], [1e153, 1e153], [0., 1e153], [0., 0.]];
    project.constraints.clear();
    project.generation.shapes = vec![Shape::Rectangle];
    project.brief.storeys = 10_000;
    let result = generate_variants(&project, 1, 42).unwrap();
    assert_eq!(result.attempts, 100);
    assert!(result.variants.is_empty());
    assert!(
        result
            .attempt_diagnostics
            .unwrap()
            .rejections
            .iter()
            .any(|entry| {
                entry.phase == archy_generate::RejectionPhase::Variant
                    && entry.message.contains("overflow")
            })
    );
}
