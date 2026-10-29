//! Fixed transient user-service control boundary. The packaged gate is a separate prerequisite.

use crate::{QuotaCheckedScratch, receive_quota_checked_scratch};
use archy_core::{ArchyError, ErrorCode};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::net::{AddressFamily, SendFlags, SocketFlags, SocketType, send, socketpair};
use rustix::process::getuid;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, DirBuilder, File};
use std::io::Read;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const GATE: &str = "/usr/lib/archy/archy-service-gate";
const SYSTEMD_RUN: &str = "/usr/bin/systemd-run";
const SYSTEMCTL: &str = "/usr/bin/systemctl";
const MEMORY: u64 = 2 * 1024 * 1024 * 1024;
const TASKS: u64 = 64;
const SCRATCH_BYTES: u64 = 128 * 1024 * 1024;
const SCRATCH_INODES: u64 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Blender,
    Ifc,
}

impl BackendKind {
    fn gate_arg(self) -> &'static str {
        match self {
            Self::Blender => "blender",
            Self::Ifc => "ifc",
        }
    }
}

fn unavailable(reason: impl Into<String>) -> ArchyError {
    ArchyError::new(ErrorCode::BackendUnavailable, "service", reason)
}

fn trusted_binary(path: &str) -> Result<(), ArchyError> {
    let target = Path::new(path);
    if fs::canonicalize(target).map_err(|e| unavailable(format!("{path}: {e}")))? != target {
        return Err(unavailable(format!("{path}: noncanonical executable path")));
    }
    for ancestor in target.ancestors().filter(|p| p != &Path::new("")) {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|e| unavailable(format!("{}: {e}", ancestor.display())))?;
        if metadata.file_type().is_symlink() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0
        {
            return Err(unavailable(format!(
                "{}: executable path is mutable",
                ancestor.display()
            )));
        }
    }
    let metadata = fs::metadata(target).map_err(|e| unavailable(format!("{path}: {e}")))?;
    if !metadata.is_file() || metadata.mode() & 0o111 == 0 {
        return Err(unavailable(format!("{path}: fixed executable unavailable")));
    }
    Ok(())
}

fn user_runtime_dir() -> Result<PathBuf, ArchyError> {
    let uid = getuid().as_raw();
    let path = PathBuf::from(format!("/run/user/{uid}"));
    let metadata = fs::symlink_metadata(&path)
        .map_err(|e| unavailable(format!("user runtime directory: {e}")))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(unavailable(
            "user runtime directory is not private and owned",
        ));
    }
    Ok(path)
}

fn generated_unit() -> Result<(String, PathBuf), ArchyError> {
    let uuid = fs::read_to_string("/proc/sys/kernel/random/uuid")
        .map_err(|e| unavailable(format!("kernel job identity: {e}")))?;
    let uuid = uuid.trim();
    if uuid.len() != 36
        || !uuid.bytes().enumerate().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
    {
        return Err(unavailable("kernel job identity malformed"));
    }
    let id = uuid.replace('-', "");
    let unit = format!("archy-job-{id}.service");
    let root = PathBuf::from(format!("/tmp/archy-job-{id}"));
    Ok((unit, root))
}

