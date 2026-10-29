use archy_core::{
    ErrorCode, MutationContext, ProjectStore, RunMetadata, RunReceipt, RunRequest, sha256_hex,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    io::Read,
    os::{
        fd::{AsFd, OwnedFd},
        unix::{fs::DirBuilderExt, net::UnixStream, process::ExitStatusExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("archy-recovery-staging-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Writer {
    child: Option<Child>,
    reaped: Rc<RefCell<Option<ExitStatus>>>,
    _parent_channel: UnixStream,
}

impl Writer {
    fn spawn(
        root: &Path,
        reaped: Rc<RefCell<Option<ExitStatus>>>,
        untrusted_root: Option<&Path>,
    ) -> Self {
        Self::spawn_test(root, reaped, untrusted_root, "child_stages_until_killed")
    }

    fn spawn_test(
        root: &Path,
        reaped: Rc<RefCell<Option<ExitStatus>>>,
        untrusted_root: Option<&Path>,
        test_name: &str,
    ) -> Self {
        let directory = fs::File::open(root).unwrap();
        let (parent_channel, child_channel) = UnixStream::pair().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", test_name, "--nocapture"])
            .stdin(Stdio::from(directory))
            .stdout(Stdio::from(OwnedFd::from(child_channel)));
        if let Some(path) = untrusted_root {
            command.env("ARCHY_RECOVERY_CHILD_ROOT", path);
        }
        let child = command.spawn().unwrap();
        drop(command);
        Self {
            child: Some(child),
            reaped,
            _parent_channel: parent_channel,
        }
    }

    fn kill_and_wait(&mut self) -> ExitStatus {
        let child = self.child.as_mut().unwrap();
        child.kill().unwrap();
        let status = child.wait().unwrap();
        *self.reaped.borrow_mut() = Some(status);
        self.child = None;
        status
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            if let Ok(status) = child.wait() {
                *self.reaped.borrow_mut() = Some(status);
            }
        }
    }
}

fn wait_for_staging(root: &Path, writer: &mut Writer) -> PathBuf {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !root.join("child-ready").exists() {
        if let Some(status) = writer.child.as_mut().unwrap().try_wait().unwrap() {
            panic!("staging child exited before readiness: {status}");
        }
        assert!(
            Instant::now() < deadline,
            "staging child did not become ready"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(fs::read(root.join("child-ready")).unwrap(), b"staged");
    fs::read_dir(root.join(".archy/staging"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}

fn accept_first(root: &Path) -> RunReceipt {
    let project = root.join("project.json");
    fs::write(&project, b"source").unwrap();
    let store = ProjectStore::open(&project).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked
        .begin(request("accepted", 0))
        .unwrap()
        .pending()
        .unwrap();
    pending
        .write("data", b"accepted", "text/plain", "recovery-test")
        .unwrap();
    pending.commit(metadata()).unwrap()
}

fn request(key: &str, revision: u64) -> RunRequest {
    RunRequest {
        schema_version: archy_core::RUN_REQUEST_SCHEMA_VERSION.into(),
        context: MutationContext {
            idempotency_key: key.into(),
            expected_project_revision: revision,
            destination: "runs".into(),
        },
        operation: "generate".into(),
        request_bytes: b"count=1,seed=42".to_vec(),
        input_sha256: sha256_hex(b"source"),
        source_run: None,
    }
}

fn metadata() -> RunMetadata {
    RunMetadata {
        project_id: "recovery-fixture".into(),
        normalized_sha256: None,
        source_format: None,
        seed: None,
        count: None,
        parent_variant_id: None,
        algorithm_version: "recovery-test-v1".into(),
    }
}

fn accepted_inventory(root: &Path, receipt: &RunReceipt) -> BTreeMap<String, String> {
    let mut inventory = BTreeMap::new();
    let run = root.join(&receipt.path);
    for entry in fs::read_dir(&run).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        inventory.insert(
            entry.file_name().to_string_lossy().into_owned(),
            sha256_hex(&fs::read(entry.path()).unwrap()),
        );
    }
    inventory.insert(
        "revision-1".into(),
        sha256_hex(&fs::read(root.join(".archy/revisions/00000000000000000001.json")).unwrap()),
    );
    inventory
}

fn watch_parent(timeout: Duration) -> bool {
    let stdout = std::io::stdout();
    let Ok(fd) = stdout.as_fd().try_clone_to_owned() else {
        return false;
    };
    let mut channel = UnixStream::from(fd);
    if channel.set_read_timeout(Some(timeout)).is_err() {
        return false;
    }
    thread::spawn(move || {
        let _ = channel.read(&mut [0u8; 1]);
        std::process::exit(0);
    });
    true
}

#[test]
fn child_stages_until_killed() {
    let root = PathBuf::from("/proc/self/fd/0");
    if !fs::metadata(&root).is_ok_and(|metadata| metadata.is_dir())
        || !watch_parent(Duration::from_secs(30))
    {
        return;
    }
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked
        .begin(request("interrupted", 1))
        .unwrap()
        .pending()
        .unwrap();
    pending
        .write("data", b"unaccepted", "text/plain", "recovery-test")
        .unwrap();
    fs::write(root.join("child-ready"), b"staged").unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn killed_staging_writer_preserves_accepted_inventory_and_retry_applies_once() {
    let fixture = Fixture::new();
    let root = &fixture.0;
    let project = root.join("project.json");
    let first = accept_first(root);
    let before = accepted_inventory(root, &first);
    println!("accepted inventory before kill: {before:?}");
    let mut writer = Writer::spawn(root, Rc::default(), None);
    let staged = wait_for_staging(root, &mut writer);
    assert_eq!(fs::read(staged.join("data")).unwrap(), b"unaccepted");
    let signal = writer.kill_and_wait().signal();
    assert_eq!(signal, Some(9));
    println!("staging child termination signal: {signal:?}");
    let restarted = ProjectStore::open(&project).unwrap();
    let locked = restarted.lock().unwrap();
    assert_eq!(locked.revision().unwrap(), 1);
    assert_eq!(
        locked
            .read_run(staged.file_name().unwrap().to_str().unwrap())
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    assert_eq!(accepted_inventory(root, &first), before);
    println!(
        "accepted inventory after kill: {:?}",
        accepted_inventory(root, &first)
    );
    assert_eq!(
        locked.read_run(&first.run_id).unwrap().artifact("data"),
        Some(b"accepted".as_slice())
    );
    let mut pending = locked
        .begin(request("interrupted", 1))
        .unwrap()
        .pending()
        .unwrap();
    pending
        .write("data", b"after-restart", "text/plain", "recovery-test")
        .unwrap();
    let second = pending.commit(metadata()).unwrap();
    assert_eq!(second.revision, 2);
    println!("retry committed revision: {}", second.revision);
    assert_eq!(
        locked.begin(request("interrupted", 1)).unwrap().receipt(),
        Some(second.clone())
    );
    assert_eq!(locked.revision().unwrap(), 2);
    assert_eq!(
        locked.receipts().unwrap(),
        vec![first.clone(), second.clone()]
    );
    assert_eq!(
        locked.read_run(&second.run_id).unwrap().artifact("data"),
        Some(b"after-restart".as_slice())
    );
    assert_eq!(accepted_inventory(root, &first), before);
    assert_eq!(fs::read(staged.join("data")).unwrap(), b"unaccepted");
    assert_eq!(
        fs::read_dir(root.join(".archy/staging")).unwrap().count(),
        1
    );
    let error = locked.begin(request("interrupted", 2)).err().unwrap();
    assert_eq!(error.code, ErrorCode::RevisionConflict);
    drop(locked);
}

#[test]
fn inherited_path_cannot_select_the_writer_project() {
    let unrelated = Fixture::new();
    fs::write(unrelated.0.join("project.json"), b"unrelated").unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_stages_until_killed", "--nocapture"])
        .stdin(Stdio::null())
        .env("ARCHY_RECOVERY_CHILD_ROOT", &unrelated.0)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!unrelated.0.join(".archy").exists());

    let fixture = Fixture::new();
    accept_first(&fixture.0);
    let mut writer = Writer::spawn(&fixture.0, Rc::default(), Some(&unrelated.0));
    let staged = wait_for_staging(&fixture.0, &mut writer);
    assert_eq!(fs::read(staged.join("data")).unwrap(), b"unaccepted");
    assert_eq!(writer.kill_and_wait().signal(), Some(9));
    assert!(!unrelated.0.join(".archy").exists());
    assert_eq!(
        fs::read(unrelated.0.join("project.json")).unwrap(),
        b"unrelated"
    );
}

#[test]
fn parent_unwind_reaps_writer_and_removes_fixture() {
    for missing_marker in [true, false] {
        let reaped = Rc::new(RefCell::new(None));
        let mut fixture_path = None;
        let mut forced_read_failure = false;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let fixture = Fixture::new();
            fixture_path = Some(fixture.0.clone());
            accept_first(&fixture.0);
            let mut writer = Writer::spawn(&fixture.0, reaped.clone(), None);
            let staged = wait_for_staging(&fixture.0, &mut writer);
            let missing = if missing_marker {
                fixture.0.join("child-ready")
            } else {
                staged.join("data")
            };
            fs::remove_file(&missing).unwrap();
            forced_read_failure = true;
            fs::read(missing).unwrap();
        }));
        assert!(forced_read_failure);
        assert!(result.is_err());
        assert_eq!(
            reaped.borrow().as_ref().and_then(ExitStatusExt::signal),
            Some(9)
        );
        assert!(!fixture_path.unwrap().exists());
    }
}

#[test]
fn parent_for_sigkill_probe() {
    let root = PathBuf::from("/proc/self/fd/0");
    if !fs::metadata(&root).is_ok_and(|metadata| metadata.is_dir())
        || !watch_parent(Duration::from_secs(30))
    {
        return;
    }
    let owned = root.join("owned");
    fs::DirBuilder::new().mode(0o700).create(&owned).unwrap();
    let fixture = Fixture(owned);
    accept_first(&fixture.0);
    let mut writer = Writer::spawn(&fixture.0, Rc::default(), None);
    wait_for_staging(&fixture.0, &mut writer);
    fs::write(
        root.join("probe-ready"),
        writer.child.as_ref().unwrap().id().to_string(),
    )
    .unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn sigkill_of_test_parent_stops_writer_and_preserves_owned_evidence() {
    let fixture = Fixture::new();
    let root = fixture.0.clone();
    let mut parent = Writer::spawn_test(&root, Rc::default(), None, "parent_for_sigkill_probe");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !root.join("probe-ready").exists() {
        if let Some(status) = parent.child.as_mut().unwrap().try_wait().unwrap() {
            panic!("probe parent exited before readiness: {status}");
        }
        assert!(
            Instant::now() < deadline,
            "probe parent did not become ready"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let pid: u32 = fs::read_to_string(root.join("probe-ready"))
        .unwrap()
        .parse()
        .unwrap();
    let owned = root.join("owned");
    let staged = fs::read_dir(owned.join(".archy/staging"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(fs::read(staged.join("data")).unwrap(), b"unaccepted");
    let before_kill = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    assert!(!before_kill.rsplit_once(") ").unwrap().1.starts_with("Z "));
    assert_eq!(parent.kill_and_wait().signal(), Some(9));

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Ok(stat)
                if stat
                    .rsplit_once(") ")
                    .is_some_and(|(_, rest)| rest.starts_with("Z ")) =>
            {
                break;
            }
            Ok(_) => {}
            Err(e) => panic!("cannot inspect writer {pid}: {e}"),
        }
        assert!(Instant::now() < deadline, "writer survived parent SIGKILL");
        thread::sleep(Duration::from_millis(10));
    }
    let store = ProjectStore::open(&owned.join("project.json")).unwrap();
    assert_eq!(store.read_history().unwrap().revision(), 1);
    assert_eq!(fs::read(staged.join("data")).unwrap(), b"unaccepted");
    drop(store);
    drop(fixture);
    assert!(!root.exists());
}

#[test]
fn watchdog_timeout_probe() {
    let root = PathBuf::from("/proc/self/fd/0");
    if !fs::metadata(&root).is_ok_and(|metadata| metadata.is_dir())
        || !watch_parent(Duration::from_millis(100))
    {
        return;
    }
    fs::write(root.join("watchdog-ready"), b"armed").unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn watchdog_exits_when_channel_stays_open_past_timeout() {
    let fixture = Fixture::new();
    let mut writer = Writer::spawn_test(&fixture.0, Rc::default(), None, "watchdog_timeout_probe");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !fixture.0.join("watchdog-ready").exists() {
        if let Some(status) = writer.child.as_mut().unwrap().try_wait().unwrap() {
            panic!("watchdog exited before readiness: {status}");
        }
        assert!(Instant::now() < deadline, "watchdog did not become ready");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read(fixture.0.join("watchdog-ready")).unwrap(),
        b"armed"
    );
    loop {
        if let Some(status) = writer.child.as_mut().unwrap().try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "watchdog did not exit on timeout"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
