use crate::artifact_invalid;
use archy_core::project::v2;
use archy_core::{ArchyError, ErrorCode, NormalizedSpec, ShapeParameters, Variant, sha256_hex};
use std::{
    io::{ErrorKind, Read, Write},
    os::fd::AsFd,
    os::unix::fs::MetadataExt,
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const READER_SHA: &str = "ac49cd744f9d85e59d3f83c3907ef10fb7c215958e024568da47c002ec649555";
const BRIDGE_SHA: &str = "71abb0c0050f2980d6c94c858ceb75796b11ab5e011c2867d7050c0ae362ef4c";
const STEP_SCOPE_SHA: &str = "1673ce64df0c8154ee4b7222b38e47612b33b6c48643eb2ad7a18730e7c53857";
const WEB_IFC_JS_SHA: &str = "592491e48f22ba5dd2f1d33a6485469bed63ac1b5f48547a80b146536556ff6f";
const WEB_IFC_WASM_SHA: &str = "36b10f4f9fada12bb89ed664f9e61eae1c82b2ce5f04ee4175205149516498f5";
const WEB_IFC_PACKAGE_SHA: &str =
    "cbe190c48abd4f353ab2b08075cd1784ca9ad95f80b06b3f7b77491d40439376";
const RUNTIME_NODE: &str = "/usr/lib/archy/runtime/node/bin/node";
const RUNTIME_BWRAP: &str = "/usr/lib/archy/runtime/bin/bwrap";
const RUNTIME_VERIFIER: &str = "/usr/lib/archy/runtime/ifc-verifier";
const RUNTIME_GATE: &str = "/usr/lib/archy/archy-service-gate";
const LIMIT: usize = 8 * 1024 * 1024;

fn unavailable(message: &str) -> ArchyError {
    ArchyError::new(ErrorCode::BackendUnavailable, "ifc_verifier", message)
}

fn trusted_file(path: &Path, digest: Option<&str>) -> Result<(), ArchyError> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| unavailable("fixed IFC verifier runtime is absent"))?;
    if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
        return Err(unavailable("fixed IFC verifier runtime is not trusted"));
    }
    if let Some(digest) = digest {
        if meta.len() > 8 * 1024 * 1024 {
            return Err(unavailable("fixed IFC verifier runtime file exceeds bound"));
        }
        let bytes = std::fs::read(path)
            .map_err(|_| unavailable("fixed IFC verifier runtime is unreadable"))?;
        if sha256_hex(&bytes) != digest {
            return Err(unavailable("fixed IFC verifier runtime digest differs"));
        }
    }
    Ok(())
}

fn trusted_directory(path: &Path) -> Result<(), ArchyError> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| unavailable("fixed IFC verifier directory is absent"))?;
    if !meta.is_dir() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
        return Err(unavailable("fixed IFC verifier directory is not trusted"));
    }
    Ok(())
}

fn stop_runtime_audit(mut child: Child) {
    let _ = child.kill();
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    // An uninterruptible kernel wait must not hold the project lock forever.
    thread::spawn(move || {
        let _ = child.wait();
    });
}

fn runtime_audit_timeout() -> ArchyError {
    ArchyError::new(
        ErrorCode::BackendTimeout,
        "ifc_verifier",
        "fixed IFC runtime inventory audit timed out",
    )
}

fn wait_for_runtime_audit(mut child: Child, timeout: Duration) -> Result<(), ArchyError> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) if started.elapsed() >= timeout => return Err(runtime_audit_timeout()),
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err(unavailable("fixed IFC runtime inventory audit failed")),
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(20)),
            result => {
                stop_runtime_audit(child);
                return match result {
                    Ok(None) => Err(runtime_audit_timeout()),
                    _ => Err(unavailable(
                        "fixed IFC runtime inventory audit could not be waited",
                    )),
                };
            }
        }
    }
}

fn audit_installed_runtime() -> Result<(), ArchyError> {
    let gate = Path::new(RUNTIME_GATE);
    trusted_file(gate, None)?;
    if std::fs::symlink_metadata(gate)
        .map_err(|_| unavailable("fixed IFC runtime gate is absent"))?
        .mode()
        & 0o111
        == 0
    {
        return Err(unavailable("fixed IFC runtime gate is not executable"));
    }
    let child = Command::new(gate)
        .arg("--audit-runtime=ifc")
        .env_clear()
        .env("LC_ALL", "C")
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| unavailable("fixed IFC runtime inventory audit cannot start"))?;
    wait_for_runtime_audit(child, Duration::from_secs(180))
}