fn read_bounded(path: &Path, cap: usize) -> Result<String, ArchyError> {
    let file = File::open(path).map_err(|e| unavailable(format!("{}: {e}", path.display())))?;
    let mut data = Vec::new();
    file.take((cap + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|e| unavailable(format!("{}: {e}", path.display())))?;
    if data.len() > cap {
        return Err(unavailable(format!(
            "{}: evidence exceeds bound",
            path.display()
        )));
    }
    String::from_utf8(data).map_err(|_| unavailable(format!("{}: invalid UTF-8", path.display())))
}

fn command_output(command: &str, args: &[&str], deadline: Instant) -> Result<String, ArchyError> {
    let runtime_dir = user_runtime_dir()?;
    let mut child = Command::new(command)
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| unavailable(format!("{command} launch: {e}")))?;
    let result = (|| {
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| unavailable("manager output pipe missing"))?;
        let flags =
            fcntl_getfl(&stdout).map_err(|e| unavailable(format!("manager pipe flags: {e}")))?;
        fcntl_setfl(&stdout, flags | OFlags::NONBLOCK)
            .map_err(|e| unavailable(format!("manager nonblocking pipe: {e}")))?;
        let mut bytes = Vec::new();
        let mut eof = false;
        loop {
            let mut buffer = [0_u8; 1024];
            match stdout.read(&mut buffer) {
                Ok(0) => eof = true,
                Ok(n) => {
                    bytes.extend_from_slice(&buffer[..n]);
                    if bytes.len() > 8192 {
                        return Err(unavailable("manager output exceeds 8192 bytes"));
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(unavailable(format!("manager output read: {e}"))),
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|e| unavailable(format!("manager wait: {e}")))?
                && eof
            {
                if !status.success() {
                    return Err(unavailable(format!("manager command exited {status}")));
                }
                return String::from_utf8(bytes)
                    .map_err(|_| unavailable("manager output invalid UTF-8"));
            }
            if Instant::now() >= deadline {
                return Err(unavailable("manager command deadline"));
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let end = Instant::now() + Duration::from_secs(1);
        while Instant::now() < end {
            if child.try_wait().ok().flatten().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    result
}

fn properties(unit: &str) -> Result<BTreeMap<String, String>, ArchyError> {
    let result = command_output(
        SYSTEMCTL,
        &[
            "--user",
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
            "-pSendSIGKILL",
            "-pLimitCORE",
        ],
        Instant::now() + Duration::from_secs(3),
    )?;
    let mut values = BTreeMap::new();
    for line in result.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| unavailable("malformed manager property"))?;
        if values.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(unavailable("duplicate manager property"));
        }
    }
    Ok(values)
}

fn property<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, ArchyError> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| unavailable(format!("missing {key} property")))
}

fn cgroup_path(unit: &str, value: &str) -> Result<PathBuf, ArchyError> {
    let path = Path::new(value);
    if !value.starts_with("/user.slice/")
        || path.file_name().and_then(|x| x.to_str()) != Some(unit)
        || path
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(unavailable("unsafe or mismatched service cgroup path"));
    }
    Ok(Path::new("/sys/fs/cgroup").join(value.trim_start_matches('/')))
}

fn verify_properties(values: &BTreeMap<String, String>) -> Result<u32, ArchyError> {
    for (key, expected) in [
        ("LoadState", "loaded"),
        ("ActiveState", "active"),
        ("MemoryMax", "2147483648"),
        ("MemorySwapMax", "0"),
        ("TasksMax", "64"),
        ("KillMode", "control-group"),
        ("RuntimeMaxUSec", "3min"),
        ("TimeoutStopUSec", "2s"),
        ("SendSIGKILL", "yes"),
        ("LimitCORE", "0"),
    ] {
        if property(values, key)? != expected {
            return Err(unavailable(format!("{key} effective property mismatch")));
        }
    }
    let pid: u32 = property(values, "MainPID")?
        .parse()
        .map_err(|_| unavailable("invalid service MainPID"))?;
    if pid == 0 {
        return Err(unavailable("service MainPID is zero"));
    }
    Ok(pid)
}

fn socket_matches_service(expected: (u64, u64), actual: (u64, u64)) -> bool {
    expected == actual
}

