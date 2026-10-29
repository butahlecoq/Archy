use archy_core::*;
use archy_generate::generate_variants;
use archy_eval::evaluate_variant;

fn spec() -> NormalizedSpec {
    parse_project(include_str!("../../../tests/fixtures/briefs/courtyard.json"), InputFormat::Json).unwrap()
}

#[test]
fn golden_is_reproducible_distinct_and_geometrically_feasible() {
    let s = spec();
    let a = generate_variants(&s, 3, 42).unwrap();
    let b = generate_variants(&s, 3, 42).unwrap();
    assert_eq!(serde_json::to_vec(&a).unwrap(), serde_json::to_vec(&b).unwrap());
    assert_eq!(a.variants.len(), 3);
    let mut footprints = std::collections::BTreeSet::new();
    for (v, w) in a.variants.iter().zip(&b.variants) {
        assert!(footprints.insert(serde_json::to_string(&v.footprint_xy_m).unwrap()));
        let report = evaluate_variant(&s, v).unwrap();
        assert!(report.required_checks_passed);
        assert_eq!(report, evaluate_variant(&s, w).unwrap());
        assert_eq!(*v, serde_json::from_str::<Variant>(&serde_json::to_string(v).unwrap()).unwrap());
    }
    assert_ne!(a.variants, generate_variants(&s, 3, 43).unwrap().variants);
}

#[test]
fn restrictions_exhaustion_invalid_counts_and_failing_candidates() {
    let mut s = spec();
    for shape in [Shape::Rectangle, Shape::LShape] {
        s.generation.shapes = vec![shape];
        assert!(generate_variants(&s, 12, 42).unwrap().variants.iter().all(|v| v.shape == shape));
    }
    for count in [0, 1001] { assert!(generate_variants(&s, count, 42).is_err()); }
    s.brief.floor_to_floor_m = 10.0;
    let result = generate_variants(&s, 3, 42).unwrap();
    assert_eq!(result.variants.len(), 3);
    assert!(result.variants.iter().all(|v| !evaluate_variant(&s, v).unwrap().required_checks_passed));
    s.site.polygon_xy_m = [[0.,0.],[6.,0.],[6.,35.],[0.,35.],[0.,0.]];
    let result = generate_variants(&s, 3, 42).unwrap();
    assert!(result.variants.is_empty());
    assert!(!result.exhaustion_explanations.is_empty());
    assert!(result.attempts <= 300);
    s.site.polygon_xy_m = [[0.,0.],[6.00000001,0.],[6.00000001,35.],[0.,35.],[0.,0.]];
    assert!(generate_variants(&s, 3, 42).unwrap().variants.is_empty());
}
