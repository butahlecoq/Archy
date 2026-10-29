use rustix::fs::{Mode, OFlags};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const LIMIT: u64 = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(2);
const COLLECTION_TIMEOUT: Duration = Duration::from_secs(15);
const MANIFEST: &str = include_str!("../../../packaging/arch/compatibility.json");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe<T> {
    Found(T),
    Missing,
    Unavailable(String),
    Deferred(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckStatus {
    Available,
    Missing,
    Unsupported,
    Unavailable,
    Deferred,
}
impl CheckStatus {
    fn name(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Missing => "missing",
            Self::Unsupported => "unsupported",
            Self::Unavailable => "unavailable",
            Self::Deferred => "deferred",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HostObservation {
    pub os_id: Probe<String>,
    pub kernel: Probe<String>,
    pub architecture: String,
    pub wsl: bool,
    pub omarchy: Probe<String>,
    pub omarchy_dev: Probe<String>,
    pub custom_omarchy_path: bool,
    pub bubblewrap: Probe<String>,
    pub shell: Probe<String>,
    pub cgroup_controllers: Probe<String>,
    pub user_manager: Probe<String>,
    pub blender_version: Probe<String>,
    pub ifc_version: Probe<String>,
}

#[derive(Clone, Debug)]
pub struct CompatibilityPolicy {
    pub omarchy_version: String,
    pub backend_evidence: String,
    pub blender_proposal: String,
    pub ifc_proposal: String,
}
impl CompatibilityPolicy {
    pub fn checked_in() -> Result<Self, String> {
        let raw: Value = serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
        if raw["schema_version"] != "1"
            || raw["backends"]["blender"]["runtime_supported"] != false
            || raw["backends"]["ifc"]["runtime_supported"] != false
            || raw["backends"]["blender"]["evidence_status"] != "unverified"
            || raw["backends"]["ifc"]["evidence_status"] != "unverified"
        {
            return Err("compatibility manifest is not reviewed for this doctor".into());
        }
        Ok(Self {
            omarchy_version: raw["omarchy"]["approved_upstream_version"]
                .as_str()
                .ok_or("missing Omarchy version")?
                .into(),
            backend_evidence: "unverified".into(),
            blender_proposal: raw["backends"]["blender"]["proposed_version"]
                .as_str()
                .ok_or("missing Blender proposal")?
                .into(),
            ifc_proposal: raw["backends"]["ifc"]["proposed_ifcopenshell"]
                .as_str()
                .ok_or("missing IFC proposal")?
                .into(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct CheckResult {
    pub id: &'static str,
    pub required_for: Vec<&'static str>,
    pub status: CheckStatus,
    pub observed: Option<String>,
    pub expected: Option<String>,
    pub evidence: Vec<&'static str>,
    pub reason: String,
    pub remedy: String,
}
impl CheckResult {
    fn json(&self) -> Value {
        json!({
            "id": self.id, "required_for": self.required_for, "status": self.status.name(),
            "observed": self.observed, "expected": self.expected, "evidence": self.evidence,
            "reason": self.reason, "remedy": self.remedy,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DoctorReport {
    pub host: Value,
    pub checks: Vec<CheckResult>,
    pub ready: bool,
    pub supported_baseline: String,
    pub upstream_version: Option<String>,
    pub raw_package_identity: Option<String>,
}
impl DoctorReport {
    pub fn exit_code(&self) -> u8 {
        if self.ready { 0 } else { 1 }
    }
    pub fn json(&self) -> Value {
        let capability = |name: &str| {
            let required: Vec<&CheckResult> = self
                .checks
                .iter()
                .filter(|c| c.required_for.contains(&name))
                .collect();
            let ready =
                !required.is_empty() && required.iter().all(|c| c.status == CheckStatus::Available);
            json!({"ready": ready, "required_checks": required.iter().map(|c| c.id).collect::<Vec<_>>()})
        };
        json!({
            "schema_version": "1", "host": self.host, "ready": self.ready,
            "platform": {"supported_baseline": self.supported_baseline, "compatibility": self.checks.iter().find(|c| c.id == "platform.omarchy").map(|c| c.status.name()),
                "raw_package_identity": self.raw_package_identity,
                "upstream_version": self.upstream_version,
                "development_override": self.checks.iter().any(|c| c.id == "platform.development-path" && c.status == CheckStatus::Unsupported)},
            "checks": self.checks.iter().map(CheckResult::json).collect::<Vec<_>>(),
            "capabilities": {"core": capability("core"), "blender": capability("blender"),
                "ifc": capability("ifc"), "containment": capability("containment"), "desktop": capability("desktop")},
            "target_acceptance": "deferred",
            "target_gates": [
                {"id":"OM-A21-01","status":"deferred","reason":"actual Omarchy package transaction and inventory not performed"},
                {"id":"OM-A21-02","status":"deferred","reason":"installed doctor and independent nonmutation evidence not performed"},
                {"id":"OM-A21-03","status":"deferred","reason":"pinned backend smoke evidence absent"},
                {"id":"OM-A21-04","status":"deferred","reason":"live containment enforcement not tested"},
                {"id":"OM-A21-05","status":"deferred","reason":"install, upgrade and removal preservation not tested"}
            ]
        })
    }
    pub fn text(&self) -> String {
        let mut out = format!(
            "Archy doctor — diagnostic ready: {}\n",
            if self.ready { "yes" } else { "no" }
        );
        for check in &self.checks {
            out.push_str(&format!(
                "{}: {} — {}\n",
                check.id,
                check.status.name(),
                check.reason
            ));
            if check.status != CheckStatus::Available {
                out.push_str(&format!("  Remedy: {}\n", check.remedy));
            }
        }
        out.push_str("Target acceptance: deferred\n");
        out
    }
}

fn check(
    id: &'static str,
    required_for: &[&'static str],
    status: CheckStatus,
    observed: Option<String>,
    expected: Option<String>,
    evidence: &[&'static str],
    messages: (impl Into<String>, impl Into<String>),
) -> CheckResult {
    CheckResult {
        id,
        required_for: required_for.to_vec(),
        status,
        observed,
        expected,
        evidence: evidence.to_vec(),
        reason: messages.0.into(),
        remedy: messages.1.into(),
    }
}

fn value(probe: &Probe<String>) -> Option<String> {
    if let Probe::Found(v) = probe {
        Some(v.clone())
    } else {
        None
    }
}
fn status(probe: &Probe<String>) -> CheckStatus {
    match probe {
        Probe::Found(_) => CheckStatus::Available,
        Probe::Missing => CheckStatus::Missing,
        Probe::Unavailable(_) => CheckStatus::Unavailable,
        Probe::Deferred(_) => CheckStatus::Deferred,
    }
}
fn reason(probe: &Probe<String>) -> String {
    match probe {
        Probe::Found(_) => "observed".into(),
        Probe::Missing => "not installed or absent".into(),
        Probe::Unavailable(e) | Probe::Deferred(e) => e.clone(),
    }
}

fn package_version(raw: &str, name: &str) -> Result<String, ()> {
    let line = raw.strip_suffix('\n').unwrap_or(raw);
    if line.contains('\n') || line.contains('\r') || line.len() > 256 {
        return Err(());
    }
    let mut words = line.split(' ');
    match (words.next(), words.next(), words.next()) {
        (Some(n), Some(v), None) if n == name && !v.is_empty() && v.is_ascii() => Ok(v.into()),
        _ => Err(()),
    }
}
fn upstream(version: &str) -> Option<&str> {
    let (base, release) = version.rsplit_once('-')?;
    if release.is_empty() || !release.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let (numeric, prerelease) = base.split_once('-').unwrap_or((base, ""));
    if !prerelease.is_empty()
        && !prerelease
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.')
    {
        return None;
    }
    if numeric.split('.').count() != 3
        || !numeric
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    Some(base)
}

pub fn evaluate(o: &HostObservation, p: &CompatibilityPolicy) -> DoctorReport {
    let mut checks = Vec::new();
    let environment = if o.wsl {
        "wsl"
    } else if matches!(&o.os_id, Probe::Found(id) if id == "arch")
        && matches!(&o.omarchy, Probe::Found(_))
    {
        "omarchy"
    } else if matches!(&o.os_id, Probe::Found(_)) {
        "other-linux"
    } else {
        "unknown"
    };
    let os_ok = matches!(&o.os_id, Probe::Found(id) if id == "arch") && !o.wsl;
    checks.push(check(
        "platform.os",
        &["core"],
        if o.wsl {
            CheckStatus::Unsupported
        } else if os_ok {
            CheckStatus::Available
        } else if matches!(o.os_id, Probe::Found(_)) {
            CheckStatus::Unsupported
        } else {
            status(&o.os_id)
        },
        value(&o.os_id),
        Some("arch on native Omarchy".into()),
        &["/etc/os-release", "/proc/sys/kernel/osrelease"],
        (
            if o.wsl {
                "WSL is development evidence only".into()
            } else {
                reason(&o.os_id)
            },
            "Use a native Omarchy target for acceptance",
        ),
    ));
    checks.push(check(
        "platform.kernel",
        &["core"],
        status(&o.kernel),
        value(&o.kernel),
        None,
        &["/proc/sys/kernel/osrelease"],
        (
            reason(&o.kernel),
            "Confirm a readable native kernel identity",
        ),
    ));
    let partial_identity = value(&o.omarchy).or_else(|| value(&o.omarchy_dev));
    let upstream_version = value(&o.omarchy)
        .and_then(|raw| package_version(&raw, "omarchy").ok())
        .and_then(|v| upstream(&v).map(str::to_owned));
    let (pkg_status, raw, pkg_reason) = match (&o.omarchy, &o.omarchy_dev) {
        (Probe::Unavailable(e), _) | (_, Probe::Unavailable(e)) => (
            CheckStatus::Unavailable,
            partial_identity.clone(),
            e.clone(),
        ),
        (Probe::Found(a), Probe::Found(b)) => (
            CheckStatus::Unsupported,
            Some(format!("{a}; {b}")),
            "conflicting stable and development packages".into(),
        ),
        (_, Probe::Found(b)) => (
            CheckStatus::Unsupported,
            Some(b.clone()),
            "development package is not approved".into(),
        ),
        (Probe::Found(a), Probe::Missing) => match package_version(a, "omarchy") {
            Err(()) => (
                CheckStatus::Unavailable,
                Some(a.clone()),
                "malformed package query output".into(),
            ),
            Ok(v) => match upstream(&v) {
                Some(base) if base == p.omarchy_version => (
                    CheckStatus::Available,
                    Some(a.clone()),
                    "approved documented baseline identity".into(),
                ),
                _ => (
                    CheckStatus::Unsupported,
                    Some(a.clone()),
                    "package version is not the approved stable baseline".into(),
                ),
            },
        },
        (Probe::Missing, Probe::Missing) => {
            (CheckStatus::Missing, None, "Omarchy package absent".into())
        }
        _ => (
            CheckStatus::Unavailable,
            None,
            "package identity could not be established".into(),
        ),
    };
    let raw_package_identity = raw.clone();
    checks.push(check(
        "platform.omarchy",
        &["core"],
        pkg_status,
        raw,
        Some(format!(
            "omarchy {} with Arch release suffix",
            p.omarchy_version
        )),
        &[
            "/usr/bin/pacman -Q omarchy",
            "/usr/bin/pacman -Q omarchy-dev",
        ],
        (
            pkg_reason,
            "Install or verify the reviewed stable Omarchy package; resolve ambiguous identities",
        ),
    ));
    checks.push(check(
        "platform.development-path",
        &["core"],
        if o.custom_omarchy_path {
            CheckStatus::Unsupported
        } else {
            CheckStatus::Available
        },
        if o.custom_omarchy_path {
            Some("custom OMARCHY_PATH set".into())
        } else {
            None
        },
        None,
        &["OMARCHY_PATH environment metadata"],
        (
            if o.custom_omarchy_path {
                "custom development path is untrusted"
            } else {
                "no custom development path observed"
            },
            "Unset the development override and inspect installed package identity",
        ),
    ));
    for (id, probe, required, evidence, remedy) in [
        (
            "tools.bwrap",
            &o.bubblewrap,
            vec!["containment"],
            "/usr/bin/pacman -Q bubblewrap",
            "Install the reviewed bubblewrap package; enforcement requires target testing",
        ),
        (
            "desktop.shell",
            &o.shell,
            vec!["desktop"],
            "/usr/bin/pacman -Q omarchy-shell",
            "Install and validate the reviewed shell integration on target",
        ),
        (
            "containment.cgroups",
            &o.cgroup_controllers,
            vec!["containment"],
            "/sys/fs/cgroup/cgroup.controllers",
            "Inspect delegation and effective limits on target",
        ),
        (
            "containment.user-manager",
            &o.user_manager,
            vec!["containment"],
            "passive policy",
            "Check an existing user manager during target acceptance",
        ),
    ] {
        checks.push(check(
            id,
            &required,
            status(probe),
            value(probe),
            None,
            &[evidence],
            (reason(probe), remedy),
        ));
    }
    for (id, capability, probe, proposed) in [
        (
            "backend.blender",
            "blender",
            &o.blender_version,
            p.blender_proposal.as_str(),
        ),
        (
            "backend.ifc",
            "ifc",
            &o.ifc_version,
            p.ifc_proposal.as_str(),
        ),
    ] {
        let (state, why) = match probe {
            Probe::Found(v) if v != proposed => (
                CheckStatus::Unsupported,
                "installed metadata differs from proposed pin".to_string(),
            ),
            Probe::Found(_) => (
                CheckStatus::Deferred,
                "matching metadata cannot prove runtime support".to_string(),
            ),
            Probe::Missing => (
                CheckStatus::Missing,
                "backend installation metadata absent".to_string(),
            ),
            Probe::Unavailable(e) => (CheckStatus::Unavailable, e.clone()),
            Probe::Deferred(e) => (CheckStatus::Deferred, e.clone()),
        };
        checks.push(check(
            id,
            &[capability],
            state,
            value(probe),
            Some(proposed.into()),
            &[
                "/usr/share/archy/backends.json",
                "packaging/arch/compatibility.json",
            ],
            (
                format!("{why}; evidence is {}", p.backend_evidence),
                "Verify exact archive, complete lock, worker and native smoke tests on target",
            ),
        ));
    }
    checks.push(check(
        "containment.enforcement",
        &["containment"],
        CheckStatus::Deferred,
        None,
        None,
        &["OM-A21-04"],
        (
            "passive inventory cannot prove resource enforcement or cancellation",
            "Run A16 containment harness on target",
        ),
    ));
    checks.push(check(
        "desktop.plugin-api",
        &["desktop"],
        CheckStatus::Deferred,
        None,
        None,
        &["OM-A22-01"],
        (
            "installed plugin API and live shell behavior not tested",
            "Run A22 plugin validation on target",
        ),
    ));
    checks.sort_by_key(|c| c.id);
    let ready = checks
        .iter()
        .filter(|c| c.required_for.contains(&"core"))
        .all(|c| c.status == CheckStatus::Available);
    DoctorReport {
        host: json!({"os_id": value(&o.os_id), "kernel": value(&o.kernel), "architecture": o.architecture,
        "environment": environment, "evidence_sources": ["/etc/os-release", "/proc/sys/kernel/osrelease", "std::env::consts::ARCH"]}),
        checks,
        ready,
        supported_baseline: p.omarchy_version.clone(),
        upstream_version,
        raw_package_identity,
    }
}

fn read_regular_file(path: &str) -> Probe<String> {
    let selected = if path == "/etc/os-release" {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => match fs::canonicalize(path) {
                Ok(target) if target == Path::new("/usr/lib/os-release") => target,
                _ => return Probe::Unavailable("os-release symlink target is not approved".into()),
            },
            Ok(_) => Path::new(path).to_path_buf(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Probe::Missing,
            Err(e) => return Probe::Unavailable(e.kind().to_string()),
        }
    } else {
        Path::new(path).to_path_buf()
    };
    let meta = match fs::symlink_metadata(&selected) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Probe::Missing,
        Err(e) => return Probe::Unavailable(e.kind().to_string()),
    };
    if !meta.file_type().is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
        return Probe::Unavailable("file is not a trusted regular file".into());
    }
    let fd = match rustix::fs::open(
        &selected,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Probe::Missing,
        Err(e) => return Probe::Unavailable(e.to_string()),
    };
    let file = fs::File::from(fd);
    let opened = match file.metadata() {
        Ok(meta) => meta,
        Err(e) => return Probe::Unavailable(e.kind().to_string()),
    };
    if !opened.file_type().is_file()
        || opened.uid() != 0
        || opened.mode() & 0o022 != 0
        || opened.dev() != meta.dev()
        || opened.ino() != meta.ino()
    {
        return Probe::Unavailable("opened file provenance changed".into());
    }
    let mut bytes = Vec::new();
    if file.take(LIMIT + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > LIMIT {
        return Probe::Unavailable("read failed or exceeded 64 KiB".into());
    }
    match String::from_utf8(bytes) {
        Ok(s) => Probe::Found(s),
        Err(_) => Probe::Unavailable("invalid UTF-8".into()),
    }
}
fn bounded_file(path: &str, deadline: Instant) -> Probe<String> {
    let wait = TIMEOUT.min(deadline.saturating_duration_since(Instant::now()));
    if wait.is_zero() {
        return Probe::Unavailable("collection deadline exhausted".into());
    }
    let path = path.to_owned();
    let (send, receive) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = send.send(read_regular_file(&path));
    });
    match receive.recv_timeout(wait) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Probe::Unavailable("file probe timed out".into()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Probe::Unavailable("file probe failed".into()),
    }
}
fn os_id(deadline: Instant) -> Probe<String> {
    match bounded_file("/etc/os-release", deadline) {
        Probe::Found(s) => s
            .lines()
            .find_map(|l| l.strip_prefix("ID="))
            .map(|v| Probe::Found(v.trim_matches('"').into()))
            .unwrap_or(Probe::Unavailable("ID missing in os-release".into())),
        Probe::Missing => Probe::Missing,
        Probe::Unavailable(e) => Probe::Unavailable(e),
        Probe::Deferred(e) => Probe::Deferred(e),
    }
}
fn backend_metadata(field: &str, deadline: Instant) -> Probe<String> {
    match bounded_file("/usr/share/archy/backends.json", deadline) {
        Probe::Found(s) => match serde_json::from_str::<Value>(&s) {
            Ok(v) => {
                v[field]
                    .as_str()
                    .map(|s| Probe::Found(s.into()))
                    .unwrap_or(Probe::Unavailable(
                        "backend metadata field absent or invalid".into(),
                    ))
            }
            Err(_) => Probe::Unavailable("malformed backend metadata".into()),
        },
        Probe::Missing => Probe::Missing,
        Probe::Unavailable(e) => Probe::Unavailable(e),
        Probe::Deferred(e) => Probe::Deferred(e),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum RunResult {
    Output {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    Unavailable(String),
}
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}
fn terminate_group(child: &mut std::process::Child) {
    if let Some(group) = i32::try_from(child.id())
        .ok()
        .and_then(|pid| pid.checked_neg())
    {
        unsafe {
            kill(group, 9);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}
#[cfg(test)]
fn run_bounded(binary: &Path, args: &[&str]) -> RunResult {
    run_bounded_with_timeout(binary, args, TIMEOUT)
}
fn run_bounded_with_timeout(binary: &Path, args: &[&str], timeout: Duration) -> RunResult {
    let mut child = match Command::new(binary)
        .args(args)
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return RunResult::Unavailable(e.kind().to_string()),
    };
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    let exceeded = Arc::new(AtomicBool::new(false));
    let read = |stream: Box<dyn Read + Send>| {
        let exceeded = Arc::clone(&exceeded);
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stream.take(LIMIT + 1).read_to_end(&mut bytes);
            if bytes.len() as u64 > LIMIT {
                exceeded.store(true, Ordering::Release);
            }
            (result, bytes)
        })
    };
    let out_handle = read(Box::new(stdout));
    let err_handle = read(Box::new(stderr));
    let start = Instant::now();
    let mut code = None;
    loop {
        if exceeded.load(Ordering::Acquire) {
            terminate_group(&mut child);
            return RunResult::Unavailable("probe output exceeded 64 KiB".into());
        }
        match child.try_wait() {
            Ok(Some(exit)) => code = exit.code(),
            Ok(None) => {}
            Err(_) => {
                terminate_group(&mut child);
                return RunResult::Unavailable("probe wait failed".into());
            }
        }
        if code.is_some() && out_handle.is_finished() && err_handle.is_finished() {
            break;
        }
        if start.elapsed() >= timeout {
            terminate_group(&mut child);
            return RunResult::Unavailable("probe timed out or output pipe remained open".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    let (out_result, out_bytes) = out_handle.join().expect("reader thread");
    let (err_result, err_bytes) = err_handle.join().expect("reader thread");
    if out_result.is_err()
        || err_result.is_err()
        || out_bytes.len() as u64 > LIMIT
        || err_bytes.len() as u64 > LIMIT
    {
        return RunResult::Unavailable("probe output exceeded 64 KiB or read failed".into());
    }
    match (String::from_utf8(out_bytes), String::from_utf8(err_bytes)) {
        (Ok(stdout), Ok(stderr)) => RunResult::Output {
            code,
            stdout,
            stderr,
        },
        _ => RunResult::Unavailable("probe output was not UTF-8".into()),
    }
}

fn trusted_pacman() -> bool {
    let path = Path::new("/usr/bin/pacman");
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    meta.file_type().is_file() && meta.uid() == 0 && meta.mode() & 0o022 == 0
}
fn query_package(name: &'static str, deadline: Instant) -> Probe<String> {
    let wait = TIMEOUT.min(deadline.saturating_duration_since(Instant::now()));
    if wait.is_zero() {
        return Probe::Unavailable("collection deadline exhausted".into());
    }
    if !trusted_pacman() {
        return Probe::Unavailable("trusted /usr/bin/pacman is absent".into());
    }
    match run_bounded_with_timeout(Path::new("/usr/bin/pacman"), &["-Q", name], wait) {
        RunResult::Output {
            code: Some(0),
            stdout,
            stderr,
        } if stderr.is_empty() => {
            if package_version(&stdout, name).is_ok() {
                Probe::Found(stdout.trim_end_matches('\n').into())
            } else {
                Probe::Unavailable("malformed package query output".into())
            }
        }
        RunResult::Output {
            code: Some(1),
            stdout,
            stderr,
        } if stdout.is_empty() && stderr.contains("was not found") && stderr.len() < 512 => {
            Probe::Missing
        }
        RunResult::Output { .. } => {
            Probe::Unavailable("package query failed or returned unexpected output".into())
        }
        RunResult::Unavailable(e) => Probe::Unavailable(e),
    }
}
pub fn collect() -> HostObservation {
    let deadline = Instant::now() + COLLECTION_TIMEOUT;
    let kernel = bounded_file("/proc/sys/kernel/osrelease", deadline);
    let wsl = matches!(&kernel, Probe::Found(k) if k.to_ascii_lowercase().contains("microsoft") || k.to_ascii_lowercase().contains("wsl"));
    HostObservation {
        os_id: os_id(deadline),
        kernel: kernel.map_trim(),
        architecture: std::env::consts::ARCH.into(),
        wsl,
        omarchy: query_package("omarchy", deadline),
        omarchy_dev: query_package("omarchy-dev", deadline),
        custom_omarchy_path: std::env::var_os("OMARCHY_PATH").is_some(),
        bubblewrap: query_package("bubblewrap", deadline),
        shell: query_package("omarchy-shell", deadline),
        cgroup_controllers: bounded_file("/sys/fs/cgroup/cgroup.controllers", deadline).map_trim(),
        user_manager: Probe::Deferred("user manager bus not queried by passive doctor".into()),
        blender_version: backend_metadata("blender_version", deadline),
        ifc_version: backend_metadata("ifcopenshell_version", deadline),
    }
}
impl Probe<String> {
    fn map_trim(self) -> Self {
        match self {
            Probe::Found(s) => Probe::Found(s.trim().into()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> HostObservation {
        HostObservation {
            os_id: Probe::Found("arch".into()),
            kernel: Probe::Found("6.12".into()),
            architecture: "x86_64".into(),
            wsl: false,
            omarchy: Probe::Missing,
            omarchy_dev: Probe::Missing,
            custom_omarchy_path: false,
            bubblewrap: Probe::Missing,
            shell: Probe::Missing,
            cgroup_controllers: Probe::Missing,
            user_manager: Probe::Deferred("not queried".into()),
            blender_version: Probe::Missing,
            ifc_version: Probe::Missing,
        }
    }
    #[test]
    fn package_fixture_statuses_and_null_unknowns() {
        let cases: Value =
            serde_json::from_str(include_str!("../../../tests/doctor/package-cases.json")).unwrap();
        let policy = CompatibilityPolicy::checked_in().unwrap();
        for case in cases.as_array().unwrap() {
            let mut o = base();
            o.omarchy = case["omarchy"]
                .as_str()
                .map(|s| Probe::Found(s.trim_end().into()))
                .unwrap_or(Probe::Missing);
            o.omarchy_dev = case["dev"]
                .as_str()
                .map(|s| Probe::Found(s.trim_end().into()))
                .unwrap_or(Probe::Missing);
            let report = evaluate(&o, &policy).json();
            let platform = report["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == "platform.omarchy")
                .unwrap();
            assert_eq!(platform["status"], case["want"], "{}", case["name"]);
            assert_eq!(
                report["platform"]["upstream_version"], case["version"],
                "{}",
                case["name"]
            );
            assert_eq!(report["capabilities"]["blender"]["ready"], false);
        }
    }
    #[test]
    fn denied_and_wsl_are_not_ready() {
        let policy = CompatibilityPolicy::checked_in().unwrap();
        let mut o = base();
        o.omarchy = Probe::Unavailable("permission denied".into());
        assert_eq!(
            evaluate(&o, &policy).json()["platform"]["compatibility"],
            "unavailable"
        );
        o.omarchy = Probe::Found("omarchy 4.0.6-1".into());
        o.wsl = true;
        let report = evaluate(&o, &policy);
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.json()["host"]["environment"], "wsl");
    }
    #[test]
    fn missing_kernel_blocks_core_even_with_stable_package() {
        let mut o = base();
        o.omarchy = Probe::Found("omarchy 4.0.6-1".into());
        o.kernel = Probe::Unavailable("denied".into());
        let report = evaluate(&o, &CompatibilityPolicy::checked_in().unwrap());
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.json()["capabilities"]["core"]["ready"], false);
    }
    #[test]
    fn optional_desktop_absence_does_not_block_core_diagnostics() {
        let mut o = base();
        o.omarchy = Probe::Found("omarchy 4.0.6-2".into());
        let report = evaluate(&o, &CompatibilityPolicy::checked_in().unwrap());
        assert_eq!(report.exit_code(), 0);
        let json = report.json();
        assert_eq!(json["capabilities"]["core"]["ready"], true);
        assert_eq!(json["capabilities"]["desktop"]["ready"], false);
        assert_eq!(json["target_acceptance"], "deferred");
    }
    #[test]
    fn matching_backend_metadata_remains_deferred() {
        let mut o = base();
        o.blender_version = Probe::Found("4.5.15".into());
        let report = evaluate(&o, &CompatibilityPolicy::checked_in().unwrap()).json();
        let check = report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "backend.blender")
            .unwrap();
        assert_eq!(check["status"], "deferred");
        assert_eq!(report["capabilities"]["blender"]["ready"], false);
    }
    #[test]
    fn runner_bounds_output_and_timeout() {
        assert!(
            matches!(run_bounded(Path::new("/usr/bin/printf"), &["bad output"]), RunResult::Output { stdout, .. } if stdout == "bad output")
        );
        assert!(
            matches!(run_bounded(Path::new("/usr/bin/yes"), &[]), RunResult::Unavailable(e) if e.contains("exceeded"))
        );
        assert!(
            matches!(run_bounded(Path::new("/usr/bin/sleep"), &["5"]), RunResult::Unavailable(e) if e.contains("timed out"))
        );
    }
    #[test]
    fn runner_bounds_descendant_held_pipe() {
        let start = Instant::now();
        let result = run_bounded(Path::new("/usr/bin/sh"), &["-c", "/usr/bin/sleep 5 &"]);
        assert!(matches!(result, RunResult::Unavailable(e) if e.contains("timed out")));
        assert!(start.elapsed() < Duration::from_secs(4));
    }
    #[test]
    fn partial_package_evidence_survives_other_query_failure() {
        let mut o = base();
        o.omarchy = Probe::Found("omarchy 4.0.6-2".into());
        o.omarchy_dev = Probe::Unavailable("denied".into());
        let report = evaluate(&o, &CompatibilityPolicy::checked_in().unwrap()).json();
        assert_eq!(report["platform"]["compatibility"], "unavailable");
        assert_eq!(
            report["platform"]["raw_package_identity"],
            "omarchy 4.0.6-2"
        );
        assert_eq!(report["platform"]["upstream_version"], "4.0.6");
    }
    #[test]
    fn passive_read_rejects_symlink_and_device() {
        let root = std::env::temp_dir().join(format!("archy-doctor-file-{}", std::process::id()));
        let _ = fs::remove_file(&root);
        std::os::unix::fs::symlink("/dev/null", &root).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        assert!(matches!(
            bounded_file(root.to_str().unwrap(), deadline),
            Probe::Unavailable(_)
        ));
        assert!(matches!(
            bounded_file("/dev/null", deadline),
            Probe::Unavailable(_)
        ));
        fs::remove_file(root).unwrap();
    }
    #[test]
    fn passive_read_rejects_fifo_without_opening_it() {
        let fifo = std::env::temp_dir().join(format!("archy-doctor-fifo-{}", std::process::id()));
        let _ = fs::remove_file(&fifo);
        assert!(
            Command::new("/usr/bin/mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let start = Instant::now();
        let result = bounded_file(fifo.to_str().unwrap(), start + Duration::from_secs(1));
        assert!(matches!(result, Probe::Unavailable(_)));
        assert!(start.elapsed() < Duration::from_secs(1));
        fs::remove_file(fifo).unwrap();
    }
    #[test]
    fn exhausted_collection_deadline_does_not_start_file_probe() {
        assert!(
            matches!(bounded_file("/etc/os-release", Instant::now() - Duration::from_secs(1)), Probe::Unavailable(e) if e.contains("deadline"))
        );
    }
    #[test]
    fn unsupported_blender_cannot_become_ready_or_fall_back() {
        let mut o = base();
        o.blender_version = Probe::Found("5.2.3".into());
        let report = evaluate(&o, &CompatibilityPolicy::checked_in().unwrap()).json();
        let check = report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "backend.blender")
            .unwrap();
        assert_eq!(check["status"], "unsupported");
        assert_eq!(check["observed"], "5.2.3");
        assert_eq!(report["capabilities"]["blender"]["ready"], false);
    }
}