fn verify_active(
    unit: &str,
    service_socket: (u64, u64),
    observed_group: &mut PathBuf,
) -> Result<u32, ArchyError> {
    let values = properties(unit)?;
    let group = property(&values, "ControlGroup")?;
    let path = cgroup_path(unit, group)?;
    *observed_group = path.clone();
    let pid = verify_properties(&values)?;
    let member = read_bounded(Path::new(&format!("/proc/{pid}/cgroup")), 4096)?;
    if member.trim() != format!("0::{group}") {
        return Err(unavailable("service MainPID cgroup mismatch"));
    }
    let gate = fs::metadata(GATE).map_err(|e| unavailable(format!("gate identity: {e}")))?;
    let running = fs::metadata(format!("/proc/{pid}/exe"))
        .map_err(|e| unavailable(format!("running gate identity: {e}")))?;
    if (gate.dev(), gate.ino()) != (running.dev(), running.ino()) {
        return Err(unavailable("service MainPID executable mismatch"));
    }
    let stdin = fs::metadata(format!("/proc/{pid}/fd/0"))
        .map_err(|e| unavailable(format!("service control fd: {e}")))?;
    if !socket_matches_service(service_socket, (stdin.dev(), stdin.ino())) {
        return Err(unavailable("service control socket mismatch"));
    }
    for (file, expected) in [
        ("memory.max", "2147483648"),
        ("memory.swap.max", "0"),
        ("pids.max", "64"),
    ] {
        if read_bounded(&path.join(file), 128)?.trim() != expected {
            return Err(unavailable(format!("effective {file} mismatch")));
        }
    }
    let cpu = read_bounded(&path.join("cpu.max"), 128)?;
    let mut parts = cpu.split_whitespace();
    let quota: u64 = parts
        .next()
        .unwrap_or("")
        .parse()
        .map_err(|_| unavailable("cpu.max quota invalid"))?;
    let period: u64 = parts
        .next()
        .unwrap_or("")
        .parse()
        .map_err(|_| unavailable("cpu.max period invalid"))?;
    if quota == 0 || quota != period || parts.next().is_some() {
        return Err(unavailable("effective CPU quota mismatch"));
    }
    if !cgroup_populated(&read_bounded(&path.join("cgroup.events"), 1024)?)? {
        return Err(unavailable("service cgroup is not populated"));
    }
    Ok(pid)
}

fn verify_scratch_mount(
    pid: u32,
    scratch_path: &Path,
    scratch: &QuotaCheckedScratch,
) -> Result<(), ArchyError> {
    let service_path = PathBuf::from(format!("/proc/{pid}/root")).join(
        scratch_path
            .strip_prefix("/")
            .map_err(|_| unavailable("scratch mount path is not absolute"))?,
    );
    let actual = fs::metadata(&service_path)
        .map_err(|e| unavailable(format!("service scratch mount: {e}")))?;
    let received = fs::metadata(format!("/proc/self/fd/{}", scratch.as_file().as_raw_fd()))
        .map_err(|e| unavailable(format!("received scratch identity: {e}")))?;
    if (actual.dev(), actual.ino()) != (received.dev(), received.ino()) {
        return Err(unavailable("received scratch is not service mount"));
    }
    let mut entries = fs::read_dir(format!("/proc/self/fd/{}", scratch.as_file().as_raw_fd()))
        .map_err(|e| unavailable(format!("scratch inventory: {e}")))?;
    if entries.next().is_some() {
        return Err(unavailable("scratch is not empty before start token"));
    }
    Ok(())
}

fn wait_child(
    child: &mut Child,
    deadline: Instant,
) -> Result<std::process::ExitStatus, ArchyError> {
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| unavailable(format!("systemd-run wait: {e}")))?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(unavailable("systemd-run did not exit before deadline"));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn cgroup_populated(events: &str) -> Result<bool, ArchyError> {
    if !events.ends_with('\n') {
        return Err(unavailable(
            "cgroup.events evidence is not newline terminated",
        ));
    }
    let mut keys = BTreeSet::new();
    let mut populated = None;
    for line in events.split_terminator('\n') {
        let (key, value) = line
            .split_once(' ')
            .ok_or_else(|| unavailable("malformed cgroup.events record"))?;
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_graphic())
            || value.is_empty()
            || !value.bytes().all(|b| b.is_ascii_graphic())
            || !keys.insert(key)
        {
            return Err(unavailable("malformed or duplicate cgroup.events record"));
        }
        if matches!(key, "populated" | "frozen") && !matches!(value, "0" | "1") {
            return Err(unavailable("invalid cgroup.events state"));
        }
        if key == "populated" {
            populated = Some(value == "1");
        }
    }
    populated.ok_or_else(|| unavailable("cgroup.events missing populated state"))
}

