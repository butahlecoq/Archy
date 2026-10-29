use archy_core::*;
use archy_eval::{
    RigidTransform2D, compare_rigid_geometry, evaluate_variant, geometry_parity_budget,
};
fn spec() -> NormalizedSpec {
    parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap()
}
fn rectangle(s: &NormalizedSpec, origin: [f64; 2]) -> Variant {
    Variant::new(
        s.project_id.clone(),
        42,
        "analytic-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: origin,
            width_m: 10.,
            depth_m: 20.,
        },
        3,
        3.2,
    )
    .unwrap()
}
fn metric(r: &EvaluationReport, name: &str) -> f64 {
    r.metrics.iter().find(|m| m.name == name).unwrap().value
}
#[test]
fn rectangle_and_l_have_independent_analytic_measurements_and_translation() {
    let mut s = spec();
    let v = rectangle(&s, [3., 3.]);
    let r = evaluate_variant(&s, &v).unwrap();
    for (name, value) in [
        ("footprint_area", 200.),
        ("gross_floor_area", 600.),
        ("height", 9.6),
        ("clearance_west", 3.),
        ("clearance_east", 7.),
        ("clearance_south", 3.),
        ("clearance_north", 12.),
    ] {
        assert!((metric(&r, name) - value).abs() < 1e-10);
    }
    let l = Variant::new(
        s.project_id.clone(),
        42,
        "analytic-v1".into(),
        ShapeParameters::LShape {
            origin_xy_m: [3., 3.],
            width_m: 10.,
            depth_m: 10.,
            notch_width_m: 5.,
            notch_depth_m: 5.,
        },
        3,
        3.2,
    )
    .unwrap();
    assert_eq!(
        metric(&evaluate_variant(&s, &l).unwrap(), "footprint_area"),
        75.
    );
    assert_eq!(
        metric(&evaluate_variant(&s, &l).unwrap(), "gross_floor_area"),
        225.
    );
    for p in &mut s.site.polygon_xy_m {
        p[0] += 10.;
        p[1] -= 5.;
    }
    let translated = evaluate_variant(&s, &rectangle(&s, [13., -2.])).unwrap();
    for name in [
        "footprint_area",
        "gross_floor_area",
        "height",
        "clearance_west",
        "clearance_east",
        "clearance_south",
        "clearance_north",
    ] {
        assert!((metric(&r, name) - metric(&translated, name)).abs() < 1e-10);
    }
}
#[test]
fn canonical_snapshot_rejects_all_tampering_and_malformed_geometry() {
    let s = spec();
    let v = rectangle(&s, [3., 3.]);
    for field in ["id", "project_id", "generation_algorithm"] {
        let mut value = serde_json::to_value(&v).unwrap();
        value[field] = serde_json::json!("tampered");
        assert!(serde_json::from_value::<Variant>(value).is_err());
    }
    let mut value = serde_json::to_value(&v).unwrap();
    value["parameters"]["width_m"] = serde_json::json!(11.);
    assert!(serde_json::from_value::<Variant>(value).is_err());
    let mut bads = vec![];
    let mut b = v.clone();
    b.footprint_xy_m.reverse();
    bads.push(b);
    let mut b = v.clone();
    b.footprint_xy_m.insert(1, b.footprint_xy_m[0]);
    bads.push(b);
    let mut b = v.clone();
    b.footprint_xy_m.swap(1, 2);
    bads.push(b);
    let mut b = v.clone();
    b.footprint_xy_m.pop();
    bads.push(b);
    let mut b = v.clone();
    b.footprint_xy_m[1][0] = f64::NAN;
    bads.push(b);
    let mut b = v.clone();
    b.footprint_xy_m.fill([3., 3.]);
    bads.push(b);
    for b in bads {
        assert_eq!(
            evaluate_variant(&s, &b).unwrap_err().code,
            ErrorCode::InvalidGeometry
        );
    }
    let mut other = s.clone();
    other.project_id = "other".into();
    assert!(evaluate_variant(&other, &v).is_err());
    assert!(
        Variant::new(
            s.project_id,
            42,
            "analytic-v1".into(),
            ShapeParameters::Rectangle {
                origin_xy_m: [0., 0.],
                width_m: f64::MAX,
                depth_m: f64::MAX
            },
            3,
            3.2
        )
        .is_err()
    );
}
#[test]
fn tolerance_hard_target_outside_and_authority_are_explicit() {
    let mut s = spec();
    for (delta, pass) in [(0., true), (-0.5e-6, true), (-2e-6, false)] {
        let r = evaluate_variant(&s, &rectangle(&s, [3. + delta, 3.])).unwrap();
        assert_eq!(r.required_checks_passed, pass);
    }
    for (delta, pass) in [(0., true), (0.5e-6, true), (2e-6, false)] {
        s.constraints[0].value_m = 9.6 - delta;
        assert_eq!(
            evaluate_variant(&s, &rectangle(&s, [3., 3.]))
                .unwrap()
                .required_checks_passed,
            pass
        );
    }
    s = spec();
    s.brief.target_gfa_hard = true;
    for (delta, pass) in [(0., true), (0.5e-6, true), (2e-6, false)] {
        s.brief.target_gfa_m2 = 600. + delta;
        assert_eq!(
            evaluate_variant(&s, &rectangle(&s, [3., 3.]))
                .unwrap()
                .required_checks_passed,
            pass
        );
    }
    s = spec();
    let r = evaluate_variant(&s, &rectangle(&s, [-2., 3.])).unwrap();
    assert_eq!(metric(&r, "clearance_west"), -2.);
    assert!(!r.required_checks_passed);
    s.constraints[0].authority = "CERTIFIED LEGAL LAW".into();
    let r = evaluate_variant(&s, &rectangle(&s, [3., 3.])).unwrap();
    assert!(r.professional_limits.contains("not legal"));
    let mut checks = r.checks;
    checks.push(Check {
        constraint_id: "uncomputed".into(),
        result: CheckResult::Unknown,
        measured: None,
        limit: None,
        explanation: "Uncomputed".into(),
        required: true,
    });
    assert!(!required_checks_pass(&checks));
}

