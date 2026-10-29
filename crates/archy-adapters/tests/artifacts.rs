use archy_adapters::{
    BlenderRequest, CaptureOutcome, ErrorBinding, IfcRequest, capture_blender, capture_ifc,
};
use archy_core::{ErrorCode, NormalizedSpec, Variant, sha256_hex};
use std::fs::{self, File};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "archy-artifact-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn dir(&self) -> File {
        File::open(&self.0).unwrap()
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn source() -> (NormalizedSpec, Variant) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let spec = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/briefs/courtyard.normalized.json")).unwrap(),
    )
    .unwrap();
    let generation: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/generation/courtyard.seed42.json")).unwrap(),
    )
    .unwrap();
    let variant = serde_json::from_value(generation["variants"][0].clone()).unwrap();
    (spec, variant)
}

fn blender_request() -> BlenderRequest {
    let (spec, variant) = source();
    BlenderRequest::new(
        &spec,
        &variant,
        "request-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/blender",
    )
    .unwrap()
}
fn other_blender_request() -> BlenderRequest {
    let (spec, variant) = source();
    BlenderRequest::new(
        &spec,
        &variant,
        "request-2",
        "run-2",
        &"b".repeat(64),
        "runs/run-2/blender",
    )
    .unwrap()
}
fn ifc_request() -> IfcRequest {
    let (spec, variant) = source();
    IfcRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/ifc",
        vec![],
    )
    .unwrap()
}
fn other_ifc_request() -> IfcRequest {
    let (spec, variant) = source();
    IfcRequest::new(
        &spec,
        &variant,
        "req-2",
        "run-2",
        &"b".repeat(64),
        "runs/run-2/ifc",
        vec![],
    )
    .unwrap()
}