fn cgroup_stopped(group: &Path) -> Result<bool, ArchyError> {
    match File::open(group.join("cgroup.events")) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(1025)
                .read_to_end(&mut bytes)
                .map_err(|e| unavailable(format!("{}: {e}", group.display())))?;
            if bytes.len() > 1024 {
                return Err(unavailable("cgroup.events evidence exceeds bound"));
            }
            let events = std::str::from_utf8(&bytes)
                .map_err(|_| unavailable("cgroup.events evidence invalid UTF-8"))?;
            Ok(!cgroup_populated(events)?)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match fs::symlink_metadata(group) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Ok(_) => Err(unavailable(format!(
                "{}: cgroup.events missing",
                group.display()
            ))),
            Err(e) => Err(unavailable(format!("{}: {e}", group.display()))),
        },
        Err(e) => Err(unavailable(format!("{}: {e}", group.display()))),
    }
}

/// A gated, fixed-policy transient service. It grants no artifact publication authority.
/// The packaged gate must independently enforce its worker and sandbox policy.
pub struct ServiceSession {
    unit: String,
    root: PathBuf,
    group: PathBuf,
    child: Option<Child>,
    channel: Option<OwnedFd>,
    scratch: Option<QuotaCheckedScratch>,
    verified_stopped: bool,
}

impl ServiceSession {
    pub fn start(backend: BackendKind) -> Result<Self, ArchyError> {
        for binary in [SYSTEMD_RUN, SYSTEMCTL, GATE] {
            trusted_binary(binary)?;
        }
        let runtime_dir = user_runtime_dir()?;
        let (unit, root) = generated_unit()?;
        DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(|e| unavailable(format!("private job root: {e}")))?;
        let mount = root.join("scratch");
        if let Err(e) = DirBuilder::new().mode(0o700).create(&mount) {
            let _ = fs::remove_dir(&root);
            return Err(unavailable(format!("private scratch mountpoint: {e}")));
        }
        let (receiver, sender) = match socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        ) {
            Ok(pair) => pair,
            Err(e) => {
                let _ = fs::remove_dir(&mount);
                let _ = fs::remove_dir(&root);
                return Err(unavailable(format!("control socket: {e}")));
            }
        };
        let service_fd = match rustix::fs::fstat(&sender) {
            Ok(stat) => stat,
            Err(e) => {
                let _ = fs::remove_dir(&mount);
                let _ = fs::remove_dir(&root);
                return Err(unavailable(format!("control peer identity: {e}")));
            }
        };
        let service_socket = (service_fd.st_dev, service_fd.st_ino);
        let args = launch_argv(&unit, &mount, backend);
        let child = Command::new(SYSTEMD_RUN)
            .args(args)
            .env_clear()
            .env("LC_ALL", "C")
            .env("XDG_RUNTIME_DIR", runtime_dir)
            .current_dir("/")
            .stdin(Stdio::from(sender))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let child = match child {
            Ok(child) => child,
            Err(e) => {
                let _ = fs::remove_dir(&mount);
                let _ = fs::remove_dir(&root);
                return Err(unavailable(format!("transient service launch: {e}")));
            }
        };
        let mut session = Self {
            unit,
            root,
            group: PathBuf::new(),
            child: Some(child),
            channel: Some(receiver),
            scratch: None,
            verified_stopped: false,
        };
        let ready = (|| {
            let channel = session
                .channel
                .as_ref()
                .ok_or_else(|| unavailable("control socket missing"))?;
            let scratch =
                receive_quota_checked_scratch(channel, Instant::now() + Duration::from_secs(8))?;
            let pid = verify_active(&session.unit, service_socket, &mut session.group)?;
            verify_scratch_mount(pid, &mount, &scratch)?;
            session.scratch = Some(scratch);
            Ok::<(), ArchyError>(())
        })();
        if let Err(error) = ready {
            let cleanup = session.stop();
            return Err(match cleanup {
                Ok(()) => error,
                Err(cleanup_error) => unavailable(format!(
                    "{}; uncertain teardown of {}: {}",
                    error.message, session.unit, cleanup_error.message,
                )),
            });
        }
        Ok(session)
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Releases the fixed gate only after identity, limits, and scratch checks succeeded.
    pub fn release(&mut self) -> Result<(), ArchyError> {
        let channel = self
            .channel
            .take()
            .ok_or_else(|| unavailable("start token already sent"))?;
        let sent = send(&channel, b"GO", SendFlags::empty())
            .map_err(|e| unavailable(format!("start token: {e}")))?;
        if sent != 2 {
            return Err(unavailable("short start token"));
        }
        Ok(())
    }

