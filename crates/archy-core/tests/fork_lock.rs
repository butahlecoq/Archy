use archy_core::{
    ErrorCode, MutationContext, ProjectStore, RUN_REQUEST_SCHEMA_VERSION, RunMetadata, RunRequest,
    sha256_hex,
};
use std::{
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

unsafe extern "C" {
    fn fork() -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn unshare(flags: i32) -> i32;
    fn read(fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize;
    fn _exit(status: i32) -> !;
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!("archy-fork-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("project.json"), b"source").unwrap();
    root
}

fn request(key: &str) -> RunRequest {
    RunRequest {
        schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
        context: MutationContext {
            idempotency_key: key.into(),
            expected_project_revision: 0,
            destination: "runs".into(),
        },
        operation: "generate".into(),
        request_bytes: b"sample".to_vec(),
        input_sha256: sha256_hex(b"source"),
        source_run: None,
    }
}

fn metadata() -> RunMetadata {
    RunMetadata {
        project_id: "fixture".into(),
        normalized_sha256: None,
        source_format: None,
        seed: None,
        count: None,
        parent_variant_id: None,
        algorithm_version: "storage-test-v1".into(),
    }
}

fn wait_child(pid: i32) {
    let mut status = 0;
    assert_eq!(unsafe { waitpid(pid, &mut status, 0) }, pid);
    assert_eq!(status, 0);
}

fn child_drop_keeps_parent_lock() {
    let root = fixture();
    let project = root.join("project.json");
    let store = ProjectStore::open(&project).unwrap();
    let locked = store.lock().unwrap();
    let owner_pid = std::process::id();
    let child = unsafe { fork() };
    assert!(child >= 0);
    if child == 0 {
        let child_pid = std::process::id();
        drop(locked);
        unsafe { _exit(if child_pid != owner_pid { 0 } else { 1 }) }
    }
    wait_child(child);
    let contender = Command::new(std::env::current_exe().unwrap())
        .arg("contender")
        .arg(&project)
        .status()
        .unwrap();
    assert!(
        contender.success(),
        "separate contender acquired a live parent's lock"
    );
    drop(locked);
    assert!(store.lock().is_ok());
    fs::remove_dir_all(root).unwrap();
}

fn child_cannot_use_inherited_guard() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let mut pending = locked.begin(request("pending")).unwrap().pending().unwrap();
    pending
        .write("base", b"source", "text/plain", "test")
        .unwrap();
    let (mut parent, mut child) = UnixStream::pair().unwrap();
    let pid = unsafe { fork() };
    assert!(pid >= 0);
    if pid == 0 {
        drop(parent);
        let mut signal = [0];
        child.read_exact(&mut signal).unwrap();
        let denied_begin = matches!(locked.begin(request("child")), Err(e) if e.code == ErrorCode::PermissionDenied);
        let denied_write = matches!(pending.write("child", b"child", "text/plain", "test"), Err(e) if e.code == ErrorCode::PermissionDenied);
        let denied_commit =
            matches!(pending.commit(metadata()), Err(e) if e.code == ErrorCode::PermissionDenied);
        child
            .write_all(&[denied_begin as u8, denied_write as u8, denied_commit as u8])
            .unwrap();
        unsafe { _exit(0) }
    }
    drop(child);
    drop(pending);
    drop(locked);
    let contender = store.lock().unwrap();
    parent.write_all(b"1").unwrap();
    let mut result = [0; 3];
    parent.read_exact(&mut result).unwrap();
    wait_child(pid);
    assert_eq!(
        result,
        [1, 1, 1],
        "inherited guard began, wrote, or committed"
    );
    assert_eq!(contender.revision().unwrap(), 0);
    drop(contender);
    fs::remove_dir_all(root).unwrap();
}

fn owner_exit(project: &Path, inherited: bool) -> ! {
    let store = ProjectStore::open(project).unwrap();
    let _locked = store.lock().unwrap();
    if !inherited {
        unsafe { _exit(0) }
    }
    let child = unsafe { fork() };
    if child == 0 {
        let mut signal = 0u8;
        let received = unsafe { read(0, std::ptr::from_mut(&mut signal).cast(), 1) };
        unsafe { _exit(if received == 1 { 0 } else { 1 }) }
    }
    unsafe { _exit(if child > 0 { 0 } else { 2 }) }
}

fn process_exit_cleanup() {
    let root = fixture();
    let project = root.join("project.json");
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("owner-exit")
        .arg(&project)
        .arg("no-child")
        .status()
        .unwrap();
    assert!(status.success());
    let store = ProjectStore::open(&project).unwrap();
    assert!(store.lock().is_ok());

    let mut owner = Command::new(std::env::current_exe().unwrap())
        .arg("owner-exit")
        .arg(&project)
        .arg("child")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut child_input = owner.stdin.take().unwrap();
    let status = owner.wait().unwrap();
    let while_inherited = store.lock().err();
    child_input.write_all(b"1").unwrap();
    let mut output = Vec::new();
    owner
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    assert!(status.success());
    assert_eq!(while_inherited.unwrap().code, ErrorCode::RevisionConflict);
    assert!(store.lock().is_ok());
    fs::remove_dir_all(root).unwrap();
}

fn pid_namespace_collision() {
    let root = fixture();
    let store = ProjectStore::open(&root.join("project.json")).unwrap();
    let locked = store.lock().unwrap();
    let owner_pid = std::process::id();
    assert_eq!(owner_pid, 1);
    assert_eq!(unsafe { unshare(0x20000000) }, 0);
    let child = unsafe { fork() };
    assert!(child >= 0);
    if child == 0 {
        let child_pid = std::process::id();
        drop(locked);
        unsafe { _exit(if child_pid == owner_pid { 0 } else { 1 }) }
    }
    wait_child(child);
    let error = store
        .lock()
        .err()
        .expect("namespace child released parent's lock");
    assert_eq!(error.code, ErrorCode::RevisionConflict);
    drop(locked);
    fs::remove_dir_all(root).unwrap();
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    match args.next().as_deref().and_then(|arg| arg.to_str()) {
        Some("contender") => {
            let project = PathBuf::from(args.next().unwrap());
            let store = ProjectStore::open(&project).unwrap();
            assert_eq!(
                store.lock().err().unwrap().code,
                ErrorCode::RevisionConflict
            );
        }
        Some("owner-exit") => {
            let project = PathBuf::from(args.next().unwrap());
            owner_exit(&project, args.next().unwrap() == "child");
        }
        Some("pid-namespace") => pid_namespace_collision(),
        _ => {
            // This test executable uses a single-threaded main before each raw fork.
            child_drop_keeps_parent_lock();
            child_cannot_use_inherited_guard();
            process_exit_cleanup();
            println!("fork lock lifecycle checks passed");
        }
    }
}
