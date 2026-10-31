use archy_core::{ErrorCode, InputFormat, parse_project, parse_project_current};

#[test]
fn downgraded_v2_source_reports_a_public_version_error() {
    let mut source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    source["schema_version"] = "0.1.0".into();
    let error = parse_project_current(&source.to_string(), InputFormat::Json).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidSchema);
    assert_eq!(error.field, "schema_version");
    assert!(error.message.contains("0.2.0"));
    assert!(!error.message.contains("ARCHY_PATH"));
}

#[test]
fn canonical_project_has_explicit_identity_bounds_units_and_typed_intentions() {
    let source = include_str!("../../../tests/fixtures/briefs/courtyard-v2.json");
    let current = parse_project_current(source, InputFormat::Json).unwrap();
    let value = serde_json::to_value(&current.canonical).unwrap();
    assert_eq!(value["schema_version"], "0.2.0");
    assert_eq!(value["normalization_version"], "si-rectangle-v2");
    assert_eq!(value["id"], "courtyard-demo");
    assert!(value.get("project_id").is_none());
    for constraint in value["constraints"].as_array().unwrap() {
        assert_eq!(constraint["unit"], "m");
        for field in ["id", "kind", "bound", "source", "authority"] {
            assert!(constraint.get(field).is_some());
        }
        assert!(constraint.get("value_m").is_none());
    }
    assert_eq!(
        value["preferences"][0],
        serde_json::json!({"kind":"massing_character","value":"courtyard"})
    );
    assert_eq!(current.canonical.numeric_view().unwrap(), current.numeric);
}

#[test]
fn fresh_legacy_import_preserves_declarative_wishes_without_losing_geometry() {
    let mut source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.json"
    ))
    .unwrap();
    source["preferences"] = serde_json::json!(["courtyard", "retain trees"]);
    let current = parse_project_current(&source.to_string(), InputFormat::Json).unwrap();
    assert_eq!(
        current.numeric,
        parse_project(&source.to_string(), InputFormat::Json).unwrap()
    );
    assert_eq!(current.canonical.preferences.len(), 2);
    for text in [
        "Courtyard",
        "warm brick facade",
        "low rise massing",
        "quiet shaded entry",
    ] {
        source["preferences"] = serde_json::json!([text]);
        let parsed = parse_project_current(&source.to_string(), InputFormat::Json).unwrap();
        assert_eq!(parsed.canonical.preferences[0].value, text);
        assert_eq!(parsed.numeric.preferences[0], text);
    }
    for text in [
        "execute: python3 -c print(1)",
        "guarantee 80% daylight autonomy",
        "<script>alert(1)</script>",
        "80% daylight autonomy",
    ] {
        source["preferences"] = serde_json::json!([text]);
        let error = parse_project_current(&source.to_string(), InputFormat::Json).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidSchema);
        assert_eq!(error.field, "preferences[0].value");
        assert!(!error.message.contains(text));
    }
}

#[test]
fn fresh_preferences_reject_executable_requests_and_preserve_architectural_prose() {
    let v2: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    let v1: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.json"
    ))
    .unwrap();
    for value in [
        "Please run rm -rf /",
        "PLEASE RUN RM -RF /",
        "Please 'run' \"rm -rf /\"",
        "Could you run rm -rf /?",
        "Please run make install",
        "Run a process outside the project",
        "Could you launch a tool?",
        "run python3 -c print(1)",
        "Run the shell script",
        "launch a bash script",
        "Delete the files",
        "curl https://example.invalid/",
        "A courtyard; please run rm -rf /",
    ] {
        for (mut source, legacy) in [(v2.clone(), false), (v1.clone(), true)] {
            source["preferences"] = if legacy {
                serde_json::json!([value])
            } else {
                serde_json::json!([{"kind":"architectural_intent","value":value}])
            };
            let error = parse_project_current(&source.to_string(), InputFormat::Json).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidSchema, "{value}");
            assert_eq!(error.field, "preferences[0].value", "{value}");
            assert!(!error.message.contains(value));
        }
    }
    for value in [
        "long run of south-facing glazing",
        "warm shell-like brick facade",
        "quiet courtyard with a shaded entry",
        "retain mature trees along the north edge",
        "open communal garden and flexible rooms",
        "a Python-inspired courtyard arrangement",
    ] {
        let mut source = v2.clone();
        source["preferences"] = serde_json::json!([{"kind":"architectural_intent","value":value}]);
        assert_eq!(
            parse_project_current(&source.to_string(), InputFormat::Json)
                .unwrap()
                .canonical
                .preferences[0]
                .value,
            value
        );
    }
    let mut canonical = parse_project_current(&v2.to_string(), InputFormat::Json)
        .unwrap()
        .canonical;
    canonical.preferences[0].value = "Please run rm -rf /".into();
    assert_eq!(
        canonical.numeric_view().unwrap_err().field,
        "preferences[0].value"
    );
}

