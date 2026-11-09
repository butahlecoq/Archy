use archy_cli::WorkflowReceipt;
use archy_core::{
    ErrorCode, MutationContext, ProjectStore, RevisionRecord, RunRequest, sha256_hex,
};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    os::{
        fd::{AsFd, OwnedFd},
        unix::{fs::DirBuilderExt, net::UnixStream, process::ExitStatusExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("archy-edit-interruption-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        fs::write(
            root.join("project.json"),
            include_bytes!("../../../tests/fixtures/briefs/courtyard.json"),
        )
        .unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Writer {
    child: Child,
    _channel: UnixStream,
}

impl Writer {
    fn spawn(root: &Path) -> Self {
        let directory = fs::File::open(root).unwrap();
        let (parent_channel, child_channel) = UnixStream::pair().unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "stage_edit_until_killed", "--nocapture"])
            .stdin(Stdio::from(directory))
            .stdout(Stdio::from(OwnedFd::from(child_channel)))
            .spawn()
            .unwrap();
        Self {
            child,
            _channel: parent_channel,
        }
    }

    fn kill_and_wait(&mut self) -> std::process::ExitStatus {
        self.child.kill().unwrap();
        self.child.wait().unwrap()
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn cli(args: &[&str]) -> WorkflowReceipt {
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "CLI failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn desktop_status(project: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["desktop", "status", "--project", project, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn edit_args<'a>(
    project: &'a str,
    from: &'a str,
    variant: &'a str,
    key: &'a str,
    revision: &'a str,
    params: &'a str,
) -> [&'a str; 14] {
    [
        "generate",
        "--project",
        project,
        "--from-run",
        from,
        "--variant",
        variant,
        "--params",
        params,
        "--idempotency-key",
        key,
        "--expected-project-revision",
        revision,
        "--json",
    ]
}

fn inventory(root: &Path, run_path: &str, revision: u64) -> BTreeMap<String, String> {
    fn files(root: &Path, path: &Path, hashes: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                files(root, &entry.path(), hashes);
            } else {
                hashes.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    sha256_hex(&fs::read(entry.path()).unwrap()),
                );
            }
        }
    }
    let mut hashes = BTreeMap::new();
    files(root, &root.join(run_path), &mut hashes);
    let revision_path = format!(".archy/revisions/{revision:020}.json");
    hashes.insert(
        revision_path.clone(),
        sha256_hex(&fs::read(root.join(revision_path)).unwrap()),
    );
    hashes
}

#[test]
fn stage_edit_until_killed() {
    let root = PathBuf::from("/proc/self/fd/0");
    if !fs::metadata(&root).is_ok_and(|metadata| metadata.is_dir()) {
        return;
    }
    let stdout = std::io::stdout();
    let fd = stdout.as_fd().try_clone_to_owned().unwrap();
    let mut channel = UnixStream::from(fd);
    channel
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let request: RunRequest =
        serde_json::from_slice(&fs::read(root.join("edit-request.json")).unwrap()).unwrap();
    let mut fingerprint_input = b"archy-run-request-fingerprint-v1\0".to_vec();
    serde_json::to_writer(&mut fingerprint_input, &request).unwrap();
    let request_sha256 = sha256_hex(&fingerprint_input);
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request).unwrap().pending().unwrap();
    let staged_id = pending.run_id().to_owned();
    pending
        .write(
            "params.json",
            &fs::read(root.join("edit.json")).unwrap(),
            "application/json",
            "archy-cli",
        )
        .unwrap();
    fs::write(
        root.join("edit-ready.tmp"),
        serde_json::to_vec(&serde_json::json!({
            "run_id": staged_id,
            "request_sha256": request_sha256,
        }))
        .unwrap(),
    )
    .unwrap();
    fs::rename(root.join("edit-ready.tmp"), root.join("edit-ready")).unwrap();
    let _ = channel.read(&mut [0u8; 1]);
}

