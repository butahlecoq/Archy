use archy_adapters::{
    BlenderRequest, IfcRequest, SourceArtifact, decode_blender_response, decode_ifc_response,
};
use archy_core::{NormalizedSpec, ShapeParameters, Variant};
use std::io::Write;
use std::process::{Command, Stdio};

fn source() -> (NormalizedSpec, Variant) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let spec = serde_json::from_slice(
        &std::fs::read(root.join("tests/fixtures/briefs/courtyard.normalized.json")).unwrap(),
    )
    .unwrap();
    let generation: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("tests/fixtures/generation/courtyard.seed42.json")).unwrap(),
    )
    .unwrap();
    let variant = serde_json::from_value(generation["variants"][0].clone()).unwrap();
    (spec, variant)
}

#[test]
fn prepares_canonical_requests_and_evaluation() {
    let (spec, variant) = source();
    let blender = BlenderRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/blender",
    )
    .unwrap();
    assert_eq!(blender.evaluation().variant_id, variant.id);
    let bytes = blender.to_bytes().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["operation"], "build_massing");
    assert_eq!(value["params"]["id"], variant.id);
    let ifc = IfcRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/ifc",
        vec![],
    )
    .unwrap();
    assert_eq!(ifc.evaluation().variant_id, variant.id);
    let value: serde_json::Value = serde_json::from_slice(&ifc.to_bytes().unwrap()).unwrap();
    assert_eq!(value["operation"], "export_schematic_ifc");
    assert_eq!(value["variant"]["id"], variant.id);
}

#[test]
fn rejects_wrong_association_and_unsafe_path() {
    let (spec, variant) = source();
    assert!(
        BlenderRequest::new(
            &spec,
            &variant,
            "req-1",
            "run-1",
            &"a".repeat(64),
            "../escape"
        )
        .is_err()
    );
    let mut wrong = variant;
    wrong.project_id = "different".into();
    assert!(
        IfcRequest::new(
            &spec,
            &wrong,
            "req-1",
            "run-1",
            &"a".repeat(64),
            "runs/run-1/ifc",
            vec![]
        )
        .is_err()
    );
}

#[test]
fn rejects_tampered_identity_capacity_and_unscoped_sources() {
    let (spec, variant) = source();
    let mut tampered = variant.clone();
    tampered.id = format!("variant-{}", "0".repeat(64));
    assert!(
        BlenderRequest::new(
            &spec,
            &tampered,
            "req-1",
            "run-1",
            &"a".repeat(64),
            "runs/run-1/blender"
        )
        .is_err()
    );
    let too_many = Variant::new(
        spec.project_id.clone(),
        42,
        "analytic-v1".into(),
        ShapeParameters::Rectangle {
            origin_xy_m: [3., 3.],
            width_m: 10.,
            depth_m: 20.,
        },
        101,
        3.2,
    )
    .unwrap();
    assert!(
        IfcRequest::new(
            &spec,
            &too_many,
            "req-1",
            "run-1",
            &"a".repeat(64),
            "runs/run-1/ifc",
            vec![]
        )
        .is_err()
    );
    assert!(SourceArtifact::new("../escape", &"b".repeat(64)).is_err());
    assert!(
        BlenderRequest::new(
            &spec,
            &variant,
            "req;python",
            "run-1",
            &"a".repeat(64),
            "runs/run-1/blender"
        )
        .is_err()
    );
}

