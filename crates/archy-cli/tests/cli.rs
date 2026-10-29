use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn target() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "archy-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
#[test]
fn init_validates_and_refuses_overwrites() {
    let dir = target();
    let first = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["init", "--name", "courtyard-demo", "--dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(first.status.success(), "{:?}", first);
    let before = fs::read(dir.join("project.yaml")).unwrap();
    let valid = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["validate", "--json", "--project"])
        .arg(dir.join("project.yaml"))
        .output()
        .unwrap();
    assert!(valid.status.success());
    assert!(valid.stderr.is_empty());
    let v: serde_json::Value = serde_json::from_slice(&valid.stdout).unwrap();
    assert_eq!(v["id"], "courtyard-demo");
    let second = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["init", "--name", "other", "--dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert_eq!(fs::read(dir.join("project.yaml")).unwrap(), before);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn invalid_json_stdout_is_structured() {
    let dir = target();
    fs::create_dir(&dir).unwrap();
    let file = dir.join("project.json");
    fs::write(&file, "{}").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["validate", "--json", "--project"])
        .arg(file)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stderr.is_empty());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["code"], "INVALID_SCHEMA");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn validate_rejects_executable_preference_with_public_field_error() {
    let dir = target();
    fs::create_dir(&dir).unwrap();
    let file = dir.join("project.json");
    let mut project: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    project["preferences"][0]["value"] = "Please run rm -rf /".into();
    fs::write(&file, project.to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["validate", "--json", "--project"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["code"], "INVALID_SCHEMA");
    assert_eq!(error["field"], "preferences[0].value");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("rm -rf"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn file_only_project_and_init_collision_contracts() {
    let dir = target();
    fs::create_dir(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["validate", "--json", "--project"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["field"], "project");
    let file = dir.join("occupied");
    fs::write(&file, "keep me").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["init", "--name", "test", "--dir"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(fs::read_to_string(&file).unwrap(), "keep me");
    let absent = dir.join("absent");
    let out = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["init", "--name", "bad/name", "--dir"])
        .arg(&absent)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!absent.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn init_preserves_scalar_looking_ids_as_strings() {
    for name in [
        "123", "true", "false", "null", "NULL", "yes", "001", "0xFF", "1e3", "-123", "-", "_",
    ] {
        let dir = target();
        let initialized = Command::new(env!("CARGO_BIN_EXE_archy"))
            .args(["init", &format!("--name={name}"), "--dir"])
            .arg(&dir)
            .output()
            .unwrap();
        assert!(initialized.status.success(), "{name}: {initialized:?}");
        let manifest = dir.join("project.yaml");
        let before = fs::read(&manifest).unwrap();
        let validated = Command::new(env!("CARGO_BIN_EXE_archy"))
            .args(["validate", "--json", "--project"])
            .arg(&manifest)
            .output()
            .unwrap();
        assert!(validated.status.success(), "{name}: {validated:?}");
        let value: serde_json::Value = serde_json::from_slice(&validated.stdout).unwrap();
        assert_eq!(value["id"].as_str(), Some(name));
        let collision = Command::new(env!("CARGO_BIN_EXE_archy"))
            .args(["init", &format!("--name={name}"), "--dir"])
            .arg(&dir)
            .output()
            .unwrap();
        assert!(!collision.status.success());
        assert_eq!(fs::read(manifest).unwrap(), before);
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn init_rejects_unsafe_ids_without_creating_target() {
    for name in [
        "bad/name",
        "bad name",
        "a\nproject_id: injected",
        "",
        &"a".repeat(129),
    ] {
        let dir = target();
        let out = Command::new(env!("CARGO_BIN_EXE_archy"))
            .args(["init", &format!("--name={name}"), "--dir"])
            .arg(&dir)
            .output()
            .unwrap();
        assert!(!out.status.success(), "{name:?}");
        assert!(!dir.exists(), "{name:?}");
    }
}