#[test]
fn interrupted_edit_retries_once_and_restore_keeps_history() {
    let fixture = Fixture::new();
    let root = &fixture.0;
    let project = root.join("project.json");
    let project_arg = project.to_str().unwrap();
    let first = cli(&[
        "generate",
        "--project",
        project_arg,
        "--count",
        "1",
        "--seed",
        "42",
        "--json",
    ]);
    assert_eq!(first.revision, 1);
    let original = archy_cli::verify_run(&project, &first.run_id).unwrap();
    let variant = &original.variants()[0];
    let original_params = serde_json::to_value(&variant.parameters).unwrap();
    let mut changed = original_params.clone();
    changed["width_m"] = serde_json::json!(changed["width_m"].as_f64().unwrap() + 0.25);
    let edit_bytes = serde_json::to_vec(&serde_json::json!({
        "parameters": changed,
        "storeys": variant.storeys,
        "floor_to_floor_m": variant.floor_to_floor_m
    }))
    .unwrap();
    fs::write(root.join("edit.json"), &edit_bytes).unwrap();
    let store = ProjectStore::open(&project).unwrap();
    let history = store.read_history().unwrap();
    let first_manifest = history.read_run(&first.run_id).unwrap().manifest().clone();
    let before = inventory(root, &first.path, first.revision);
    println!("accepted inventory before edit interruption: {before:?}");
    let request = RunRequest {
        schema_version: archy_core::RUN_REQUEST_SCHEMA_VERSION.into(),
        context: MutationContext {
            idempotency_key: "interrupted-edit".into(),
            expected_project_revision: 1,
            destination: "runs".into(),
        },
        operation: "edit".into(),
        request_bytes: serde_json::to_vec(&serde_json::json!({
            "mode": "edit",
            "from_run": first.run_id,
            "variant": first.variant_ids[0],
            "params_sha256": sha256_hex(&edit_bytes),
            "profile": "si-rectangle-v2"
        }))
        .unwrap(),
        input_sha256: first_manifest.input_sha256,
        source_run: Some(first.run_id.clone()),
    };
    fs::write(
        root.join("edit-request.json"),
        serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    let mut writer = Writer::spawn(root);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !root.join("edit-ready").exists() {
        if let Some(status) = writer.child.try_wait().unwrap() {
            panic!("writer exited before staging: {status}");
        }
        assert!(Instant::now() < deadline, "writer did not stage edit");
        thread::sleep(Duration::from_millis(10));
    }
    let staged: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("edit-ready")).unwrap()).unwrap();
    let staged_id = staged["run_id"].as_str().unwrap();
    let staged_request_sha256 = staged["request_sha256"].as_str().unwrap();
    assert_eq!(
        fs::read(
            root.join(".archy/staging")
                .join(staged_id)
                .join("params.json")
        )
        .unwrap(),
        edit_bytes
    );
    let signal = writer.kill_and_wait().signal();
    assert_eq!(signal, Some(9));
    println!("edit staging writer signal: {signal:?}; staged run: {staged_id}");
    drop(writer);
    let restarted = ProjectStore::open(&project).unwrap();
    let history = restarted.read_history().unwrap();
    assert_eq!(history.revision(), 1);
    assert_eq!(
        history.read_run(staged_id).err().unwrap().code,
        ErrorCode::ArtifactInvalid
    );
    let status = desktop_status(project_arg);
    assert_eq!(status["project"]["revision"], 1);
    assert_eq!(status["runs"].as_array().unwrap().len(), 1);
    assert_eq!(status["runs"][0]["run_id"], first.run_id);
    assert_eq!(inventory(root, &first.path, first.revision), before);
    println!(
        "accepted inventory after edit interruption: {:?}",
        inventory(root, &first.path, first.revision)
    );
    assert!(archy_cli::verify_run(&project, &first.run_id).is_ok());

    let edited = cli(&edit_args(
        project_arg,
        &first.run_id,
        &first.variant_ids[0],
        "interrupted-edit",
        "1",
        "edit.json",
    ));
    assert_eq!(edited.revision, 2);
    println!("edit retry revision: {}", edited.revision);
    let revision: RevisionRecord = serde_json::from_slice(
        &fs::read(root.join(".archy/revisions/00000000000000000002.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(revision.receipt.run_id, edited.run_id);
    assert_eq!(revision.receipt.revision, edited.revision);
    println!(
        "staged request fingerprint: {staged_request_sha256}; accepted request fingerprint: {}",
        revision.request_sha256
    );
    assert_eq!(
        staged_request_sha256, revision.request_sha256,
        "staged request differs from the accepted CLI retry"
    );
    let replay = cli(&edit_args(
        project_arg,
        &first.run_id,
        &first.variant_ids[0],
        "interrupted-edit",
        "1",
        "edit.json",
    ));
    assert_eq!(edited, replay);
    let edited_run = archy_cli::verify_run(&project, &edited.run_id).unwrap();
    assert_eq!(
        edited_run.manifest().parent_run_id.as_deref(),
        Some(first.run_id.as_str())
    );
    assert_eq!(
        edited_run.manifest().parent_variant_id.as_deref(),
        Some(first.variant_ids[0].as_str())
    );
    assert_eq!(inventory(root, &first.path, first.revision), before);
    let edited_before = inventory(root, &edited.path, edited.revision);
    assert_eq!(restarted.read_history().unwrap().revision(), 2);
    let status = desktop_status(project_arg);
    assert_eq!(status["project"]["revision"], 2);
    assert_eq!(status["runs"].as_array().unwrap().len(), 2);

    let restore_bytes = serde_json::to_vec(&serde_json::json!({
        "parameters": original_params,
        "storeys": variant.storeys,
        "floor_to_floor_m": variant.floor_to_floor_m
    }))
    .unwrap();
    fs::write(root.join("restore.json"), restore_bytes).unwrap();
    let restored = cli(&edit_args(
        project_arg,
        &edited.run_id,
        &edited.variant_ids[0],
        "restore-after-interruption",
        "2",
        "restore.json",
    ));
    assert_eq!(restored.revision, 3);
    println!("restore revision: {}", restored.revision);
    assert_eq!(restored.variant_ids, first.variant_ids);
    assert_eq!(inventory(root, &first.path, first.revision), before);
    assert_eq!(
        inventory(root, &edited.path, edited.revision),
        edited_before
    );
    assert!(archy_cli::verify_run(&project, &first.run_id).is_ok());
    assert!(archy_cli::verify_run(&project, &edited.run_id).is_ok());
    assert!(archy_cli::verify_run(&project, &restored.run_id).is_ok());
    assert_eq!(restarted.read_history().unwrap().revision(), 3);
    assert_eq!(restarted.read_history().unwrap().receipts().count(), 3);
    assert_eq!(
        fs::read_dir(root.join(".archy/staging")).unwrap().count(),
        1
    );
}