#[test]
fn canonical_preferences_reject_extra_fields_duplicates_and_unknown_values() {
    let mut source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    source["preferences"][0]["target"] = 80.into();
    let extra = parse_project_current(&source.to_string(), InputFormat::Json).unwrap_err();
    assert_eq!(extra.code, ErrorCode::InvalidSchema);
    source["preferences"][0]
        .as_object_mut()
        .unwrap()
        .remove("target");
    source["preferences"][0]["value"] = "low_rise".into();
    assert!(parse_project_current(&source.to_string(), InputFormat::Json).is_ok());
    source["preferences"][0]["value"] = "x".repeat(900_000).into();
    let oversized = parse_project_current(&source.to_string(), InputFormat::Json).unwrap_err();
    assert_eq!(oversized.field, "preferences[0].value");
    assert!(oversized.message.len() < 200);
    source["preferences"][0]["value"] = "courtyard".into();
    let first = source["preferences"][0].clone();
    source["preferences"].as_array_mut().unwrap().push(first);
    let duplicate = parse_project_current(&source.to_string(), InputFormat::Json).unwrap_err();
    assert_eq!(duplicate.field, "preferences[2]");
    source["preferences"].as_array_mut().unwrap().pop();
    source["preferences"][1] = source["preferences"][0].clone();
    assert_eq!(
        parse_project_current(&source.to_string(), InputFormat::Json)
            .unwrap_err()
            .field,
        "preferences[1]"
    );
}

#[test]
fn passive_notes_preserve_other_prose_without_changing_numeric_design() {
    let mut source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    let baseline = parse_project_current(&source.to_string(), InputFormat::Json).unwrap();
    let note = "Prefer a warm brick facade; please retain the old orchard";
    source["notes"] = serde_json::json!([note]);
    let changed = parse_project_current(&source.to_string(), InputFormat::Json).unwrap();
    assert_eq!(changed.canonical.notes, vec![note]);
    assert_eq!(changed.numeric, baseline.numeric);
    source["notes"] = serde_json::json!(["x".repeat(513)]);
    assert_eq!(
        parse_project_current(&source.to_string(), InputFormat::Json)
            .unwrap_err()
            .field,
        "notes[0]"
    );
}

#[test]
fn canonical_bound_converts_declared_unit_and_keeps_source_authority() {
    let mut source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    source["constraints"][0]["bound"] = serde_json::json!(12.0 / 0.3048);
    source["constraints"][0]["unit"] = "ft".into();
    source["constraints"][0]["source"] = "architect_sketch".into();
    source["constraints"][0]["authority"] = "unverified_client_note".into();
    let current = parse_project_current(&source.to_string(), InputFormat::Json).unwrap();
    let constraint = &current.canonical.constraints[0];
    assert!((constraint.bound - 12.0).abs() < 1e-12);
    assert_eq!(serde_json::to_value(constraint.unit).unwrap(), "m");
    assert_eq!(constraint.source, "architect_sketch");
    assert_eq!(constraint.authority, "unverified_client_note");
}

#[test]
fn canonical_preferences_keep_order_and_yaml_json_agree() {
    let mut source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    source["preferences"] = serde_json::json!([
        {"kind":"architectural_intent","value":"warm brick facade"},
        {"kind":"massing_character","value":"low_rise"},
        {"kind":"landscape_intent","value":"retain mature trees"}
    ]);
    let json = parse_project_current(&source.to_string(), InputFormat::Json).unwrap();
    let yaml = serde_saphyr::to_string(&source).unwrap();
    let yaml = parse_project_current(&yaml, InputFormat::Yaml).unwrap();
    assert_eq!(json.canonical, yaml.canonical);
    assert_eq!(json.numeric, yaml.numeric);
    assert_eq!(
        json.canonical
            .preferences
            .iter()
            .map(|p| p.value.as_str())
            .collect::<Vec<_>>(),
        vec!["warm brick facade", "low_rise", "retain mature trees"]
    );
    assert_eq!(json.canonical.numeric_view().unwrap(), json.numeric);
}

#[test]
fn canonical_semantic_errors_name_source_fields() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    for (field, change) in [
        ("id", serde_json::json!({"id":"x".repeat(129)})),
        (
            "brief.floor_to_floor_m",
            serde_json::json!({"brief":{"storeys":3,"floor_to_floor_m":-1,"target_gfa_m2":900}}),
        ),
    ] {
        let mut source = fixture.clone();
        for (key, value) in change.as_object().unwrap() {
            source[key] = value.clone();
        }
        assert_eq!(
            parse_project_current(&source.to_string(), InputFormat::Json)
                .unwrap_err()
                .field,
            field
        );
    }
    let mut source = fixture.clone();
    source["constraints"][0]["bound"] = (-1).into();
    assert_eq!(
        parse_project_current(&source.to_string(), InputFormat::Json)
            .unwrap_err()
            .field,
        "constraints[0].bound"
    );
    let yaml = include_str!("../../../examples/courtyard-housing/project-v2.yaml").to_owned()
        + "\nschema_version: 0.1.0\n";
    let duplicate = parse_project_current(&yaml, InputFormat::Yaml).unwrap_err();
    assert_eq!(duplicate.field, "schema_version");
    assert!(!duplicate.message.contains("ARCHY_PATH"));
}

#[test]
fn public_canonical_values_must_revalidate_before_numeric_use() {
    let source = include_str!("../../../tests/fixtures/briefs/courtyard-v2.json");
    let mut spec = parse_project_current(source, InputFormat::Json)
        .unwrap()
        .canonical;
    spec.constraints[0].bound = -1.0;
    assert!(spec.numeric_view().is_err());
    spec.constraints[0].bound = 12.0;
    spec.tolerance_m = 0.01;
    assert!(spec.numeric_view().is_err());
    spec.tolerance_m = 1e-6;
    spec.site.polygon_xy_m[1][1] = 10.0;
    assert!(spec.numeric_view().is_err());
    spec.site.polygon_xy_m[1][1] = 0.0;
    spec.notes = vec!["x".repeat(513)];
    assert!(spec.numeric_view().is_err());
}

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
