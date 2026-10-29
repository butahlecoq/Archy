use std::{fs, process::Command, time::{SystemTime, UNIX_EPOCH}};
fn target() -> std::path::PathBuf { std::env::temp_dir().join(format!("archy-test-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())) }
#[test]
fn init_validates_and_refuses_overwrites() {
    let dir = target();
    let first = Command::new(env!("CARGO_BIN_EXE_archy")).args(["init","--name","courtyard-demo","--dir"]).arg(&dir).output().unwrap();
    assert!(first.status.success(), "{:?}", first);
    let before = fs::read(dir.join("project.yaml")).unwrap();
    let valid = Command::new(env!("CARGO_BIN_EXE_archy")).args(["validate","--json","--project"]).arg(dir.join("project.yaml")).output().unwrap();
    assert!(valid.status.success()); assert!(valid.stderr.is_empty());
    let v: serde_json::Value = serde_json::from_slice(&valid.stdout).unwrap(); assert_eq!(v["project_id"],"courtyard-demo");
    let second = Command::new(env!("CARGO_BIN_EXE_archy")).args(["init","--name","other","--dir"]).arg(&dir).output().unwrap();
    assert!(!second.status.success()); assert_eq!(fs::read(dir.join("project.yaml")).unwrap(),before);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn invalid_json_stdout_is_structured() {
    let dir = target(); fs::create_dir(&dir).unwrap(); let file = dir.join("project.json"); fs::write(&file,"{}").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_archy")).args(["validate","--json","--project"]).arg(file).output().unwrap();
    assert!(!out.status.success()); assert!(out.stderr.is_empty()); let v:serde_json::Value=serde_json::from_slice(&out.stdout).unwrap(); assert_eq!(v["code"],"INVALID_SCHEMA");
    fs::remove_dir_all(dir).unwrap();
}
