use archy_adapters::{
    BlenderRequest, CaptureOutcome, ErrorBinding, IfcRequest, capture_blender, capture_ifc,
};
use archy_core::{ErrorCode, NormalizedSpec, Variant, sha256_hex};
use std::fs::{self, File};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;
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
    fn ifc_path(&self, name: &str) -> PathBuf {
        self.0.join("ifc").join(name)
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
        "output/ifc",
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
        "output/ifc",
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
    if backend == "ifc" {
        fs::create_dir_all(scratch.path("ifc")).unwrap();
    }
    let path = |name: &str| {
        if backend == "ifc" {
            scratch.ifc_path(name)
        } else {
            scratch.path(name)
        }
    };
    for artifact in value["artifacts"].as_array_mut().unwrap() {
        let name = artifact["path"].as_str().unwrap().to_string();
        let bytes = format!("captured:{name}").into_bytes();
        fs::write(path(&name), &bytes).unwrap();
        artifact["sha256"] = sha256_hex(&bytes).into();
        artifact["size_bytes"] = bytes.len().into();
    }
    fs::write(path("response.json"), serde_json::to_vec(&value).unwrap()).unwrap();
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
    fs::create_dir(scratch.path("ifc")).unwrap();
    let bytes = serde_json::to_vec(&error).unwrap();
    fs::write(scratch.ifc_path("response.json"), &bytes).unwrap();
    fs::write(scratch.ifc_path("model.ifc"), b"partial").unwrap();
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
    fs::write(scratch.ifc_path("unexpected"), b"extra").unwrap();
    let CaptureOutcome::WorkerError(failure) = capture_ifc(&scratch.dir(), &ifc_request()).unwrap()
    else {
        panic!("not worker error")
    };
    assert_eq!(failure.response_bytes(), bytes);
    assert_eq!(
        failure.scratch_issue().unwrap().code,
        ErrorCode::ArtifactInvalid
    );
    fs::remove_file(scratch.ifc_path("unexpected")).unwrap();
    fs::remove_file(scratch.ifc_path("model.ifc")).unwrap();
    rustix::fs::mkfifoat(
        File::open(scratch.path("ifc")).unwrap(),
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
        scratch.ifc_path("response.json"),
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
        scratch.ifc_path("response.json"),
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
    File::create(scratch.ifc_path("model.ifc"))
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
    fs::write(scratch.ifc_path("model.ifc"), &model).unwrap();
    response["artifacts"][0]["size_bytes"] = model.len().into();
    response["artifacts"][0]["sha256"] = sha256_hex(&model).into();
    let mut bytes = serde_json::to_vec(&response).unwrap();
    bytes.resize(1024 * 1024, b' ');
    fs::write(scratch.ifc_path("response.json"), &bytes).unwrap();
    let CaptureOutcome::Success(outputs) = capture_ifc(&scratch.dir(), &ifc_request()).unwrap()
    else {
        panic!("not success")
    };
    assert_eq!(outputs.response_bytes().len(), 1024 * 1024);
    assert_eq!(outputs.artifacts()[0].bytes().len(), 16 * 1024 * 1024);
    bytes.push(b' ');
    fs::write(scratch.ifc_path("response.json"), &bytes).unwrap();
    assert_eq!(
        capture_ifc(&scratch.dir(), &ifc_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
}

#[test]
fn actual_ifc_worker_layout_and_handled_error_capture_without_native_backend() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = r#"
import hashlib, json, sys
from unittest import mock
sys.path.insert(0, sys.argv[1] + '/workers/ifc')
from archy_ifc_worker import worker, protocol

def run(request, output, request_path):
    if sys.argv[4] == 'error':
        raise protocol.ProtocolError('fixture native failure', 'model.ifc', 'ARTIFACT_INVALID')
    value = json.load(open(sys.argv[1] + '/tests/fixtures/backend/ifc-response.json'))
    for key in ('request_id', 'project_id', 'variant_id', 'run_id', 'source_digest'):
        value[key] = request[key]
    for item in value['artifacts']:
        data = ('actual-worker:' + item['path']).encode()
        (output / item['path']).write_bytes(data)
        item['size_bytes'] = len(data)
        item['sha256'] = hashlib.sha256(data).hexdigest()
    return value

with mock.patch.object(worker, 'run', side_effect=run):
    sys.exit(worker.main(['--project-root', sys.argv[2], '--request', sys.argv[3]]))
"#;
    for mode in ["success", "error"] {
        let scratch = Scratch::new();
        fs::create_dir(scratch.path("input")).unwrap();
        fs::create_dir(scratch.path("output")).unwrap();
        let request_path = scratch.path("input/request.json");
        fs::write(&request_path, ifc_request().to_bytes().unwrap()).unwrap();
        let status = Command::new("python3")
            .args([
                "-I",
                "-c",
                script,
                root.to_str().unwrap(),
                scratch.0.to_str().unwrap(),
                request_path.to_str().unwrap(),
                mode,
            ])
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(if mode == "success" { 0 } else { 20 }));
        let output = File::open(scratch.path("output")).unwrap();
        let bytes = fs::read(scratch.path("output/ifc/response.json")).unwrap();
        let captured = capture_ifc(&output, &ifc_request()).unwrap();
        match captured {
            CaptureOutcome::Success(value) if mode == "success" => {
                assert_eq!(value.response_bytes(), bytes);
                assert_eq!(value.artifacts().len(), 2);
                assert_eq!(value.artifacts()[0].bytes(), b"actual-worker:model.ifc");
            }
            CaptureOutcome::WorkerError(value) if mode == "error" => {
                assert_eq!(value.response_bytes(), bytes);
                assert_eq!(
                    value.bound_error().unwrap().code,
                    ErrorCode::ArtifactInvalid
                );
                assert!(value.scratch_issue().is_none());
                assert_eq!(
                    capture_ifc(&output, &other_ifc_request()).unwrap_err().code,
                    ErrorCode::ArtifactInvalid
                );
            }
            other => panic!("unexpected worker capture: {other:?}"),
        }
        assert!(!scratch.path("published").exists());
        let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        forged["request_id"] = "forged".into();
        fs::write(
            scratch.path("output/ifc/response.json"),
            serde_json::to_vec(&forged).unwrap(),
        )
        .unwrap();
        assert_eq!(
            capture_ifc(&output, &ifc_request()).unwrap_err().code,
            ErrorCode::ArtifactInvalid
        );
        fs::remove_file(scratch.path("output/ifc/response.json")).unwrap();
        assert_eq!(
            capture_ifc(&output, &ifc_request()).unwrap_err().code,
            ErrorCode::ArtifactInvalid
        );
        fs::write(scratch.path("output/ifc/response.json"), &bytes).unwrap();
        fs::write(scratch.path("output/extra"), b"unexpected").unwrap();
        assert_eq!(
            capture_ifc(&output, &ifc_request()).unwrap_err().code,
            ErrorCode::ArtifactInvalid
        );
    }
}

#[test]
fn ifc_output_directory_cannot_be_a_symlink() {
    let scratch = Scratch::new();
    let outside = Scratch::new();
    fs::write(outside.path("response.json"), b"sentinel").unwrap();
    symlink(&outside.0, scratch.path("ifc")).unwrap();
    assert_eq!(
        capture_ifc(&scratch.dir(), &ifc_request())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    assert_eq!(
        fs::read(outside.path("response.json")).unwrap(),
        b"sentinel"
    );
}

#[test]
fn actual_blender_worker_handled_error_uses_flat_production_layout() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = Scratch::new();
    fs::create_dir(scratch.path("input")).unwrap();
    fs::create_dir(scratch.path("output")).unwrap();
    let (spec, variant) = source();
    let request = BlenderRequest::new(
        &spec,
        &variant,
        "request-1",
        "run-1",
        &"a".repeat(64),
        "output",
    )
    .unwrap();
    let request_path = scratch.path("input/request.json");
    fs::write(&request_path, request.to_bytes().unwrap()).unwrap();
    let response_path = scratch.path("output/response.json");
    let script = r#"
import sys, types
from unittest import mock
sys.modules['bpy'] = types.ModuleType('bpy')
sys.path.insert(0, sys.argv[1] + '/workers/blender')
import worker, protocol
sys.argv = ['worker.py', '--', '--request', sys.argv[2], '--response', sys.argv[3]]
with mock.patch.object(worker, '_run', side_effect=protocol.ProtocolError(
    'fixture render failure', 'backend', 'BACKEND_UNAVAILABLE')):
    worker.main()
"#;
    let status = Command::new("python3")
        .args([
            "-I",
            "-c",
            script,
            root.to_str().unwrap(),
            request_path.to_str().unwrap(),
            response_path.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(20));
    let output = File::open(scratch.path("output")).unwrap();
    let bytes = fs::read(&response_path).unwrap();
    let CaptureOutcome::WorkerError(failure) = capture_blender(&output, &request).unwrap() else {
        panic!("expected handled Blender error")
    };
    assert_eq!(failure.binding(), ErrorBinding::UnboundDiagnostic);
    assert_eq!(failure.response_bytes(), bytes);
    assert_eq!(
        failure.diagnostic_error().code,
        ErrorCode::BackendUnavailable
    );
    assert!(!scratch.path("published").exists());
    fs::write(&response_path, b"{\"status\":\"ok\"}").unwrap();
    assert_eq!(
        capture_blender(&output, &request).unwrap_err().code,
        ErrorCode::ArtifactInvalid
    );
}