fn success(scratch: &Scratch, backend: &str) -> serde_json::Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut value: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(format!("tests/fixtures/backend/{backend}-response.json"))).unwrap(),
    )
    .unwrap();
    for artifact in value["artifacts"].as_array_mut().unwrap() {
        let name = artifact["path"].as_str().unwrap().to_string();
        let bytes = format!("captured:{name}").into_bytes();
        fs::write(scratch.path(&name), &bytes).unwrap();
        artifact["sha256"] = sha256_hex(&bytes).into();
        artifact["size_bytes"] = bytes.len().into();
    }
    fs::write(
        scratch.path("response.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    value
}

#[test]
fn captures_blender_and_ifc_claimed_bytes_without_semantic_authority() {
    let blender = Scratch::new();
    success(&blender, "blender");
    let CaptureOutcome::Success(outputs) =
        capture_blender(&blender.dir(), &blender_request()).unwrap()
    else {
        panic!("not success")
    };
    assert_eq!(outputs.artifacts().len(), 5);
    assert!(outputs.response_bytes().starts_with(b"{"));
    assert_eq!(
        outputs.response_sha256(),
        sha256_hex(outputs.response_bytes())
    );
    let scene = outputs
        .artifacts()
        .iter()
        .find(|item| item.path() == "scene.blend")
        .unwrap();
    assert_eq!(scene.bytes(), b"captured:scene.blend");
    assert_eq!(scene.sha256(), sha256_hex(scene.bytes()));
    fs::remove_file(blender.path("scene.blend")).unwrap();
    assert_eq!(scene.bytes(), b"captured:scene.blend");
    let ifc = Scratch::new();
    success(&ifc, "ifc");
    let CaptureOutcome::Success(outputs) = capture_ifc(&ifc.dir(), &ifc_request()).unwrap() else {
        panic!("not success")
    };
    assert_eq!(outputs.artifacts().len(), 2);
    assert_eq!(outputs.artifacts()[0].path(), "model.ifc");
}

#[test]
fn declared_worker_error_retains_response_and_does_not_require_success_inventory() {
    let scratch = Scratch::new();
    let error = serde_json::json!({"status":"error","schema_version":"ifc-worker-v1",
        "request_id":"req-1","project_id":"courtyard-demo","variant_id":source().1.id,
        "run_id":"run-1","source_digest":"a".repeat(64),"artifacts":[],
        "error":{"code":"BACKEND_TIMEOUT","field":"backend","message":"worker timed out","retriable":false}});
    let bytes = serde_json::to_vec(&error).unwrap();
    fs::write(scratch.path("response.json"), &bytes).unwrap();
    fs::write(scratch.path("model.ifc"), b"partial").unwrap();
    let CaptureOutcome::WorkerError(failure) = capture_ifc(&scratch.dir(), &ifc_request()).unwrap()
    else {
        panic!("not worker error")
    };
    assert_eq!(failure.response_bytes(), bytes);
    assert_eq!(failure.response_sha256(), sha256_hex(&bytes));
    assert_eq!(failure.binding(), ErrorBinding::RequestBound);
    assert_eq!(
        failure.bound_error().unwrap().code,
        ErrorCode::BackendTimeout
    );
    assert!(failure.scratch_issue().is_none());
    assert_eq!(
        capture_ifc(&scratch.dir(), &other_ifc_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    fs::write(scratch.path("unexpected"), b"extra").unwrap();
    let CaptureOutcome::WorkerError(failure) = capture_ifc(&scratch.dir(), &ifc_request()).unwrap()
    else {
        panic!("not worker error")
    };
    assert_eq!(failure.response_bytes(), bytes);
    assert_eq!(
        failure.scratch_issue().unwrap().code,
        ErrorCode::ArtifactInvalid
    );
    fs::remove_file(scratch.path("unexpected")).unwrap();
    fs::remove_file(scratch.path("model.ifc")).unwrap();
    rustix::fs::mkfifoat(
        scratch.dir(),
        "model.ifc",
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let CaptureOutcome::WorkerError(failure) = capture_ifc(&scratch.dir(), &ifc_request()).unwrap()
    else {
        panic!("not worker error")
    };
    assert_eq!(failure.response_bytes(), bytes);
    assert_eq!(
        failure.scratch_issue().unwrap().code,
        ErrorCode::ArtifactInvalid
    );
}

#[test]
fn blender_declared_error_retains_bytes_with_known_partial_output() {
    let scratch = Scratch::new();
    let response = serde_json::json!({"status":"error","schema_version":"blender-worker-v1",
        "artifacts":[],"error":{"code":"BACKEND_UNAVAILABLE","field":"backend",
        "message":"runtime absent","retriable":false}});
    let bytes = serde_json::to_vec(&response).unwrap();
    fs::write(scratch.path("response.json"), &bytes).unwrap();
    fs::write(scratch.path("scene.blend"), b"partial").unwrap();
    let CaptureOutcome::WorkerError(failure) =
        capture_blender(&scratch.dir(), &blender_request()).unwrap()
    else {
        panic!("not worker error")
    };
    assert_eq!(failure.binding(), ErrorBinding::UnboundDiagnostic);
    assert!(failure.bound_error().is_none());
    assert_eq!(
        failure.diagnostic_error().code,
        ErrorCode::BackendUnavailable
    );
    assert_eq!(failure.response_bytes(), bytes);
    assert_eq!(failure.response_sha256(), sha256_hex(&bytes));
    assert!(failure.scratch_issue().is_none());
    let CaptureOutcome::WorkerError(other_failure) =
        capture_blender(&scratch.dir(), &other_blender_request()).unwrap()
    else {
        panic!("not worker error")
    };
    assert_eq!(other_failure.binding(), ErrorBinding::UnboundDiagnostic);
    assert!(other_failure.bound_error().is_none());
    assert_eq!(other_failure.response_bytes(), bytes);
    assert_eq!(other_failure.response_sha256(), failure.response_sha256());
}

#[test]
fn rejects_missing_extra_symlink_hardlink_and_fifo_without_blocking() {
    let scratch = Scratch::new();
    success(&scratch, "blender");
    fs::remove_file(scratch.path("scene.blend")).unwrap();
    assert_eq!(
        capture_blender(&scratch.dir(), &blender_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    fs::write(scratch.path("scene.blend"), b"captured:scene.blend").unwrap();
    fs::write(scratch.path("extra"), b"extra").unwrap();
    assert_eq!(
        capture_blender(&scratch.dir(), &blender_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    fs::remove_file(scratch.path("extra")).unwrap();
    fs::remove_file(scratch.path("scene.blend")).unwrap();
    let outside = Scratch::new();
    fs::write(outside.path("sentinel"), b"unchanged").unwrap();
    symlink(outside.path("sentinel"), scratch.path("scene.blend")).unwrap();
    assert_eq!(
        capture_blender(&scratch.dir(), &blender_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    assert_eq!(fs::read(outside.path("sentinel")).unwrap(), b"unchanged");
    fs::remove_file(scratch.path("scene.blend")).unwrap();
    fs::hard_link(outside.path("sentinel"), scratch.path("scene.blend")).unwrap();
    assert_eq!(
        capture_blender(&scratch.dir(), &blender_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    fs::remove_file(scratch.path("scene.blend")).unwrap();
    rustix::fs::mkfifoat(
        scratch.dir(),
        "scene.blend",
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let start = std::time::Instant::now();
    assert_eq!(
        capture_blender(&scratch.dir(), &blender_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
    fs::remove_file(scratch.path("scene.blend")).unwrap();
    fs::create_dir(scratch.path("scene.blend")).unwrap();
    assert_eq!(
        capture_blender(&scratch.dir(), &blender_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
}

#[test]
fn rejects_claim_mismatch_traversal_and_oversized_regular_file() {
    let scratch = Scratch::new();
    let mut response = success(&scratch, "ifc");
    response["artifacts"][0]["sha256"] = "f".repeat(64).into();
    fs::write(
        scratch.path("response.json"),
        serde_json::to_vec(&response).unwrap(),
    )
    .unwrap();
    assert_eq!(
        capture_ifc(&scratch.dir(), &ifc_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    response["artifacts"][0]["path"] = "../model.ifc".into();
    fs::write(
        scratch.path("response.json"),
        serde_json::to_vec(&response).unwrap(),
    )
    .unwrap();
    assert_eq!(
        capture_ifc(&scratch.dir(), &ifc_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    success(&scratch, "ifc");
    File::create(scratch.path("model.ifc"))
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert_eq!(
        capture_ifc(&scratch.dir(), &ifc_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
}

#[test]
fn accepts_exact_response_and_artifact_limits_then_rejects_one_byte_over() {
    let scratch = Scratch::new();
    let mut response = success(&scratch, "ifc");
    let model = vec![b'x'; 16 * 1024 * 1024];
    fs::write(scratch.path("model.ifc"), &model).unwrap();
    response["artifacts"][0]["size_bytes"] = model.len().into();
    response["artifacts"][0]["sha256"] = sha256_hex(&model).into();
    let mut bytes = serde_json::to_vec(&response).unwrap();
    bytes.resize(1024 * 1024, b' ');
    fs::write(scratch.path("response.json"), &bytes).unwrap();
    let CaptureOutcome::Success(outputs) = capture_ifc(&scratch.dir(), &ifc_request()).unwrap()
    else {
        panic!("not success")
    };
    assert_eq!(outputs.response_bytes().len(), 1024 * 1024);
    assert_eq!(outputs.artifacts()[0].bytes().len(), 16 * 1024 * 1024);
    bytes.push(b' ');
    fs::write(scratch.path("response.json"), &bytes).unwrap();
    assert_eq!(
        capture_ifc(&scratch.dir(), &ifc_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
}
