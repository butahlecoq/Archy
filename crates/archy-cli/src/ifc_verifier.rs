use crate::artifact_invalid;
use archy_core::project::v2;
use archy_core::{ArchyError, ErrorCode, NormalizedSpec, ShapeParameters, Variant, sha256_hex};
use std::{
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
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
        Path::new("/usr/lib/archy/ifc-verifier")
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
            Path::new("/usr/bin/node")
        }
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
        for parent in ["/", "/usr", "/usr/bin", "/usr/lib", "/usr/lib/archy"] {
            trusted_directory(Path::new(parent))?;
        }
        trusted_directory(root)?;
        trusted_directory(&root.join("node_modules"))?;
        trusted_directory(&root.join("node_modules/web-ifc"))?;
        trusted_file(node, None)?;
        trusted_file(Path::new("/usr/bin/bwrap"), None)?;
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
    let mut command = Command::new("/usr/bin/bwrap");
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
        .arg("--ro-bind")
        .arg(node)
        .arg("/usr/bin/node")
        .arg("/usr/bin/node")
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
    let mut child = command
        .spawn()
        .map_err(|_| unavailable("fixed IFC verifier cannot start"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| unavailable("IFC verifier stdin absent"))?;
    let writer = thread::spawn(move || stdin.write_all(&frame));
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| unavailable("IFC verifier stdout absent"))?;
    let reader_thread = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(8193).read_to_end(&mut bytes).map(|_| bytes)
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < Duration::from_secs(20) => {
                thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = writer.join();
                let _ = reader_thread.join();
                return Err(ArchyError::new(
                    ErrorCode::BackendTimeout,
                    "ifc_verifier",
                    "IFC verifier timed out or failed to wait",
                ));
            }
        }
    };
    let written = writer
        .join()
        .map_err(|_| unavailable("IFC verifier input thread failed"))?;
    let output = reader_thread
        .join()
        .map_err(|_| unavailable("IFC verifier output thread failed"))?
        .map_err(|_| artifact_invalid("IFC verifier output unreadable"))?;
    if written.is_err() || output.len() > 8192 {
        return Err(artifact_invalid("IFC verifier protocol failure"));
    }
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
