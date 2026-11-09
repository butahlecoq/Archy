use crate::{BoundProjectRead, artifact_invalid};
use archy_core::{ArchyError, ErrorCode, sha256_hex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub const DESKTOP_OPEN_VERSION: &str = "archy-desktop-open-v1";

#[derive(Debug, Clone)]
pub struct DesktopOpenInput {
    pub project: PathBuf,
    pub run: String,
    pub artifact: String,
    pub kind: DesktopOpenKind,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopOpenKind {
    Report,
    Preview,
    Blender,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopOpenStatus {
    HandedToViewer,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopOpenReceipt {
    pub schema_version: String,
    pub status: DesktopOpenStatus,
    pub run_id: String,
    pub artifact_id: String,
    pub kind: DesktopOpenKind,
    pub sha256: String,
}
fn denied(message: &str) -> ArchyError {
    ArchyError::new(ErrorCode::PermissionDenied, "desktop.open", message)
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_/.".contains(&b))
        && !value.starts_with('/')
        && !value
            .split('/')
            .any(|s| s == "." || s == ".." || s.is_empty())
}
fn viewer_argv(kind: DesktopOpenKind, staged: &Path) -> (&'static str, Vec<std::ffi::OsString>) {
    match kind {
        DesktopOpenKind::Report | DesktopOpenKind::Preview => {
            ("/usr/bin/xdg-open", vec![staged.as_os_str().to_owned()])
        }
        DesktopOpenKind::Blender => (
            "/usr/bin/blender",
            vec![
                "--factory-startup".into(),
                "--disable-autoexec".into(),
                staged.as_os_str().to_owned(),
            ],
        ),
    }
}
fn viewer_command(kind: DesktopOpenKind, staged: &Path) -> Command {
    let (executable, args) = viewer_argv(kind, staged);
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir("/")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in [
        "HOME",
        "XDG_RUNTIME_DIR",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_CURRENT_DESKTOP",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
}
fn launch_viewer(mut command: Command, diagnostic: &Path) -> Result<(), ArchyError> {
    let mut stderr = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(diagnostic)
        .map_err(|e| denied(&e.to_string()))?;
    command.stderr(Stdio::from(
        stderr.try_clone().map_err(|e| denied(&e.to_string()))?,
    ));
    let mut child = command.spawn().map_err(|e| {
        ArchyError::new(ErrorCode::BackendUnavailable, "desktop.open", e.to_string())
    })?;
    // The open operation is a bounded handoff. Catch immediate failures without
    // waiting for a viewer that remains open for the whole editing session.
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| denied(&e.to_string()))? {
            if status.success() {
                return Ok(());
            }
            stderr
                .seek(SeekFrom::Start(0))
                .map_err(|e| denied(&e.to_string()))?;
            let mut bytes = Vec::new();
            stderr
                .take(4096)
                .read_to_end(&mut bytes)
                .map_err(|e| denied(&e.to_string()))?;
            let detail = String::from_utf8_lossy(&bytes);
            return Err(ArchyError::new(
                ErrorCode::BackendUnavailable,
                "desktop.open",
                format!("viewer exited {status}: {}", detail.trim()),
            ));
        }
        if Instant::now() >= deadline {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
}
pub(crate) fn is_openable_path(kind: DesktopOpenKind, artifact: &str) -> bool {
    if !valid_id(artifact) {
        return false;
    }
    let render_file = artifact.strip_prefix("renders/").and_then(|tail| {
        let (variant, name) = tail.split_once('/')?;
        (variant.starts_with("variant-")
            && variant.len() == 72
            && variant[8..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
        .then_some(name)
    });
    match kind {
        DesktopOpenKind::Report => artifact == "report.md" || artifact == "report.json",
        DesktopOpenKind::Preview => {
            (artifact.starts_with("previews/") && artifact.ends_with(".png"))
                || matches!(
                    render_file,
                    Some("view-top.png" | "view-front.png" | "view-iso.png")
                )
        }
        DesktopOpenKind::Blender => {
            (artifact.starts_with("scenes/") && artifact.ends_with(".blend"))
                || render_file == Some("scene.blend")
        }
    }
}
pub fn desktop_open(input: DesktopOpenInput) -> Result<DesktopOpenReceipt, ArchyError> {
    if !input.project.is_absolute()
        || uuid::Uuid::parse_str(&input.run).is_err()
        || !valid_id(&input.artifact)
        || !is_openable_path(input.kind, &input.artifact)
    {
        return Err(denied("invalid project, run, artifact, or kind"));
    }
    let bound = BoundProjectRead::open(&input.project)?;
    let snapshot = bound.snapshot()?;
    let run = snapshot.verify_run(&input.run)?;
    let recorded = run
        .manifest()
        .artifacts
        .iter()
        .find(|a| a.path == input.artifact)
        .ok_or_else(|| artifact_invalid("artifact is absent from accepted run"))?;
    let bytes = snapshot.verified_artifact(&input.run, &input.artifact)?;
    if bytes.len() as u64 != recorded.byte_len || sha256_hex(&bytes) != recorded.sha256 {
        return Err(artifact_invalid(
            "artifact bytes differ from accepted manifest",
        ));
    }
    let (executable, _) = viewer_argv(input.kind, Path::new("/"));
    if !Path::new(executable).is_file() {
        return Err(ArchyError::new(
            ErrorCode::BackendUnavailable,
            "desktop.open",
            "fixed viewer is unavailable",
        ));
    }
    // The viewer receives a fresh copy of verified bytes. It never receives a
    // project-controlled path that could be replaced between verification and open.
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| denied("XDG_RUNTIME_DIR is required for desktop opening"))?;
    let runtime_meta = fs::symlink_metadata(&runtime).map_err(|e| denied(&e.to_string()))?;
    if !runtime.is_absolute()
        || !runtime_meta.file_type().is_dir()
        || runtime_meta.permissions().mode() & 0o077 != 0
    {
        return Err(denied("invalid runtime directory"));
    }
    let dir = runtime.join(format!("archy-open-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).map_err(|e| denied(&e.to_string()))?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
        .map_err(|e| denied(&e.to_string()))?;
    let suffix = Path::new(&input.artifact)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin");
    let staged = dir.join(format!("artifact.{suffix}"));
    let result = (|| -> Result<(), ArchyError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
            .map_err(|e| denied(&e.to_string()))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| denied(&e.to_string()))?;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o400))
            .map_err(|e| denied(&e.to_string()))?;
        let command = viewer_command(input.kind, &staged);
        let diagnostic = dir.join("viewer.stderr");
        let launch = launch_viewer(command, &diagnostic);
        let _ = fs::remove_file(diagnostic);
        launch?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staged);
        let _ = fs::remove_dir(&dir);
    }
    result?;
    Ok(DesktopOpenReceipt {
        schema_version: DESKTOP_OPEN_VERSION.into(),
        status: DesktopOpenStatus::HandedToViewer,
        run_id: input.run,
        artifact_id: input.artifact,
        kind: input.kind,
        sha256: recorded.sha256.clone(),
    })
}
pub fn desktop_open_schema() -> schemars::Schema {
    schemars::schema_for!(DesktopOpenReceipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_argv_and_injected_identifiers() {
        let path = Path::new("/run/user/1000/archy-open-x/artifact.md");
        let (program, args) = viewer_argv(DesktopOpenKind::Report, path);
        assert_eq!(program, "/usr/bin/xdg-open");
        assert_eq!(args, vec![path.as_os_str()]);
        let (program, args) = viewer_argv(DesktopOpenKind::Blender, path);
        assert_eq!(program, "/usr/bin/blender");
        assert_eq!(
            args,
            vec![
                "--factory-startup",
                "--disable-autoexec",
                "/run/user/1000/archy-open-x/artifact.md"
            ]
        );
        for bad in [
            "../report.md",
            "--help",
            "report.md;touch /tmp/x",
            "file:///tmp/x",
            "a\nwhoami",
            "previews/../../evil.png",
        ] {
            assert!(!valid_id(bad) || !is_openable_path(DesktopOpenKind::Report, bad));
        }
        assert!(is_openable_path(DesktopOpenKind::Report, "report.md"));
        let command = viewer_command(DesktopOpenKind::Report, path);
        assert_eq!(command.get_program(), "/usr/bin/xdg-open");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![path.as_os_str()]
        );
        assert!(
            command
                .get_envs()
                .all(|(name, _)| name != "BROWSER" && name != "PYTHONPATH" && name != "LD_PRELOAD")
        );
    }
    #[test]
    fn immediate_viewer_failure_and_bounded_success() {
        let root = std::env::temp_dir().join(format!("archy-viewer-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let diagnostic = root.join("stderr");
        let mut failed = Command::new("/bin/sh");
        failed.args(["-c", "echo cannot-open >&2; exit 3"]);
        let error = launch_viewer(failed, &diagnostic).unwrap_err();
        assert!(error.to_string().contains("cannot-open"), "{error}");
        fs::remove_file(&diagnostic).unwrap();
        let mut alive = Command::new("/bin/sh");
        alive.args(["-c", "sleep 1"]);
        let started = Instant::now();
        launch_viewer(alive, &diagnostic).unwrap();
        assert!(started.elapsed() < Duration::from_millis(800));
        fs::remove_file(diagnostic).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
