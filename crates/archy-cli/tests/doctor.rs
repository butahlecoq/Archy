use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn doctor_json_reports_unresolved_target_evidence_without_path_execution() {
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["doctor", "--json"])
        .env("PATH", "/definitely/not/a/tool")
        .env("PYTHONPATH", "/untrusted/python")
        .env("MISE_DATA_DIR", "/untrusted/mise")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], "1");
    assert_eq!(report["ready"], false);
    assert_eq!(report["target_acceptance"], "deferred");
    assert_eq!(report["capabilities"]["blender"]["ready"], false);
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["id"] == "platform.omarchy")
    );
}

#[test]
fn doctor_text_is_readable_and_reports_failed_prerequisites() {
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .arg("doctor")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Archy doctor"));
    assert!(text.contains("Target acceptance: deferred"));
}

#[test]
fn doctor_does_not_run_path_shim_or_change_home_and_config() {
    let dir = std::env::temp_dir().join(format!(
        "archy-doctor-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bin = dir.join("path with spaces");
    let home = dir.join("home");
    let config = home.join(".config");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("sentinel"), b"keep").unwrap();
    let shim = bin.join("pacman");
    fs::write(&shim, b"#!/bin/sh\necho touched > executed\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["doctor", "--json"])
        .env("PATH", &bin)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &config)
        .env("PYTHONPATH", &bin)
        .env("MISE_DATA_DIR", &bin)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(fs::read(config.join("sentinel")).unwrap(), b"keep");
    assert!(!home.join("executed").exists());
    assert_eq!(fs::read_dir(&config).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}