#[test]
fn serialized_report_rejects_false_required_pass() {
    let spec = spec();
    let report = evaluate_variant(&spec, &rectangle(&spec, [3., 3.])).unwrap();
    let mut serialized = serde_json::to_value(report).unwrap();
    serialized["checks"][0]["result"] = serde_json::json!("UNKNOWN");
    assert!(serde_json::from_value::<EvaluationReport>(serialized).is_err());
}

#[test]
fn public_normalized_spec_rejects_ambiguous_provenance() {
    let original = spec();
    for mutation in 0..4 {
        let mut candidate: NormalizedSpec =
            serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
        match mutation {
            0 => candidate.project_id = "bad id".into(),
            1 => candidate.constraints[0].id = candidate.constraints[1].id.clone(),
            2 => candidate.constraints[0].source = " ".into(),
            _ => candidate.constraints[0].authority = " ".into(),
        }
        assert!(evaluate_variant(&candidate, &rectangle(&candidate, [3., 3.])).is_err());
    }
}

#[test]
fn signed_zero_has_one_canonical_serialized_snapshot() {
    let project_id = spec().project_id;
    let make = |x| {
        Variant::new(
            project_id.clone(),
            42,
            "analytic-v1".into(),
            ShapeParameters::Rectangle {
                origin_xy_m: [x, 0.],
                width_m: 10.,
                depth_m: 20.,
            },
            3,
            3.2,
        )
        .unwrap()
    };
    let negative = make(-0.);
    let positive = make(0.);
    assert_eq!(negative.id, positive.id);
    assert_eq!(
        serde_json::to_vec(&negative).unwrap(),
        serde_json::to_vec(&positive).unwrap()
    );
}

#[test]
fn reserved_check_ids_fail_for_parsed_and_public_specs() {
    let input: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.json"
    ))
    .unwrap();
    for reserved in ["site_containment", "target_gfa"] {
        let mut brief = input.clone();
        brief["constraints"][0]["id"] = serde_json::json!(reserved);
        assert!(parse_project(&brief.to_string(), InputFormat::Json).is_err());
        let mut normalized = spec();
        normalized.constraints[0].id = reserved.into();
        assert!(evaluate_variant(&normalized, &rectangle(&normalized, [3., 3.])).is_err());
    }
}

#[test]
fn negative_zero_in_stored_footprint_is_rejected() {
    let variant = Variant::new(
        spec().project_id,
        42,
        "analytic-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [0., 0.],
            width_m: 10.,
            depth_m: 20.,
        },
        3,
        3.2,
    )
    .unwrap();
    let mut stored = serde_json::to_value(&variant).unwrap();
    stored["footprint_xy_m"][0][0] = serde_json::json!(-0.0);
    assert!(stored.to_string().contains("-0.0"));
    assert!(serde_json::from_value::<Variant>(stored).is_err());
    let mut in_memory = variant;
    in_memory.footprint_xy_m[0][0] = -0.0;
    assert_eq!(
        in_memory.validate().unwrap_err().code,
        ErrorCode::InvalidGeometry
    );
}

#[test]
fn parity_budget_propagates_length_error_by_size_and_storeys() {
    let project = spec();
    let small = rectangle(&project, [3., 3.]);
    let small_budget = geometry_parity_budget(&small).unwrap();
    assert!((small_budget.footprint_area_m2 - (60.0e-6 + 4.0e-12)).abs() < 1e-15);
    assert!(
        (small_budget.gross_floor_area_m2 - 3.0 * small_budget.footprint_area_m2).abs() < 1e-15
    );
    let large = Variant::new(
        project.project_id.clone(),
        42,
        "analytic-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [3., 3.],
            width_m: 100.,
            depth_m: 200.,
        },
        5,
        3.2,
    )
    .unwrap();
    let large_budget = geometry_parity_budget(&large).unwrap();
    assert!((large_budget.footprint_area_m2 - (600.0e-6 + 4.0e-12)).abs() < 1e-15);
    assert!(
        (large_budget.gross_floor_area_m2 - 5.0 * large_budget.footprint_area_m2).abs() < 1e-15
    );
    assert_eq!(small_budget.policy_version, "polygon-coordinate-parity-v1");
    assert_eq!(
        small_budget.rigid_transform_policy_version,
        "rigid-xy-ordered-v1"
    );
}

