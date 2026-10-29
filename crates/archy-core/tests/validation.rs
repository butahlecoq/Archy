use archy_core::{ErrorCode, InputFormat, parse_project};

const GOLDEN: &str = include_str!("../../../examples/courtyard-housing/project.yaml");
fn parse(s: &str) -> archy_core::NormalizedSpec {
    parse_project(s, InputFormat::Yaml).unwrap()
}

#[test]
fn golden_json_and_yaml_are_identical_si() {
    let a = parse(GOLDEN);
    let b = parse_project(
        include_str!("../../../tests/fixtures/briefs/courtyard.json"),
        InputFormat::Json,
    )
    .unwrap();
    assert_eq!(a, b);
    assert_eq!(
        a.site.polygon_xy_m,
        [[0., 0.], [20., 0.], [20., 35.], [0., 35.], [0., 0.]]
    );
    assert_eq!(a.brief.floor_to_floor_m, 3.2);
    assert_eq!(a.constraints[0].source, "user_brief");
    assert_eq!(a.constraints[0].authority, "assumption");
    assert!(!a.brief.target_gfa_hard);
}

#[test]
fn generic_feet_and_millimeters_normalize_to_si() {
    for (unit, scale) in [("ft", 0.3048), ("mm", 0.001), ("cm", 0.01)] {
        let mut p: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/briefs/courtyard.json"
        ))
        .unwrap();
        p["units"] = unit.into();
        for (section, si, generic) in [
            ("site", "width_m", "width"),
            ("site", "depth_m", "depth"),
            ("brief", "floor_to_floor_m", "floor_to_floor"),
            ("brief", "target_gfa_m2", "target_gfa"),
        ] {
            let value = p[section]
                .as_object_mut()
                .unwrap()
                .remove(si)
                .unwrap()
                .as_f64()
                .unwrap();
            p[section][generic] = (value
                / if si.ends_with("m2") {
                    scale * scale
                } else {
                    scale
                })
            .into();
        }
        for c in p["constraints"].as_array_mut().unwrap() {
            let v = c
                .as_object_mut()
                .unwrap()
                .remove("value_m")
                .unwrap()
                .as_f64()
                .unwrap();
            c["value"] = (v / scale).into();
            c["unit"] = unit.into();
        }
        let n = parse_project(&p.to_string(), InputFormat::Json).unwrap();
        assert!((n.site.polygon_xy_m[2][1] - 35.).abs() < 1e-6);
        assert!((n.brief.target_gfa_m2 - 900.).abs() < 1e-6);
    }
}

#[test]
fn translated_rectangle_coordinates_are_preserved() {
    let s = "schema_version: 0.1.0\nproject_id: translated\nunits: ft\nsite:\n  polygon_xy_m: [[10, -5], [30, -5], [30, 30], [10, 30], [10, -5]]\nbrief:\n  storeys: 3\n  floor_to_floor_m: 3.2\n  target_gfa_m2: 900\nconstraints: []\n";
    assert_eq!(parse(s).site.polygon_xy_m[0], [10., -5.]);
}

#[test]
fn explicit_si_and_preferences_survive_alternate_units() {
    let mut p: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.json"
    ))
    .unwrap();
    p["units"] = "ft".into();
    p["preferences"] = serde_json::json!(["courtyard", "retain trees"]);
    let n = parse_project(&p.to_string(), InputFormat::Json).unwrap();
    assert_eq!(n.brief.floor_to_floor_m, 3.2);
    assert_eq!(n.site.polygon_xy_m[2], [20., 35.]);
    assert_eq!(
        serde_json::to_value(n).unwrap()["preferences"],
        p["preferences"]
    );
}

#[test]
fn manifest_negative_fixtures_are_executed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/briefs");
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("negative-cases.json")).unwrap())
            .unwrap();
    for c in cases.as_array().unwrap() {
        let name = c["file"].as_str().unwrap();
        let text = std::fs::read_to_string(root.join(name)).unwrap();
        let format = if c["format"] == "yaml" {
            InputFormat::Yaml
        } else {
            InputFormat::Json
        };
        let e = parse_project(&text, format).expect_err(name);
        assert_eq!(serde_json::to_value(e.code).unwrap(), c["code"], "{name}");
        if let Some(field) = c["field"].as_str() {
            assert_eq!(e.field, field, "{name}: {e:?}");
        }
    }
}

#[test]
fn nested_parser_errors_have_paths() {
    for format in [InputFormat::Json, InputFormat::Yaml] {
        for (section, key, value, field) in [
            ("site", "guess", serde_json::json!(1), "site.guess"),
            (
                "brief",
                "storeys",
                serde_json::json!("bad"),
                "brief.storeys",
            ),
            (
                "constraints",
                "source",
                serde_json::json!(3),
                "constraints[0].source",
            ),
        ] {
            let mut p: serde_json::Value = serde_json::from_str(include_str!(
                "../../../tests/fixtures/briefs/courtyard.json"
            ))
            .unwrap();
            if section == "constraints" {
                p[section][0][key] = value;
            } else {
                p[section][key] = value;
            }
            let e = parse_project(&p.to_string(), format).unwrap_err();
            assert_eq!(e.field, field, "{e:?}");
        }
    }
}