    /// Stops the entire unit and requires inactive/empty-cgroup evidence.
    pub fn stop(&mut self) -> Result<(), ArchyError> {
        if self.verified_stopped {
            return Ok(());
        }
        self.channel.take();
        let stop = command_output(
            SYSTEMCTL,
            &["--user", "stop", &self.unit],
            Instant::now() + Duration::from_secs(3),
        );
        let started_waiting = Instant::now();
        let mut escalated = stop.is_err();
        if escalated {
            let _ = command_output(
                SYSTEMCTL,
                &[
                    "--user",
                    "kill",
                    "--kill-whom=all",
                    "--signal=SIGKILL",
                    &self.unit,
                ],
                Instant::now() + Duration::from_secs(3),
            );
        }
        let deadline = started_waiting + Duration::from_secs(4);
        loop {
            let state = properties(&self.unit)?;
            let active_state = property(&state, "ActiveState")?;
            let group = if self.group.as_os_str().is_empty() {
                let value = property(&state, "ControlGroup")?;
                if value.is_empty() {
                    return Err(unavailable(format!(
                        "unit {} has no observed cgroup path; teardown unverified",
                        self.unit
                    )));
                }
                let group = cgroup_path(&self.unit, value)?;
                self.group = group.clone();
                group
            } else {
                self.group.clone()
            };
            if active_state == "inactive" && cgroup_stopped(&group)? {
                break;
            }
            if !escalated && started_waiting.elapsed() >= Duration::from_secs(2) {
                let _ = command_output(
                    SYSTEMCTL,
                    &[
                        "--user",
                        "kill",
                        "--kill-whom=all",
                        "--signal=SIGKILL",
                        &self.unit,
                    ],
                    Instant::now() + Duration::from_secs(3),
                );
                escalated = true;
            }
            if Instant::now() >= deadline {
                return Err(unavailable(format!(
                    "unit {} teardown unverified",
                    self.unit
                )));
            }
            thread::sleep(Duration::from_millis(20));
        }
        if let Some(child) = self.child.as_mut()
            && wait_child(child, Instant::now() + Duration::from_secs(2)).is_err()
        {
            let _ = child.kill();
            wait_child(child, Instant::now() + Duration::from_secs(1))?;
        }
        self.child.take();
        self.verified_stopped = true;
        let _ = fs::remove_dir(self.root.join("scratch"));
        let _ = fs::remove_dir(&self.root);
        Ok(())
    }