#[test]
fn rigid_transform_parity_applies_coordinate_and_propagated_area_bounds() {
    let project = spec();
    let variant = rectangle(&project, [3., 3.]);
    let transform = RigidTransform2D {
        rotation_rad: 0.,
        translation_xy_m: [10., -5.],
    };
    let mut translated: Vec<[f64; 2]> = variant
        .footprint_xy_m
        .iter()
        .map(|point| [point[0] + 10., point[1] - 5.])
        .collect();
    translated[1][0] += 0.5e-6;
    translated[2][0] += 0.5e-6;
    assert!(
        compare_rigid_geometry(&variant, &translated, transform)
            .unwrap()
            .passed
    );
    let mut hard_target = project.clone();
    hard_target.brief.target_gfa_hard = true;
    hard_target.brief.target_gfa_m2 = 600. + 2e-6;
    assert!(
        !evaluate_variant(&hard_target, &variant)
            .unwrap()
            .required_checks_passed
    );
    translated[1][0] += 2.0e-6;
    translated[2][0] += 2.0e-6;
    assert!(
        !compare_rigid_geometry(&variant, &translated, transform)
            .unwrap()
            .passed
    );
}

#[test]
fn rotated_parity_budget_uses_transformed_axis_lengths() {
    let project = spec();
    let variant = rectangle(&project, [3., 3.]);
    let rotation_rad = std::f64::consts::FRAC_PI_4;
    let (sine, cosine) = rotation_rad.sin_cos();
    let observed: Vec<[f64; 2]> = variant
        .footprint_xy_m
        .iter()
        .map(|point| {
            [
                cosine * point[0] - sine * point[1],
                sine * point[0] + cosine * point[1],
            ]
        })
        .collect();
    let result = compare_rigid_geometry(
        &variant,
        &observed,
        RigidTransform2D {
            rotation_rad,
            translation_xy_m: [0., 0.],
        },
    )
    .unwrap();
    assert!(result.passed);
    assert!(
        result.budget.footprint_area_m2
            > geometry_parity_budget(&variant).unwrap().footprint_area_m2 * 1.4
    );
}

#[test]
fn named_analytic_fixture_matches_rectangle_and_l_measurements() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/analytic/courtyard-shapes.json"
    ))
    .unwrap();
    let project = spec();
    for case in fixture["cases"].as_array().unwrap() {
        let origin: [f64; 2] = serde_json::from_value(case["origin_xy_m"].clone()).unwrap();
        let width = case["width_m"].as_f64().unwrap();
        let depth = case["depth_m"].as_f64().unwrap();
        let parameters = match case["shape"].as_str().unwrap() {
            "rectangle" => ShapeParameters::Rectangle {
                origin_xy_m: origin,
                width_m: width,
                depth_m: depth,
            },
            "l_shape" => ShapeParameters::LShape {
                origin_xy_m: origin,
                width_m: width,
                depth_m: depth,
                notch_width_m: case["notch_width_m"].as_f64().unwrap(),
                notch_depth_m: case["notch_depth_m"].as_f64().unwrap(),
            },
            _ => panic!("unknown fixture shape"),
        };
        let variant = Variant::new(
            project.project_id.clone(),
            42,
            "analytic-v1".into(),
            parameters,
            fixture["storeys"].as_u64().unwrap() as u32,
            fixture["floor_to_floor_m"].as_f64().unwrap(),
        )
        .unwrap();
        let expected_footprint: Vec<[f64; 2]> =
            serde_json::from_value(case["footprint_xy_m"].clone()).unwrap();
        assert_eq!(variant.footprint_xy_m, expected_footprint);
        let report = evaluate_variant(&project, &variant).unwrap();
        for (name, expected) in [
            (
                "footprint_area",
                case["footprint_area_m2"].as_f64().unwrap(),
            ),
            (
                "gross_floor_area",
                case["gross_floor_area_m2"].as_f64().unwrap(),
            ),
            ("height", case["height_m"].as_f64().unwrap()),
        ] {
            assert!((metric(&report, name) - expected).abs() < 1e-10);
        }
        for (index, name) in [
            "clearance_west",
            "clearance_east",
            "clearance_south",
            "clearance_north",
        ]
        .iter()
        .enumerate()
        {
            assert!(
                (metric(&report, name) - case["clearances_m"][index].as_f64().unwrap()).abs()
                    < 1e-10
            );
        }
    }
}