#[test]
fn rejects_duplicate_and_oversized_response_bytes() {
    let (spec, variant) = source();
    let request = BlenderRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/blender",
    )
    .unwrap();
    assert!(decode_blender_response(br#"{"status":"error","status":"ok"}"#, &request).is_err());
    assert!(decode_blender_response(&vec![b' '; 1024 * 1024 + 1], &request).is_err());
}

#[test]
fn strict_blender_response_fixture_and_mutations() {
    let (spec, variant) = source();
    let request = BlenderRequest::new(
        &spec,
        &variant,
        "request-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/blender",
    )
    .unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = std::fs::read(root.join("tests/fixtures/backend/blender-response.json")).unwrap();
    assert!(decode_blender_response(&fixture, &request).is_ok());
    let mut value: serde_json::Value = serde_json::from_slice(&fixture).unwrap();
    for (path, changed) in [
        ("/preset/engine", serde_json::json!("BLENDER_EEVEE_NEXT")),
        ("/preset/glb_export/compression", serde_json::json!(true)),
        (
            "/geometry/world_coordinate_max_error_m",
            serde_json::json!(0.01),
        ),
        ("/request_id", serde_json::json!("forged")),
        ("/build_hash", serde_json::json!("forged")),
        ("/artifacts/0/path", serde_json::json!("../scene.blend")),
    ] {
        let old = value.pointer(path).unwrap().clone();
        *value.pointer_mut(path).unwrap() = changed;
        assert!(
            decode_blender_response(&serde_json::to_vec(&value).unwrap(), &request).is_err(),
            "{path}"
        );
        *value.pointer_mut(path).unwrap() = old;
    }
    value["preset"]["world"]["unexpected"] = serde_json::json!(1);
    assert!(decode_blender_response(&serde_json::to_vec(&value).unwrap(), &request).is_err());
    assert!(decode_blender_response(br#"{"status":"error","status":"ok"}"#, &request).is_err());
    let text = String::from_utf8(fixture).unwrap();
    let duplicate = text.replace(
        "\"engine\":\"CYCLES\"",
        "\"engine\":\"CYCLES\",\"engine\":\"CPU\"",
    );
    assert!(
        decode_blender_response(duplicate.as_bytes(), &request)
            .unwrap_err()
            .message
            .contains("duplicate JSON key")
    );
    let nonfinite = text.replace("\"volume_m3\":", "\"volume_m3\":NaN,\"extra_volume\":");
    assert!(decode_blender_response(nonfinite.as_bytes(), &request).is_err());
}

#[test]
fn strict_ifc_response_fixture_and_mutations() {
    let (spec, variant) = source();
    let request = IfcRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/ifc",
        vec![],
    )
    .unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = std::fs::read(root.join("tests/fixtures/backend/ifc-response.json")).unwrap();
    decode_ifc_response(&fixture, &request).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&fixture).unwrap();
    for (path, changed) in [
        ("/backend_version", serde_json::json!("0.9.0")),
        ("/validation/draft", serde_json::json!(false)),
        (
            "/validation/reopen_process/returncode",
            serde_json::json!(1),
        ),
        (
            "/validation/reopen_process/failure",
            serde_json::json!("timeout"),
        ),
        (
            "/validation/reopen_process/stderr",
            serde_json::json!("x".repeat(4097)),
        ),
        (
            "/validation/express_validation/exit_code",
            serde_json::json!(1),
        ),
        (
            "/validation/express_validation/failure",
            serde_json::json!("output_limit"),
        ),
        (
            "/validation/express_validation/stderr",
            serde_json::json!("x".repeat(4097)),
        ),
        (
            "/validation/tessellation/0/volume_m3",
            serde_json::json!(-1),
        ),
        ("/artifacts/0/media_type", serde_json::json!("text/plain")),
    ] {
        let old = value.pointer(path).unwrap().clone();
        *value.pointer_mut(path).unwrap() = changed;
        assert!(
            decode_ifc_response(&serde_json::to_vec(&value).unwrap(), &request).is_err(),
            "{path}"
        );
        *value.pointer_mut(path).unwrap() = old;
    }
    value["validation"]["reopen_process"]["unexpected"] = serde_json::json!(true);
    assert!(decode_ifc_response(&serde_json::to_vec(&value).unwrap(), &request).is_err());
    value["validation"]["reopen_process"]
        .as_object_mut()
        .unwrap()
        .remove("unexpected");
    value["validation"]["express_validation"]["unexpected"] = serde_json::json!(true);
    assert!(decode_ifc_response(&serde_json::to_vec(&value).unwrap(), &request).is_err());
    value["validation"]["express_validation"]
        .as_object_mut()
        .unwrap()
        .remove("unexpected");
    value["validation"]["reopen_process"]
        .as_object_mut()
        .unwrap()
        .remove("failure");
    assert!(decode_ifc_response(&serde_json::to_vec(&value).unwrap(), &request).is_err());
}

#[test]
#[ignore = "requires pinned CPython 3.12.16 and IfcOpenShell worker; set ARCHY_IFC_PYTHON"]
fn rust_request_to_native_ifc_worker_to_rust_decoder() {
    let python = std::env::var("ARCHY_IFC_PYTHON").expect("set ARCHY_IFC_PYTHON");
    let (spec, variant) = source();
    let request = IfcRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/ifc",
        vec![],
    )
    .unwrap();
    let bytes = request.to_bytes().unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = std::fs::read(root.join("tests/fixtures/backend/ifc-request.json")).unwrap();
    assert_eq!(bytes, fixture.trim_ascii_end());
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let project =
        std::env::temp_dir().join(format!("archy-ifc-adapter-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(project.join("runs/run-1")).unwrap();
    let request_path = project.join("request.json");
    std::fs::write(&request_path, bytes).unwrap();
    let output = Command::new(python)
        .args(["-I", "-m", "archy_ifc_worker", "--project-root"])
        .arg(&project)
        .arg("--request")
        .arg(&request_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response = std::fs::read(project.join("runs/run-1/ifc/response.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(
        parsed["validation"]["reopen_process"]["failure"],
        serde_json::Value::Null
    );
    assert_eq!(
        parsed["validation"]["express_validation"]["failure"],
        serde_json::Value::Null
    );
    decode_ifc_response(&response, &request).unwrap();
    std::fs::remove_dir_all(project).unwrap();
}

#[test]
fn error_responses_cannot_claim_partial_artifacts() {
    let (spec, variant) = source();
    let blender = BlenderRequest::new(
        &spec,
        &variant,
        "request-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/blender",
    )
    .unwrap();
    let ifc = IfcRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/ifc",
        vec![],
    )
    .unwrap();
    let error = serde_json::json!({
        "code": "ARTIFACT_INVALID", "field": "artifacts", "message": "failed", "retriable": false
    });
    let mut blender_error = serde_json::json!({
        "status": "error", "schema_version": "blender-worker-v1", "error": error,
        "artifacts": []
    });
    assert!(
        decode_blender_response(&serde_json::to_vec(&blender_error).unwrap(), &blender).is_ok()
    );
    blender_error["artifacts"] = serde_json::json!([{"path": "scene.blend"}]);
    assert!(
        decode_blender_response(&serde_json::to_vec(&blender_error).unwrap(), &blender).is_err()
    );

    let mut ifc_error = serde_json::json!({
        "status": "error", "schema_version": "ifc-worker-v1", "error": error,
        "request_id": "req-1", "project_id": spec.project_id,
        "variant_id": variant.id, "run_id": "run-1",
        "source_digest": "a".repeat(64), "artifacts": []
    });
    assert!(decode_ifc_response(&serde_json::to_vec(&ifc_error).unwrap(), &ifc).is_ok());
    ifc_error["artifacts"] = serde_json::json!([{"path": "model.ifc"}]);
    assert!(decode_ifc_response(&serde_json::to_vec(&ifc_error).unwrap(), &ifc).is_err());
}

#[test]
fn rust_serializer_matches_python_validated_fixtures() {
    let (spec, variant) = source();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let blender = BlenderRequest::new(
        &spec,
        &variant,
        "request-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/blender",
    )
    .unwrap();
    let ifc = IfcRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/ifc",
        vec![],
    )
    .unwrap();
    let cases = [
        ("blender", blender.to_bytes().unwrap()),
        ("ifc", ifc.to_bytes().unwrap()),
    ];
    for (backend, bytes) in cases {
        let expected = std::fs::read_to_string(
            root.join(format!("tests/fixtures/backend/{backend}-request.json")),
        )
        .unwrap();
        assert_eq!(bytes, expected.trim_end_matches('\n').as_bytes());
        let script = if backend == "blender" {
            "import sys; sys.path.insert(0,'workers/blender'); import protocol; protocol.parse_request(sys.stdin.buffer.read())"
        } else {
            "import sys; sys.path.insert(0,'workers/ifc'); from archy_ifc_worker import protocol; protocol.parse_request(sys.stdin.buffer.read())"
        };
        let mut child = Command::new("python3")
            .arg("-c")
            .arg(script)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