#[test]
fn negative_fixtures_have_stable_errors_and_paths() {
    for (text, code, field) in [
        (
            GOLDEN.replace("width_m: 20", "width_m: 0"),
            ErrorCode::InvalidSchema,
            "site.width",
        ),
        (
            GOLDEN.replace("storeys: 3", "storeys: 0"),
            ErrorCode::InvalidSchema,
            "brief.storeys",
        ),
        (
            GOLDEN.replace("0.1.0", "9.0.0"),
            ErrorCode::InvalidSchema,
            "schema_version",
        ),
        (
            GOLDEN.replace("value_m: 3", "value_m: 10"),
            ErrorCode::InvalidSchema,
            "constraints",
        ),
        (
            GOLDEN.replace("floor_to_floor_m: 3.2", "floor_to_floor_m: .inf"),
            ErrorCode::InvalidSchema,
            "brief.floor_to_floor",
        ),
    ] {
        let e = parse_project(&text, InputFormat::Yaml).unwrap_err();
        assert_eq!(e.code, code);
        assert_eq!(e.field, field);
        assert!(!e.retriable);
    }
}

#[test]
fn duplicates_unknown_fields_and_aliases_fail() {
    for s in [
        format!("{GOLDEN}\nproject_id: duplicate"),
        format!("{GOLDEN}\nunknown: true"),
        GOLDEN.replace("width_m: 20", "width_m: &a 20"),
        GOLDEN.replace("width_m: 20", "width_m: 20\n  width: 20"),
    ] {
        assert_eq!(
            parse_project(&s, InputFormat::Yaml).unwrap_err().code,
            ErrorCode::InvalidSchema
        );
    }
    assert!(parse_project(r#"{"project_id":"a","project_id":"b"}"#, InputFormat::Json).is_err());
}

#[test]
fn infeasible_height_is_a_later_candidate_check() {
    assert!(
        parse_project(
            &GOLDEN.replace("value_m: 12", "value_m: 8"),
            InputFormat::Yaml
        )
        .is_ok()
    );
}

#[test]
fn all_error_codes_are_stable() {
    for (code, name) in [
        (ErrorCode::InvalidSchema, "INVALID_SCHEMA"),
        (ErrorCode::InvalidGeometry, "INVALID_GEOMETRY"),
        (ErrorCode::ConstraintFail, "CONSTRAINT_FAIL"),
        (ErrorCode::BackendUnavailable, "BACKEND_UNAVAILABLE"),
        (ErrorCode::BackendTimeout, "BACKEND_TIMEOUT"),
        (ErrorCode::ArtifactInvalid, "ARTIFACT_INVALID"),
        (ErrorCode::RevisionConflict, "REVISION_CONFLICT"),
        (ErrorCode::PermissionDenied, "PERMISSION_DENIED"),
        (ErrorCode::InternalError, "INTERNAL_ERROR"),
    ] {
        assert_eq!(serde_json::to_value(code).unwrap(), name);
    }
}

#[test]
fn required_fields_nested_missing_and_parser_limits() {
    let baseline: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.json"
    ))
    .unwrap();
    for key in [
        "project_id",
        "schema_version",
        "site",
        "brief",
        "units",
        "constraints",
    ] {
        let mut p = baseline.clone();
        p.as_object_mut().unwrap().remove(key);
        for f in [InputFormat::Json, InputFormat::Yaml] {
            assert_eq!(parse_project(&p.to_string(), f).unwrap_err().field, key);
        }
    }
    let mut p = baseline.clone();
    p["brief"].as_object_mut().unwrap().remove("storeys");
    for f in [InputFormat::Json, InputFormat::Yaml] {
        assert_eq!(
            parse_project(&p.to_string(), f).unwrap_err().field,
            "brief.storeys"
        );
    }
    for text in [
        "a: *missing".to_string(),
        "a: &a 1\nb: *a".to_string(),
        " ".repeat(archy_core::validation::MAX_INPUT_BYTES + 1),
    ] {
        assert_eq!(
            parse_project(&text, InputFormat::Yaml).unwrap_err().code,
            ErrorCode::InvalidSchema
        );
    }
    assert!(
        parse_project(
            &GOLDEN.replace("floor_to_floor_m: 3.2", "floor_to_floor_m: 1e999"),
            InputFormat::Yaml
        )
        .is_err()
    );
}

#[test]
fn mixed_dimensions_and_area_tolerance_use_correct_units() {
    let mut p: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.json"
    ))
    .unwrap();
    p["units"] = "ft".into();
    p["site"].as_object_mut().unwrap().remove("width_m");
    p["site"]["width"] = (20.0 / 0.3048).into();
    assert_eq!(
        parse_project(&p.to_string(), InputFormat::Json)
            .unwrap()
            .brief
            .floor_to_floor_m,
        3.2
    );
    p["brief"]["target_gfa_hard"] = true.into();
    p["brief"]["target_gfa_m2"] = (14.0 * 29.0 * 3.0 + 0.5e-6).into();
    assert!(parse_project(&p.to_string(), InputFormat::Json).is_ok());
    p["brief"]["target_gfa_m2"] = (14.0 * 29.0 * 3.0 + 2e-6).into();
    assert!(parse_project(&p.to_string(), InputFormat::Json).is_err());
}