fn exchange_verifier(
    mut child: Child,
    frame: &[u8],
    timeout: Duration,
) -> Result<(ExitStatus, Vec<u8>), ArchyError> {
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

    let mut input = child.stdin.take();
    let mut output = child.stdout.take();
    let setup = || -> Result<(), ArchyError> {
        let stdin = input
            .as_ref()
            .ok_or_else(|| unavailable("IFC verifier stdin absent"))?;
        let stdout = output
            .as_ref()
            .ok_or_else(|| unavailable("IFC verifier stdout absent"))?;
        for fd in [stdin.as_fd(), stdout.as_fd()] {
            let flags = fcntl_getfl(fd).map_err(|_| unavailable("IFC verifier pipe flags"))?;
            fcntl_setfl(fd, flags | OFlags::NONBLOCK)
                .map_err(|_| unavailable("IFC verifier nonblocking pipe"))?;
        }
        Ok(())
    };
    if let Err(error) = setup() {
        drop(input);
        drop(output);
        stop_runtime_audit(child);
        return Err(error);
    }
    let started = Instant::now();
    let mut written = 0;
    let mut write_failed = false;
    let mut bytes = Vec::new();
    let mut status = None;
    let result = loop {
        if started.elapsed() >= timeout {
            break Err(ArchyError::new(
                ErrorCode::BackendTimeout,
                "ifc_verifier",
                "IFC verifier deadline exceeded",
            ));
        }
        let mut progressed = false;
        if let Some(stdin) = input.as_mut() {
            match stdin.write(&frame[written..]) {
                Ok(0) => {
                    write_failed = true;
                    input = None;
                }
                Ok(n) => {
                    progressed = true;
                    written += n;
                    if written == frame.len() {
                        input = None;
                    }
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                Err(_) => {
                    write_failed = true;
                    input = None;
                }
            }
        }
        if let Some(stdout) = output.as_mut() {
            let mut buffer = [0u8; 8193];
            match stdout.read(&mut buffer) {
                Ok(0) => output = None,
                Ok(n) => {
                    progressed = true;
                    bytes.extend_from_slice(&buffer[..n]);
                    if bytes.len() > 8192 {
                        break Err(artifact_invalid("IFC verifier protocol failure"));
                    }
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                Err(_) => break Err(artifact_invalid("IFC verifier output unreadable")),
            }
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(_) => break Err(unavailable("IFC verifier failed to wait")),
            }
        }
        if let Some(done) = status
            && input.is_none()
            && output.is_none()
        {
            if write_failed || written != frame.len() {
                break Err(artifact_invalid("IFC verifier protocol failure"));
            }
            break Ok((done, bytes));
        }
        if !progressed {
            thread::sleep(Duration::from_millis(2));
        }
    };
    drop(input);
    drop(output);
    if result.is_err() {
        stop_runtime_audit(child);
    }
    result
}

fn expected(
    spec: &NormalizedSpec,
    canonical: Option<&v2::NormalizedSpec>,
    source_format: &str,
    variant: &Variant,
    source_run: &str,
    source: &[u8],
) -> serde_json::Value {
    let (origin, width, depth, notch_width, notch_depth) = match &variant.parameters {
        ShapeParameters::Rectangle {
            origin_xy_m,
            width_m,
            depth_m,
        } => (origin_xy_m, width_m, depth_m, None, None),
        ShapeParameters::LShape {
            origin_xy_m,
            width_m,
            depth_m,
            notch_width_m,
            notch_depth_m,
        } => (
            origin_xy_m,
            width_m,
            depth_m,
            Some(notch_width_m),
            Some(notch_depth_m),
        ),
    };
    let area = variant
        .footprint_xy_m
        .windows(2)
        .map(|pair| pair[0][0] * pair[1][1] - pair[1][0] * pair[0][1])
        .sum::<f64>()
        .abs()
        / 2.0;
    let storeys = variant.storeys as f64;
    let (source_file, source_mode) = match (canonical, source_format) {
        (None, "json") => ("input.json", "accepted_project_json_v1"),
        (None, "yaml") => ("input.yaml", "accepted_project_normalized_v1"),
        (Some(_), "json") => ("input.json", "accepted_project_normalized_v2"),
        (Some(_), "yaml") => ("input.yaml", "accepted_project_normalized_v2"),
        _ => ("invalid", "invalid"),
    };
    serde_json::json!({
        "file":"model.ifc", "source_file":source_file, "source_mode":source_mode,
        "shape":variant.shape, "project_id":spec.project_id, "variant_id":variant.id,
        "run_id":source_run, "source_digest":sha256_hex(source),
        "origin_xy_m":origin, "width_m":width, "depth_m":depth,
        "notch_width_m":notch_width, "notch_depth_m":notch_depth,
        "north_angle_deg":spec.site.north_angle_deg,
        "footprint_xy_m":variant.footprint_xy_m, "footprint_area_m2":area,
        "approximate_gfa_m2":area * storeys,
        "mass_volume_m3":area * storeys * variant.floor_to_floor_m,
        "storeys":variant.storeys, "floor_to_floor_m":variant.floor_to_floor_m,
        "height_m":storeys * variant.floor_to_floor_m
    })
}

pub(crate) fn verify(
    model: &[u8],
    source: &[u8],
    spec: &NormalizedSpec,
    canonical: Option<&v2::NormalizedSpec>,
    source_format: &str,
    variant: &Variant,
    source_run: &str,
) -> Result<(), ArchyError> {
    if model.is_empty()
        || model.len() > LIMIT
        || source.is_empty()
        || source.len() > 1_048_576
        || !matches!(source_format, "json" | "yaml")
    {
        return Err(artifact_invalid("IFC verifier input exceeds bounds"));
    }
    let fixture = cfg!(test) && std::env::var_os("ARCHY_IFC_TEST_PYTHON").is_some();
    let root = if fixture {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/backend/ifc-reader"
        ))
    } else {
        Path::new(RUNTIME_VERIFIER)
    };
    let test_node = if fixture {
        std::env::var_os("ARCHY_IFC_TEST_NODE")
    } else {
        None
    };
    let node = test_node.as_deref().map(Path::new).unwrap_or_else(|| {
        if fixture {
            Path::new("node")
        } else {
            Path::new(RUNTIME_NODE)
        }
    });
    let bwrap = Path::new(if fixture {
        "/usr/bin/bwrap"
    } else {
        RUNTIME_BWRAP
    });
    let bridge = root.join("verify-stdin.cjs");
    let reader = root.join("reader.cjs");
    let modules = if fixture {
        std::fs::canonicalize(root.join("node_modules"))
            .map_err(|_| unavailable("test IFC verifier modules are absent"))?
    } else {
        root.join("node_modules")
    };
    let separate_lib64 = std::fs::symlink_metadata("/usr/lib64")
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false);
    if !fixture {
        for parent in [
            "/",
            "/usr",
            "/usr/lib",
            "/usr/lib/archy",
            "/usr/lib/archy/runtime",
            "/usr/lib/archy/runtime/bin",
            "/usr/lib/archy/runtime/node",
            "/usr/lib/archy/runtime/node/bin",
        ] {
            trusted_directory(Path::new(parent))?;
        }
        audit_installed_runtime()?;
        trusted_directory(root)?;
        trusted_directory(&root.join("node_modules"))?;
        trusted_directory(&root.join("node_modules/web-ifc"))?;
        trusted_file(node, None)?;
        trusted_file(bwrap, None)?;
        if separate_lib64 {
            trusted_directory(Path::new("/usr/lib64"))?;
        }
        trusted_file(&bridge, Some(BRIDGE_SHA))?;
        trusted_file(&reader, Some(READER_SHA))?;
        trusted_file(&root.join("step-scope.cjs"), Some(STEP_SCOPE_SHA))?;
        trusted_file(
            &root.join("node_modules/web-ifc/web-ifc-api-node.js"),
            Some(WEB_IFC_JS_SHA),
        )?;
        trusted_file(
            &root.join("node_modules/web-ifc/web-ifc-node.wasm"),
            Some(WEB_IFC_WASM_SHA),
        )?;
        trusted_file(
            &root.join("node_modules/web-ifc/package.json"),
            Some(WEB_IFC_PACKAGE_SHA),
        )?;
    }
    let expected = serde_json::to_vec(&expected(
        spec,
        canonical,
        source_format,
        variant,
        source_run,
        source,
    ))
    .map_err(|e| artifact_invalid(e.to_string()))?;
    if expected.len() > 16384 {
        return Err(artifact_invalid("IFC verifier expectation exceeds bound"));
    }
    let mut frame = Vec::with_capacity(12 + expected.len() + source.len() + model.len());
    for size in [expected.len(), source.len(), model.len()] {
        frame.extend_from_slice(&(size as u32).to_be_bytes());
    }
    frame.extend_from_slice(&expected);
    frame.extend_from_slice(source);
    frame.extend_from_slice(model);
    let mut command = if fixture {
        Command::new(bwrap)
    } else {
        let mut cmd = Command::new("/usr/lib/ld-linux-x86-64.so.2");
        cmd.args([
            "--inhibit-cache",
            "--library-path",
            "/usr/lib",
            RUNTIME_BWRAP,
        ]);
        cmd
    };
    command.args([
        "--unshare-user",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--die-with-parent",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--clearenv",
        "--dir",
        "/etc",
        "--setenv",
        "TZ",
        "UTC",
        "--setenv",
        "NODE_PATH",
        "/runtime/node_modules",
        "--dir",
        "/usr",
        "--dir",
        "/usr/bin",
        "--ro-bind",
        "/usr/lib",
        "/usr/lib",
    ]);
    if separate_lib64 {
        command.args(["--ro-bind", "/usr/lib64", "/usr/lib64"]);
    }
    command
        .args(["--symlink", "usr/lib", "/lib", "--symlink"])
        .arg(if separate_lib64 {
            "usr/lib64"
        } else {
            "usr/lib"
        })
        .args([
            "/lib64", "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp", "--dir", "/runtime",
            "--chdir", "/",
        ])
        .args(if fixture {
            vec!["--ro-bind", root.to_str().unwrap(), "/verifier"]
        } else {
            vec![]
        })
        .arg("--ro-bind")
        .arg(&modules)
        .arg("/runtime/node_modules")
        .args(if fixture {
            vec!["--ro-bind", node.to_str().unwrap(), "/usr/bin/node"]
        } else {
            vec![]
        })
        .arg(if fixture {
            "/usr/bin/node"
        } else {
            RUNTIME_NODE
        })
        .arg("--max-old-space-size=256")
        .arg("--no-addons")
        .arg(if fixture {
            Path::new("/verifier/verify-stdin.cjs")
        } else {
            bridge.as_path()
        })
        .env_clear()
        .env("TZ", "UTC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // The WASM engine reserves large virtual address ranges, so RLIMIT_AS is
    // unsuitable. RLIMIT_DATA caps committed heap/mmap bytes on Linux.
    unsafe {
        command.pre_exec(|| {
            use rustix::process::{Resource, Rlimit, setrlimit};
            setrlimit(
                Resource::Data,
                Rlimit {
                    current: Some(1024 * 1024 * 1024),
                    maximum: Some(1024 * 1024 * 1024),
                },
            )?;
            setrlimit(
                Resource::Cpu,
                Rlimit {
                    current: Some(20),
                    maximum: Some(20),
                },
            )?;
            setrlimit(
                Resource::Fsize,
                Rlimit {
                    current: Some(0),
                    maximum: Some(0),
                },
            )?;
            Ok(())
        });
    }
    let child = command
        .spawn()
        .map_err(|_| unavailable("fixed IFC verifier cannot start"))?;
    let (status, output) = exchange_verifier(child, &frame, Duration::from_secs(20))?;
    let report: serde_json::Value = serde_json::from_slice(&output)
        .map_err(|_| artifact_invalid("IFC verifier report invalid"))?;
    if report["code"] == "RUNTIME_UNAVAILABLE" {
        return Err(unavailable(
            "fixed IFC verifier runtime version or digest differs",
        ));
    }
    if !status.success()
        || report["status"] != "ok"
        || report["engine"] != "web-ifc"
        || report["engine_version"] != "0.0.78"
        || report["source_digest"] != sha256_hex(source)
    {
        return Err(artifact_invalid(format!(
            "independent IFC parser/geometry verification failed: {}",
            report["message"]
                .as_str()
                .unwrap_or("invalid verifier result")
                .chars()
                .take(256)
                .collect::<String>()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod runtime_trust_tests {
    use super::{
        exchange_verifier, sha256_hex, trusted_directory, trusted_file, verify,
        wait_for_runtime_audit,
    };
    use archy_core::{ErrorCode, NormalizedSpec, Variant};
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn immutable_runtime_ownership_modes_and_pins_are_enforced() {
        let root = std::env::temp_dir().join(format!("archy-ifc-runtime-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let file = root.join("reader.cjs");
        std::fs::write(&file, b"reviewed").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let mapped_root = std::fs::metadata(&root).unwrap().uid() == 0;
        if mapped_root {
            assert!(trusted_directory(&root).is_ok());
            assert!(trusted_file(&file, Some(&sha256_hex(b"reviewed"))).is_ok());
            assert!(trusted_file(&file, Some(&sha256_hex(b"unreviewed"))).is_err());
        } else {
            assert!(trusted_directory(&root).is_err());
            assert!(trusted_file(&file, None).is_err());
        }
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o664)).unwrap();
        assert!(trusted_file(&file, None).is_err());
        let link = root.join("link");
        symlink(&file, &link).unwrap();
        assert!(trusted_file(&link, None).is_err());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o775)).unwrap();
        assert!(trusted_directory(&root).is_err());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_audit_wait_fails_closed_and_reaps_on_timeout() {
        let start = |program: &str, args: &[&str]| {
            Command::new(program)
                .args(args)
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap()
        };
        assert!(
            wait_for_runtime_audit(start("/usr/bin/true", &[]), Duration::from_secs(2)).is_ok()
        );
        assert_eq!(
            wait_for_runtime_audit(start("/usr/bin/false", &[]), Duration::from_secs(2))
                .unwrap_err()
                .code,
            ErrorCode::BackendUnavailable
        );
        let mut completed = start("/usr/bin/true", &[]);
        assert!(completed.wait().unwrap().success());
        assert_eq!(
            wait_for_runtime_audit(completed, Duration::ZERO)
                .unwrap_err()
                .code,
            ErrorCode::BackendTimeout
        );
        let child = start("/usr/bin/sleep", &["60"]);
        let pid = child.id();
        let began = Instant::now();
        assert_eq!(
            wait_for_runtime_audit(child, Duration::from_millis(40))
                .unwrap_err()
                .code,
            ErrorCode::BackendTimeout
        );
        assert!(began.elapsed() < Duration::from_secs(10));
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }

    #[test]
    fn verifier_deadline_covers_pipe_io_after_child_exit() {
        let spawn = |script: &str| {
            Command::new("/bin/sh")
                .args(["-c", script])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap()
        };
        let (status, output) =
            exchange_verifier(spawn("/bin/cat"), b"payload", Duration::from_secs(2)).unwrap();
        assert!(status.success());
        assert_eq!(output, b"payload");
        let big = vec![7u8; 9 * 1024 * 1024];
        let (status, output) =
            exchange_verifier(spawn("/usr/bin/wc -c"), &big, Duration::from_secs(5)).unwrap();
        assert!(status.success());
        assert_eq!(
            String::from_utf8(output).unwrap().trim(),
            big.len().to_string()
        );
        let started = Instant::now();
        let error = exchange_verifier(
            spawn("/usr/bin/sleep 2 & printf '{}'"),
            b"payload",
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::BackendTimeout);
        assert!(started.elapsed() < Duration::from_secs(2));
        let started = Instant::now();
        let error = exchange_verifier(
            spawn("/usr/bin/sleep 2 &"),
            &vec![0u8; 1_000_000],
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::BackendTimeout);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    fn exercise_fixture_verifier() -> archy_core::ArchyError {
        let spec: NormalizedSpec = serde_json::from_slice(include_bytes!(
            "../../../tests/fixtures/legacy-preference-run/archived-runs/5248af90-8dfe-4743-b056-7d3385c62c97/normalized.json"
        )).unwrap();
        let variant: Variant = serde_json::from_slice(include_bytes!(
            "../../../tests/fixtures/legacy-preference-run/archived-runs/5248af90-8dfe-4743-b056-7d3385c62c97/variants/variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b.json"
        )).unwrap();
        verify(
            b"invalid model",
            b"{}",
            &spec,
            None,
            "json",
            &variant,
            "run",
        )
        .unwrap_err()
    }

    #[test]
    #[ignore = "requires a disposable container with a descendant holding the verifier pipe"]
    fn verifier_deadline_covers_descendant_held_pipe() {
        let started = Instant::now();
        let error = exercise_fixture_verifier();
        assert_eq!(error.code, ErrorCode::BackendTimeout);
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    #[test]
    #[ignore = "requires a disposable container with the actual gate and a tampered inventory"]
    fn invalid_inventory_stops_verifier_before_bwrap() {
        let marker = Path::new("/tmp/archy-untrusted-bwrap-ran");
        assert!(!marker.exists());
        let error = exercise_fixture_verifier();
        assert_eq!(error.code, ErrorCode::BackendUnavailable);
        assert!(
            !marker.exists(),
            "bwrap ran after the inventory was rejected"
        );
    }

    #[test]
    #[ignore = "requires a disposable container with synthetic inventoried runtime files"]
    fn valid_inventory_reaches_fixture_bwrap() {
        let marker = Path::new("/tmp/archy-untrusted-bwrap-ran");
        assert!(!marker.exists());
        let error = exercise_fixture_verifier();
        assert_eq!(error.code, ErrorCode::ArtifactInvalid);
        assert!(marker.exists(), "fixture bwrap was not reached");
    }
}
