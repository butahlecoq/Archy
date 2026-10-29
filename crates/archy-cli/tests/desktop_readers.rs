use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

fn temp_project() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("archy-desktop-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/courtyard-housing/project.yaml"),
        dir.join("project.yaml"),
    )
    .unwrap();
    dir.join("project.yaml")
}
fn cli(args: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        output.status.code().unwrap(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}
fn inventory(root: &Path) -> BTreeMap<PathBuf, (u32, SystemTime, Option<Vec<u8>>)> {
    fn visit(path: &Path, out: &mut BTreeMap<PathBuf, (u32, SystemTime, Option<Vec<u8>>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let bytes = metadata.is_file().then(|| fs::read(&path).unwrap());
            out.insert(
                path.clone(),
                (
                    metadata.permissions().mode(),
                    metadata.modified().unwrap(),
                    bytes,
                ),
            );
            if metadata.is_dir() {
                visit(&path, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, &mut out);
    out
}
#[test]
fn capabilities_only_advertise_readers_and_genesis_status_creates_nothing() {
    let (exit, caps) = cli(&["desktop", "capabilities", "--json"]);
    assert_eq!(exit, 0);
    let ids: Vec<_> = caps["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|op| op["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["project_status", "run_status"]);
    let project = temp_project();
    let project_arg = project.to_str().unwrap();
    let before = fs::read(&project).unwrap();
    let (exit, status) = cli(&["desktop", "status", "--project", project_arg, "--json"]);
    assert_eq!(exit, 0);
    assert_eq!(status["project"]["revision"], 0);
    assert_eq!(status["runs"].as_array().unwrap().len(), 0);
    assert_eq!(status["backend_jobs"]["status"], "unknown");
    assert_eq!(fs::read(&project).unwrap(), before);
    assert_eq!(fs::read_dir(project.parent().unwrap()).unwrap().count(), 1);
}
#[test]
fn corrupt_and_symlink_history_refuse_without_repair() {
    let project = temp_project();
    let arg = project.to_str().unwrap();
    let archy = project.parent().unwrap().join(".archy");
    fs::create_dir(&archy).unwrap();
    fs::set_permissions(&archy, fs::Permissions::from_mode(0o700)).unwrap();
    let (exit, error) = cli(&["desktop", "status", "--project", arg, "--json"]);
    assert_ne!(exit, 0);
    assert_eq!(error["code"], "ARTIFACT_INVALID");
    assert_eq!(fs::read_dir(&archy).unwrap().count(), 0);
    fs::remove_dir(&archy).unwrap();
    std::os::unix::fs::symlink(project.parent().unwrap(), &archy).unwrap();
    let (exit, _) = cli(&["desktop", "status", "--project", arg, "--json"]);
    assert_ne!(exit, 0);
    assert!(
        fs::symlink_metadata(&archy)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn symlinked_project_source_is_refused_without_touching_target() {
    let project = temp_project();
    let target =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/courtyard-housing/project.yaml");
    fs::remove_file(&project).unwrap();
    std::os::unix::fs::symlink(&target, &project).unwrap();
    let before = inventory(project.parent().unwrap());
    let (exit, error) = cli(&[
        "desktop",
        "status",
        "--project",
        project.to_str().unwrap(),
        "--json",
    ]);
    assert_ne!(exit, 0);
    assert_eq!(error["code"], "PERMISSION_DENIED");
    assert_eq!(inventory(project.parent().unwrap()), before);
    assert!(!project.parent().unwrap().join(".archy").exists());
}

#[test]
fn accepted_pages_are_verified_and_stale_cursor_conflicts_without_writes() {
    let project = temp_project();
    let project_arg = project.to_str().unwrap();
    for seed in [42, 43] {
        archy_cli::generate(archy_cli::GenerateInput {
            project: project.clone(),
            mode: archy_cli::GenerationMode::Sample { count: 1, seed },
            mutation: archy_cli::MutationOptions::default(),
        })
        .unwrap();
    }
    let store = archy_core::ProjectStore::open(&project).unwrap();
    let lock = store.lock().unwrap();
    let before = inventory(project.parent().unwrap());
    let (exit, first) = cli(&[
        "desktop",
        "status",
        "--project",
        project_arg,
        "--limit",
        "1",
        "--json",
    ]);
    assert_eq!(exit, 0);
    assert_eq!(first["project"]["revision"], 2);
    assert_eq!(first["runs"].as_array().unwrap().len(), 1);
    assert_eq!(
        first["selected_run"]["artifacts"][0]["integrity"],
        "verified"
    );
    let cursor = first["next_cursor"].as_str().unwrap();
    let (exit, second) = cli(&[
        "desktop",
        "status",
        "--project",
        project_arg,
        "--limit",
        "1",
        "--cursor",
        cursor,
        "--json",
    ]);
    assert_eq!(exit, 0);
    assert_ne!(first["runs"][0]["run_id"], second["runs"][0]["run_id"]);
    let older = second["runs"][0]["run_id"].as_str().unwrap();
    let (exit, selected) = cli(&[
        "desktop",
        "status",
        "--project",
        project_arg,
        "--run",
        older,
        "--limit",
        "1",
        "--json",
    ]);
    assert_eq!(exit, 0);
    assert_eq!(selected["selected_run"]["summary"]["run_id"], older);
    assert_eq!(inventory(project.parent().unwrap()), before);
    drop(lock);
    archy_cli::generate(archy_cli::GenerateInput {
        project: project.clone(),
        mode: archy_cli::GenerationMode::Sample { count: 1, seed: 44 },
        mutation: archy_cli::MutationOptions::default(),
    })
    .unwrap();
    let before = inventory(project.parent().unwrap());
    let (exit, error) = cli(&[
        "desktop",
        "status",
        "--project",
        project_arg,
        "--cursor",
        cursor,
        "--json",
    ]);
    assert_ne!(exit, 0);
    assert_eq!(error["code"], "REVISION_CONFLICT");
    assert_eq!(inventory(project.parent().unwrap()), before);
}

#[test]
fn tampered_accepted_artifact_refuses_without_cleanup() {
    let project = temp_project();
    let receipt = archy_cli::generate(archy_cli::GenerateInput {
        project: project.clone(),
        mode: archy_cli::GenerationMode::Sample { count: 1, seed: 42 },
        mutation: archy_cli::MutationOptions::default(),
    })
    .unwrap();
    let report = project
        .parent()
        .unwrap()
        .join("runs")
        .join(receipt.run_id)
        .join("report.md");
    fs::write(&report, b"tampered").unwrap();
    let before = inventory(project.parent().unwrap());
    let (exit, error) = cli(&[
        "desktop",
        "status",
        "--project",
        project.to_str().unwrap(),
        "--json",
    ]);
    assert_ne!(exit, 0);
    assert_eq!(error["code"], "ARTIFACT_INVALID");
    assert_eq!(inventory(project.parent().unwrap()), before);
}

#[test]
fn rehashed_false_evaluation_summary_never_becomes_desktop_truth() {
    let project = temp_project();
    let receipt = archy_cli::generate(archy_cli::GenerateInput {
        project: project.clone(),
        mode: archy_cli::GenerationMode::Sample { count: 1, seed: 42 },
        mutation: archy_cli::MutationOptions::default(),
    })
    .unwrap();
    let root = project.parent().unwrap();
    let run_dir = root.join("runs").join(&receipt.run_id);
    let evaluation_path = format!("evaluations/{}.json", receipt.variant_ids[0]);
    let evaluation_file = run_dir.join(&evaluation_path);
    let original = String::from_utf8(fs::read(&evaluation_file).unwrap()).unwrap();
    let (needle, replacement) = if original.contains("\"required_checks_passed\":true") {
        (
            "\"required_checks_passed\":true",
            "\"required_checks_passed\":false",
        )
    } else {
        (
            "\"required_checks_passed\":false",
            "\"required_checks_passed\":true",
        )
    };
    assert!(original.contains(needle));
    let bytes = original.replacen(needle, replacement, 1).into_bytes();
    fs::write(&evaluation_file, &bytes).unwrap();
    let manifest_file = run_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_file).unwrap()).unwrap();
    let artifacts = manifest["artifacts"].as_array_mut().unwrap();
    let entry = artifacts
        .iter_mut()
        .find(|a| a["path"] == evaluation_path)
        .unwrap();
    entry["sha256"] = Value::String(archy_core::sha256_hex(&bytes));
    entry["byte_len"] = Value::from(bytes.len());
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(&manifest_file, &manifest_bytes).unwrap();
    let revision_file = root.join(".archy/revisions/00000000000000000001.json");
    let mut revision: Value = serde_json::from_slice(&fs::read(&revision_file).unwrap()).unwrap();
    revision["receipt"]["manifest_sha256"] = Value::String(archy_core::sha256_hex(&manifest_bytes));
    fs::write(&revision_file, serde_json::to_vec(&revision).unwrap()).unwrap();
    let before = inventory(root);
    let (exit, error) = cli(&[
        "desktop",
        "status",
        "--project",
        project.to_str().unwrap(),
        "--json",
    ]);
    assert_ne!(exit, 0);
    assert_eq!(error["code"], "ARTIFACT_INVALID");
    assert_eq!(inventory(root), before);
}