    /// Returns the retained scratch only after the unit and wrapper have stopped.
    pub fn finish(mut self, cancel: &AtomicBool) -> Result<QuotaCheckedScratch, ArchyError> {
        if self.channel.is_some() {
            if let Err(error) = self.stop() {
                return Err(unavailable(format!(
                    "unit {} teardown uncertain: {}",
                    self.unit, error.message
                )));
            }
            return Err(unavailable("service gate was not released"));
        }
        let deadline = Instant::now() + Duration::from_secs(180);
        let status = loop {
            if cancel.load(Ordering::Relaxed) {
                break Err(ArchyError::new(
                    ErrorCode::BackendTimeout,
                    "service",
                    "worker cancelled",
                ));
            }
            let Some(child) = self.child.as_mut() else {
                break Err(unavailable("systemd-run child missing"));
            };
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {}
                Err(e) => break Err(unavailable(format!("systemd-run wait: {e}"))),
            }
            if Instant::now() >= deadline {
                break Err(ArchyError::new(
                    ErrorCode::BackendTimeout,
                    "service",
                    "worker wall deadline",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        };
        let teardown = self.stop();
        if let Err(error) = teardown {
            return Err(unavailable(format!(
                "unit {} teardown uncertain: {}",
                self.unit, error.message
            )));
        }
        if !status?.success() {
            return Err(unavailable("worker service exited unsuccessfully"));
        }
        self.scratch
            .take()
            .ok_or_else(|| unavailable("retained scratch missing"))
    }
}

impl Drop for ServiceSession {
    fn drop(&mut self) {
        if !self.verified_stopped {
            let _ = self.stop();
        }
    }
}

fn launch_argv(unit: &str, scratch: &Path, backend: BackendKind) -> Vec<String> {
    let mount = format!(
        "TemporaryFileSystem={}:size={SCRATCH_BYTES},nr_inodes={SCRATCH_INODES},mode=0700,nosuid,nodev,noexec",
        scratch.display()
    );
    vec![
        "--user".into(),
        "--pipe".into(),
        "--wait".into(),
        "--collect".into(),
        "--service-type=exec".into(),
        "--expand-environment=no".into(),
        "--quiet".into(),
        format!("--unit={unit}"),
        format!("--property={mount}"),
        "--property=RuntimeMaxSec=180s".into(),
        format!("--property=MemoryMax={MEMORY}"),
        "--property=MemorySwapMax=0".into(),
        "--property=CPUQuota=100%".into(),
        format!("--property=TasksMax={TASKS}"),
        "--property=KillMode=control-group".into(),
        "--property=TimeoutStopSec=2s".into(),
        "--property=SendSIGKILL=yes".into(),
        "--property=LimitCORE=0".into(),
        "--".into(),
        GATE.into(),
        format!("--backend={}", backend.gate_arg()),
        format!("--scratch={}", scratch.display()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_is_fixed_policy_without_shell_or_caller_argv() {
        let args = launch_argv(
            "archy-job-0123456789abcdef.service",
            Path::new("/tmp/archy-job-0123456789abcdef/scratch"),
            BackendKind::Blender,
        );
        assert_eq!(args[0], "--user");
        assert!(args.contains(&"--property=CPUQuota=100%".to_owned()));
        assert!(args.contains(&"--property=KillMode=control-group".to_owned()));
        assert!(args.contains(&GATE.to_owned()));
        assert_eq!(
            args.last().unwrap(),
            "--scratch=/tmp/archy-job-0123456789abcdef/scratch"
        );
        assert!(!args.iter().any(|arg| arg == "/bin/sh" || arg == "-c"));
    }

    #[test]
    fn rejects_forged_or_unenforced_service_properties() {
        let mut values = BTreeMap::from([
            ("LoadState".into(), "loaded".into()),
            ("ActiveState".into(), "active".into()),
            ("MemoryMax".into(), "2147483648".into()),
            ("MemorySwapMax".into(), "0".into()),
            ("TasksMax".into(), "64".into()),
            ("KillMode".into(), "control-group".into()),
            ("RuntimeMaxUSec".into(), "3min".into()),
            ("TimeoutStopUSec".into(), "2s".into()),
            ("SendSIGKILL".into(), "yes".into()),
            ("LimitCORE".into(), "0".into()),
            ("MainPID".into(), "123".into()),
        ]);
        assert_eq!(verify_properties(&values).unwrap(), 123);
        values.insert("MemoryMax".into(), "infinity".into());
        assert_eq!(
            verify_properties(&values).unwrap_err().code,
            ErrorCode::BackendUnavailable
        );
        values.insert("MemoryMax".into(), "2147483648".into());
        values.insert("MainPID".into(), "0".into());
        assert!(verify_properties(&values).is_err());
    }

    #[test]
    fn cgroup_path_must_name_exact_unit_without_traversal() {
        let unit = "archy-job-0123456789abcdef.service";
        assert!(
            cgroup_path(
                unit,
                &format!("/user.slice/user-1000.slice/user@1000.service/app.slice/{unit}")
            )
            .is_ok()
        );
        for path in [
            "/user.slice/../archy-job-0123456789abcdef.service",
            "/user.slice/other.service",
            "/system.slice/archy-job-0123456789abcdef.service",
        ] {
            assert!(cgroup_path(unit, path).is_err());
        }
    }

    #[test]
    fn missing_packaged_gate_fails_closed_before_service_launch() {
        if !Path::new(GATE).exists() {
            let error = ServiceSession::start(BackendKind::Ifc).err().unwrap();
            assert_eq!(error.code, ErrorCode::BackendUnavailable);
        }
    }

    #[test]
    fn cgroup_proof_requires_empty_events_or_confirmed_absence() {
        let id = fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
        let parent = std::env::temp_dir().join(format!("archy-a09-test-{}", id.trim()));
        let group = parent.join("unit.service");
        fs::create_dir(&parent).unwrap();
        assert!(cgroup_stopped(&group).unwrap());
        fs::create_dir(&group).unwrap();
        assert!(cgroup_stopped(&group).is_err());
        fs::write(group.join("cgroup.events"), "populated 1\n").unwrap();
        assert!(!cgroup_stopped(&group).unwrap());
        fs::write(group.join("cgroup.events"), "populated 0\n").unwrap();
        assert!(cgroup_stopped(&group).unwrap());
        fs::remove_file(group.join("cgroup.events")).unwrap();
        assert!(cgroup_stopped(&group).is_err());
        fs::remove_dir(&group).unwrap();
        assert!(cgroup_stopped(&group).unwrap());
        fs::remove_dir_all(&parent).unwrap();
    }

    #[test]
    fn malformed_cgroup_events_cannot_prove_teardown() {
        let id = fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
        let parent = std::env::temp_dir().join(format!("archy-a09-test-{}", id.trim()));
        let group = parent.join("unit.service");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&group).unwrap();
        let events = group.join("cgroup.events");
        for content in [
            "populated 0\npopulated 1\n",
            "populated 0\nnot-an-event\n",
            "populated 0\nfrozen 2\n",
            "populated 0\nfuture_field 3 4\n",
            "frozen 0\n",
            "populated 0\nfuture_field 3\nfuture_field 4\n",
            "populated 0\nfrozen 0",
        ] {
            fs::write(&events, content).unwrap();
            assert!(cgroup_stopped(&group).is_err(), "{content:?}");
        }
        fs::write(&events, "populated 0\nfrozen 0\nfuture-field 3\n").unwrap();
        assert!(cgroup_stopped(&group).unwrap());
        fs::remove_dir_all(&parent).unwrap();
    }

    #[test]
    fn denied_cgroup_lookup_never_verifies_teardown() {
        use std::os::unix::fs::PermissionsExt;
        if getuid().as_raw() == 0 {
            return;
        }
        let id = fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
        let parent = std::env::temp_dir().join(format!("archy-a09-test-{}", id.trim()));
        let group = parent.join("unit.service");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&group).unwrap();
        fs::write(group.join("cgroup.events"), "populated 1\n").unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o000)).unwrap();
        let read_error = File::open(group.join("cgroup.events")).unwrap_err();
        let result = cgroup_stopped(&group);
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(&parent).unwrap();
        assert_eq!(read_error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(
            result.is_err(),
            "PermissionDenied cannot prove group removal"
        );
    }

    #[test]
    fn control_identity_uses_service_socket_endpoint() {
        let (parent, service) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let parent_stat = rustix::fs::fstat(&parent).unwrap();
        let service_stat = rustix::fs::fstat(&service).unwrap();
        let expected = (service_stat.st_dev, service_stat.st_ino);
        assert!(!socket_matches_service(
            expected,
            (parent_stat.st_dev, parent_stat.st_ino)
        ));
        assert!(socket_matches_service(expected, expected));
    }
}
