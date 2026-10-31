#![cfg(target_os = "linux")]

use archy_adapters::receive_quota_checked_scratch;
use archy_core::ErrorCode;
use rustix::fs::{self, Mode, OFlags, StatVfsMountFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{Errno, FdFlags, fcntl_getfd};
use rustix::net::{
    AddressFamily, RecvFlags, SendFlags, Shutdown, SocketFlags, SocketType, recv, send, shutdown,
    socketpair,
};
use rustix::process::getuid;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{File, create_dir, read, read_to_string, remove_dir, remove_file, write};
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

type TestResult<T> = Result<T, Box<dyn Error>>;

const SYSTEMD_RUN: &str = "/usr/bin/systemd-run";
const SYSTEMCTL: &str = "/usr/bin/systemctl";
const PYTHON: &str = "/usr/bin/python3";
const PAYLOAD: &[u8] = b"archy-rust-service-receiver\n";
const BYTE_CAP: u64 = 65_536;
const INODE_CAP: u64 = 8;

#[derive(Clone, Copy, Debug)]
enum Case {
    PrivateDirectory,
    UnsafeMode,
    RegularFile,
}

fn problem(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn wait_child(child: &mut Child, deadline: Instant) -> TestResult<std::process::ExitStatus> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(problem("child did not exit before deadline").into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn systemctl(args: &[&str]) -> TestResult<String> {
    let mut child = Command::new(SYSTEMCTL)
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = wait_child(&mut child, Instant::now() + Duration::from_secs(2));
        return Err(problem("missing systemctl stdout").into());
    };
    let nonblocking =
        fcntl_getfl(&stdout).and_then(|flags| fcntl_setfl(&stdout, flags | OFlags::NONBLOCK));
    if let Err(error) = nonblocking {
        let _ = child.kill();
        let _ = wait_child(&mut child, Instant::now() + Duration::from_secs(2));
        return Err(error.into());
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut output = Vec::new();
    let mut eof = false;
    let result = loop {
        let mut block = [0_u8; 1024];
        match stdout.read(&mut block) {
            Ok(0) => eof = true,
            Ok(count) => {
                output.extend_from_slice(&block[..count]);
                if output.len() > 8192 {
                    break Err(problem("systemctl output exceeds bound").into());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => break Err(error.into()),
        }
        match child.try_wait() {
            Ok(Some(status)) if eof => {
                if status.success() {
                    break String::from_utf8(output).map_err(Into::into);
                }
                break Err(problem(format!("systemctl exited {status}")).into());
            }
            Ok(_) => {}
            Err(error) => break Err(error.into()),
        }
        if Instant::now() >= deadline {
            break Err(problem("systemctl deadline").into());
        }
        thread::sleep(Duration::from_millis(5));
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = wait_child(&mut child, Instant::now() + Duration::from_secs(2));
    }
    result
}

fn properties(unit: &str) -> TestResult<BTreeMap<String, String>> {
    let text = systemctl(&[
        "show",
        unit,
        "-pLoadState",
        "-pActiveState",
        "-pControlGroup",
        "-pMainPID",
        "-pMemoryMax",
        "-pMemorySwapMax",
        "-pTasksMax",
        "-pKillMode",
        "-pRuntimeMaxUSec",
        "-pTimeoutStopUSec",
    ])?;
    Ok(text
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}

struct Fixture {
    root: PathBuf,
    unit: String,
    child: Option<Child>,
    channel: Option<OwnedFd>,
    finished: bool,
}

impl Fixture {
    fn start(case: Case) -> TestResult<Self> {
        let helper = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/security/systemd_rust_receiver_helper.py");
        if !helper.is_file() {
            return Err(problem("fixed service helper is missing").into());
        }
        let uuid = read_to_string("/proc/sys/kernel/random/uuid")?
            .trim()
            .to_owned();
        if uuid.len() != 36 || !uuid.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
            return Err(problem("invalid generated fixture UUID").into());
        }
        let root = PathBuf::from("/tmp").join(format!("archy-rust-receiver-{uuid}"));
        create_dir(&root)?;
        let mut fixture = Self {
            root,
            unit: format!("archy-rust-receiver-{uuid}.service"),
            child: None,
            channel: None,
            finished: false,
        };
        std::fs::set_permissions(&fixture.root, std::fs::Permissions::from_mode(0o700))?;
        let scratch = fixture.root.join("scratch");
        create_dir(&scratch)?;
        std::fs::set_permissions(&scratch, std::fs::Permissions::from_mode(0o700))?;
        write(scratch.join("host-sentinel"), b"host backing unchanged\n")?;
        write(fixture.root.join("outside"), b"outside unchanged\n")?;
        let (receiver, service) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )?;
        fixture.channel = Some(receiver);
        let mode = if matches!(case, Case::UnsafeMode) {
            "0777"
        } else {
            "0700"
        };
        let descriptor = if matches!(case, Case::RegularFile) {
            "regular"
        } else {
            "directory"
        };
        let mount = format!(
            "TemporaryFileSystem={}:size={BYTE_CAP},nr_inodes={INODE_CAP},mode={mode},nosuid,nodev,noexec",
            scratch.display()
        );
        fixture.child = Some(
            Command::new(SYSTEMD_RUN)
                .args([
                    "--user",
                    "--pipe",
                    "--wait",
                    "--collect",
                    "--service-type=exec",
                    "--expand-environment=no",
                    "--quiet",
                    &format!("--unit={}", fixture.unit),
                    &format!("--property={mount}"),
                    "--property=RuntimeMaxSec=20s",
                    "--property=MemoryMax=268435456",
                    "--property=MemorySwapMax=0",
                    "--property=TasksMax=16",
                    "--property=KillMode=control-group",
                    "--property=TimeoutStopSec=2s",
                    "--",
                    PYTHON,
                    "-I",
                    "-B",
                ])
                .arg(helper)
                .arg(&scratch)
                .arg(descriptor)
                .stdin(Stdio::from(service))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        );
        Ok(fixture)
    }

    fn channel(&self) -> &OwnedFd {
        self.channel.as_ref().expect("fixture channel")
    }

    fn cleanup_files(&self) -> TestResult<()> {
        for (path, directory) in [
            (self.root.join("scratch/host-sentinel"), false),
            (self.root.join("outside"), false),
            (self.root.join("scratch"), true),
            (self.root.clone(), true),
        ] {
            let result = if directory {
                remove_dir(path)
            } else {
                remove_file(path)
            };
            if let Err(error) = result
                && error.kind() != io::ErrorKind::NotFound
            {
                return Err(error.into());
            }
        }
        Ok(())
    }
}

fn cleanup_confirmed(unit: &str) -> bool {
    let Ok(state) = properties(unit) else {
        return false;
    };
    if state.get("ActiveState").map(String::as_str) != Some("inactive") {
        return false;
    }
    let Some(value) = state.get("ControlGroup") else {
        return false;
    };
    if value.is_empty() {
        return state.get("LoadState").map(String::as_str) == Some("not-found");
    }
    let Some(relative) = value.strip_prefix('/') else {
        return false;
    };
    let relative = Path::new(relative);
    if !relative.starts_with("user.slice")
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || relative.file_name().and_then(|v| v.to_str()) != Some(unit)
    {
        return false;
    }
    match read_to_string(
        Path::new("/sys/fs/cgroup")
            .join(relative)
            .join("cgroup.events"),
    ) {
        Ok(events) => events.lines().any(|line| line == "populated 0"),
        Err(error) => error.kind() == io::ErrorKind::NotFound,
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.channel.take();
        if !self.finished {
            let mut cleanup_safe = self.child.is_none();
            if let Some(child) = &mut self.child {
                let _ = systemctl(&["stop", &self.unit]);
                let _ = child.kill();
                let _ = wait_child(child, Instant::now() + Duration::from_secs(2));
                cleanup_safe = cleanup_confirmed(&self.unit);
            }
            if cleanup_safe {
                if let Err(error) = self.cleanup_files() {
                    eprintln!("fixture cleanup {}: {error}", self.root.display());
                }
            } else {
                eprintln!(
                    "fixture teardown uncertain: {} at {}",
                    self.unit,
                    self.root.display()
                );
            }
        }
    }
}

fn group_before_token(unit: &str) -> TestResult<PathBuf> {
    let state = properties(unit)?;
    if state.get("LoadState").map(String::as_str) != Some("loaded")
        || state.get("ActiveState").map(String::as_str) != Some("active")
        || state
            .get("MainPID")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0)
            == 0
        || state.get("MemoryMax").map(String::as_str) != Some("268435456")
        || state.get("MemorySwapMax").map(String::as_str) != Some("0")
        || state.get("TasksMax").map(String::as_str) != Some("16")
        || state.get("KillMode").map(String::as_str) != Some("control-group")
        || state.get("RuntimeMaxUSec").map(String::as_str) != Some("20s")
        || state.get("TimeoutStopUSec").map(String::as_str) != Some("2s")
    {
        return Err(problem(format!("service properties unavailable: {state:?}")).into());
    }
    let value = state
        .get("ControlGroup")
        .ok_or_else(|| problem("missing cgroup"))?;
    let relative = Path::new(
        value
            .strip_prefix('/')
            .ok_or_else(|| problem("invalid cgroup"))?,
    );
    if !relative.starts_with("user.slice")
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || relative.file_name().and_then(|v| v.to_str()) != Some(unit)
    {
        return Err(problem("unexpected cgroup path").into());
    }
    let group = Path::new("/sys/fs/cgroup").join(relative);
    let pid = state
        .get("MainPID")
        .ok_or_else(|| problem("missing service PID"))?;
    if !read_to_string(group.join("cgroup.procs"))?
        .lines()
        .any(|line| line == pid)
    {
        return Err(problem("service MainPID absent from cgroup").into());
    }
    for (name, expected) in [
        ("memory.max", "268435456"),
        ("memory.swap.max", "0"),
        ("pids.max", "16"),
    ] {
        if read_to_string(group.join(name))?.trim() != expected {
            return Err(problem(format!("effective {name} mismatch")).into());
        }
    }
    if !read_to_string(group.join("cgroup.events"))?
        .lines()
        .any(|line| line == "populated 1")
    {
        return Err(problem("service cgroup not populated before token").into());
    }
    Ok(group)
}

fn wait_ack(channel: &OwnedFd, deadline: Instant) -> TestResult<()> {
    let mut message = [0_u8; 32];
    loop {
        match recv(channel, &mut message[..], RecvFlags::DONTWAIT) {
            Ok((bytes, _)) if bytes == 5 && &message[..bytes] == b"NO-GO" => return Ok(()),
            Ok(_) => return Err(problem("missing no-token acknowledgement").into()),
            Err(Errno::AGAIN | Errno::INTR) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(problem(format!("no-token acknowledgement: {error}")).into()),
        }
    }
}

fn run_case(case: Case) -> TestResult<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut fixture = Fixture::start(case)?;
    let result = receive_quota_checked_scratch(fixture.channel(), deadline);
    let scratch = match case {
        Case::PrivateDirectory => {
            Some(result.map_err(|e| problem(format!("Rust receiver unavailable: {e}")))?)
        }
        Case::UnsafeMode | Case::RegularFile => {
            let error = result.expect_err("unsafe service descriptor accepted");
            let expected = if matches!(case, Case::UnsafeMode) {
                "private mode"
            } else {
                "owned directory"
            };
            if error.code != ErrorCode::BackendUnavailable || !error.message.contains(expected) {
                return Err(problem(format!("unexpected refusal: {error}")).into());
            }
            None
        }
    };
    let group = group_before_token(&fixture.unit)?;
    let mut details = String::new();
    if let Some(scratch) = &scratch {
        let metadata = fs::fstat(scratch.as_file())?;
        let filesystem = fs::fstatfs(scratch.as_file())?;
        let stats = fs::fstatvfs(scratch.as_file())?;
        if !fs::FileType::from_raw_mode(metadata.st_mode).is_dir()
            || metadata.st_uid != getuid().as_raw()
            || metadata.st_mode & 0o7777 != 0o700
            || filesystem.f_type as u64 != 0x0102_1994
            || scratch.byte_capacity() != BYTE_CAP
            || scratch.inode_capacity() != INODE_CAP
            || stats.f_blocks.checked_mul(stats.f_frsize) != Some(BYTE_CAP)
            || stats.f_files != INODE_CAP
            || !stats.f_flag.contains(
                StatVfsMountFlags::NOSUID | StatVfsMountFlags::NODEV | StatVfsMountFlags::NOEXEC,
            )
            || !fcntl_getfd(scratch.as_file())?.contains(FdFlags::CLOEXEC)
        {
            return Err(problem("accepted FD metadata or quota mismatch").into());
        }
        details = format!(
            "uid={} mode={:04o} tmpfs bytes={} inodes={} flags=nosuid,nodev,noexec cloexec",
            metadata.st_uid,
            metadata.st_mode & 0o7777,
            scratch.byte_capacity(),
            scratch.inode_capacity()
        );
        if send(
            fixture.channel(),
            b"GO",
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
        )? != 2
        {
            return Err(problem("short start token").into());
        }
    } else {
        shutdown(fixture.channel(), Shutdown::Write)?;
        wait_ack(fixture.channel(), deadline)?;
    }
    let status = wait_child(fixture.child.as_mut().expect("service child"), deadline)?;
    if !status.success() {
        return Err(problem(format!(
            "systemd-run unavailable or service failed: {status}"
        ))
        .into());
    }
    let state = properties(&fixture.unit)?;
    if state.get("ActiveState").map(String::as_str) != Some("inactive") {
        return Err(problem(format!("service not inactive: {state:?}")).into());
    }
    let cgroup_exit = match read_to_string(group.join("cgroup.events")) {
        Ok(events) if events.lines().any(|line| line == "populated 0") => "empty",
        Err(error) if error.kind() == io::ErrorKind::NotFound => "absent",
        Ok(_) => return Err(problem("service cgroup still populated").into()),
        Err(error) => return Err(error.into()),
    };
    if state.get("LoadState").map(String::as_str) != Some("not-found") {
        return Err(problem(format!("transient unit not collected: {state:?}")).into());
    }
    if let Some(scratch) = scratch {
        let fd = fs::openat(
            scratch.as_file(),
            "payload.bin",
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?;
        let mut bytes = Vec::new();
        File::from(fd).take(128).read_to_end(&mut bytes)?;
        if bytes != PAYLOAD {
            return Err(problem("retained FD payload differs").into());
        }
    }
    if read(fixture.root.join("scratch/host-sentinel"))? != b"host backing unchanged\n"
        || read(fixture.root.join("outside"))? != b"outside unchanged\n"
        || fixture.root.join("scratch/payload.bin").exists()
    {
        return Err(problem("host sentinel changed").into());
    }
    fixture.channel.take();
    fixture.cleanup_files()?;
    fixture.finished = true;
    println!(
        "{case:?}: Rust receiver {}, {details} service inactive, unit collected, cgroup {cgroup_exit}, host unchanged",
        if matches!(case, Case::PrivateDirectory) {
            "accepted"
        } else {
            "refused before GO"
        }
    );
    Ok(())
}

#[test]
#[ignore = "requires a user systemd manager with tmpfs namespaces and cgroup v2 limits"]
fn transient_user_service_passes_real_fd_to_public_receiver() -> TestResult<()> {
    let baseline = std::fs::read_dir("/proc/self/fd")?.count();
    for case in [Case::PrivateDirectory, Case::UnsafeMode, Case::RegularFile] {
        run_case(case)?;
    }
    if std::fs::read_dir("/proc/self/fd")?.count() != baseline {
        return Err(problem("test process descriptor leak").into());
    }
    Ok(())
}
