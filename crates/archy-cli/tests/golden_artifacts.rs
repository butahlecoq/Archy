use archy_cli::verify_run;
use archy_core::{ErrorCode, RunManifest, sha256_hex};
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

fn repository_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temporary_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("archy-golden-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    root
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    fs::set_permissions(destination, fs::Permissions::from_mode(0o755)).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            assert!(
                entry.file_type().unwrap().is_file(),
                "unexpected fixture entry"
            );
            fs::copy(source_path, destination_path).unwrap();
            fs::set_permissions(
                destination.join(entry.file_name()),
                fs::Permissions::from_mode(0o644),
            )
            .unwrap();
        }
    }
}

fn make_managed_directories_private(directory: &Path) {
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            make_managed_directories_private(&entry.path());
        }
    }
}

fn command(project: &Path, arguments: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(arguments)
        .arg("--project")
        .arg(project)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "CLI failed: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn receipt_id(receipt: &Value) -> &str {
    receipt["run_id"].as_str().unwrap()
}

fn run_path(project: &Path, receipt: &Value) -> PathBuf {
    project
        .parent()
        .unwrap()
        .join(receipt["path"].as_str().unwrap())
}

fn assert_inventory(project: &Path, receipt: &Value, saved_inventory: &Value) {
    let run = run_path(project, receipt);
    let manifest_bytes = fs::read(run.join("manifest.json")).unwrap();
    let manifest: RunManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest.run_id, receipt_id(receipt));
    assert_eq!(manifest.operation, saved_inventory["operation"]);
    assert_eq!(
        sha256_hex(&manifest_bytes),
        saved_inventory["manifest_sha256"]
    );
    let expected_artifacts = saved_inventory["artifacts"].as_array().unwrap();
    assert_eq!(manifest.artifacts.len(), expected_artifacts.len());
    for (artifact, expected) in manifest.artifacts.iter().zip(expected_artifacts) {
        assert_eq!(artifact.path, expected["path"]);
        assert_eq!(artifact.sha256, expected["sha256"]);
        assert_eq!(artifact.byte_len, expected["byte_len"]);
        let bytes = fs::read(run.join(&artifact.path)).unwrap();
        assert_eq!(sha256_hex(&bytes), artifact.sha256);
        assert_eq!(bytes.len() as u64, artifact.byte_len);
    }
    assert_eq!(
        verify_run(project, receipt_id(receipt)).unwrap().manifest(),
        &manifest
    );
}

fn assert_design_bytes(actual: &Path, saved: &Path, variant_ids: &[Value]) {
    for variant_id in variant_ids {
        let variant_id = variant_id.as_str().unwrap();
        for folder in ["variants", "evaluations"] {
            let relative = format!("{folder}/{variant_id}.json");
            assert_eq!(
                fs::read(actual.join(&relative)).unwrap(),
                fs::read(saved.join(relative)).unwrap()
            );
        }
    }
}

fn assert_report(actual: &Path, saved: &Path, dynamic_ids: &[(&str, &str)]) {
    let mut actual_json = String::from_utf8(fs::read(actual.join("report.json")).unwrap()).unwrap();
    for (dynamic, saved) in dynamic_ids {
        for field in ["run_id", "parent_run_id"] {
            actual_json = actual_json.replace(
                &format!("\"{field}\": \"{dynamic}\""),
                &format!("\"{field}\": \"{saved}\""),
            );
        }
    }
    assert!(
        actual_json.as_bytes() == fs::read(saved.join("report.json")).unwrap(),
        "report JSON differs after run ID fields are mapped"
    );
    let mut actual_markdown = fs::read_to_string(actual.join("report.md")).unwrap();
    for (dynamic, saved) in dynamic_ids {
        actual_markdown =
            actual_markdown.replace(&dynamic.replace('-', "\\-"), &saved.replace('-', "\\-"));
    }
    assert!(
        actual_markdown.as_bytes() == fs::read(saved.join("report.md")).unwrap(),
        "Markdown differs after execution IDs are mapped"
    );
    assert_eq!(
        fs::read(actual.join("events.jsonl")).unwrap(),
        fs::read(saved.join("events.jsonl")).unwrap()
    );
}

#[test]
fn curated_run_chain_and_fresh_cli_workflow_match_golden_evidence() {
    let evidence: Value =
        serde_json::from_slice(&fs::read(repository_path("tests/golden/evidence.json")).unwrap())
            .unwrap();
    let fixture = repository_path("examples/courtyard-housing/expected/accepted-project");
    let source = repository_path("examples/courtyard-housing/project.yaml");
    assert_eq!(
        fs::read(fixture.join("project.yaml")).unwrap(),
        fs::read(&source).unwrap()
    );
    assert_eq!(
        sha256_hex(&fs::read(&source).unwrap()),
        evidence["source_sha256"]
    );
    let root = temporary_root();
    let saved_copy = root.join("saved");
    copy_tree(&fixture, &saved_copy);
    let saved_project = saved_copy.join("project.yaml");
    let receipts = &evidence["capture_receipts"];
    assert_eq!(
        fs::metadata(saved_copy.join(".archy"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(saved_copy.join("runs"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(saved_copy.join("project.yaml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
    assert_eq!(
        verify_run(&saved_project, receipt_id(&receipts["generate"]))
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    let original_archy_mode = fs::metadata(fixture.join(".archy"))
        .unwrap()
        .permissions()
        .mode();
    make_managed_directories_private(&saved_copy.join(".archy"));
    make_managed_directories_private(&saved_copy.join("runs"));
    assert_eq!(
        fs::metadata(saved_copy.join(".archy"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(saved_copy.join("runs"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(fixture.join(".archy"))
            .unwrap()
            .permissions()
            .mode(),
        original_archy_mode
    );
    for operation in [
        "generate",
        "evaluate_one",
        "evaluate_all",
        "edit",
        "restore",
    ] {
        assert_inventory(
            &saved_project,
            &receipts[operation],
            &evidence["accepted_runs"][operation],
        );
    }

    let first_root = root.join("first");
    let second_root = root.join("second");
    fs::create_dir(&first_root).unwrap();
    fs::create_dir(&second_root).unwrap();
    let first_project = first_root.join("project.yaml");
    let second_project = second_root.join("project.yaml");
    fs::copy(&source, &first_project).unwrap();
    fs::copy(&source, &second_project).unwrap();
    let first = command(
        &first_project,
        &["generate", "--count", "3", "--seed", "42"],
    );
    let repeat = command(
        &second_project,
        &["generate", "--count", "3", "--seed", "42"],
    );
    assert_ne!(receipt_id(&first), receipt_id(&repeat));
    assert_eq!(first["revision"], 1);
    assert_eq!(repeat["revision"], 1);
    assert_eq!(first["variant_ids"], repeat["variant_ids"]);
    assert_eq!(first["variant_ids"], receipts["generate"]["variant_ids"]);
    assert_eq!(first["all_required_checks_passed"], true);
    let saved_generate = run_path(&saved_project, &receipts["generate"]);
    let first_generate = run_path(&first_project, &first);
    let repeat_generate = run_path(&second_project, &repeat);
    let variant_ids = first["variant_ids"].as_array().unwrap();
    assert_eq!(variant_ids.len(), 3);
    assert_design_bytes(&first_generate, &saved_generate, variant_ids);
    assert_design_bytes(&repeat_generate, &saved_generate, variant_ids);
    assert_eq!(
        fs::read(first_generate.join("generation.json")).unwrap(),
        fs::read(saved_generate.join("generation.json")).unwrap()
    );
    assert_eq!(
        fs::read(repeat_generate.join("generation.json")).unwrap(),
        fs::read(saved_generate.join("generation.json")).unwrap()
    );
    for design in evidence["deterministic_design"].as_array().unwrap() {
        let variant_id = design["variant_id"].as_str().unwrap();
        let variant_bytes =
            fs::read(first_generate.join(format!("variants/{variant_id}.json"))).unwrap();
        let evaluation_bytes =
            fs::read(first_generate.join(format!("evaluations/{variant_id}.json"))).unwrap();
        assert_eq!(sha256_hex(&variant_bytes), design["variant_sha256"]);
        assert_eq!(sha256_hex(&evaluation_bytes), design["evaluation_sha256"]);
        let evaluation: Value = serde_json::from_slice(&evaluation_bytes).unwrap();
        let measured: serde_json::Map<String, Value> = evaluation["metrics"].as_array().unwrap().iter().map(|metric| {
            (metric["name"].as_str().unwrap().to_owned(), serde_json::json!({"value": metric["value"], "unit": metric["unit"], "method_version": metric["method_version"]}))
        }).collect();
        assert_eq!(Value::Object(measured), design["metrics"]);
        assert_eq!(
            evaluation["required_checks_passed"],
            design["required_checks_passed"]
        );
    }
    let original_manifest = fs::read(first_generate.join("manifest.json")).unwrap();
    let original_files: Vec<_> = evidence["accepted_runs"]["generate"]["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|artifact| {
            let path = artifact["path"].as_str().unwrap().to_owned();
            let bytes = fs::read(first_generate.join(&path)).unwrap();
            (path, bytes)
        })
        .collect();
    let selected_id = variant_ids[0].as_str().unwrap();
    let one = command(
        &first_project,
        &[
            "evaluate",
            "--variant",
            selected_id,
            "--run",
            receipt_id(&first),
        ],
    );
    let all = command(&first_project, &["evaluate", "--all"]);
    assert_eq!(one["variant_ids"].as_array().unwrap().len(), 1);
    assert_eq!(all["variant_ids"], first["variant_ids"]);
    let original_variant: Value = serde_json::from_slice(
        &fs::read(first_generate.join(format!("variants/{selected_id}.json"))).unwrap(),
    )
    .unwrap();
    let original_parameters = serde_json::json!({"parameters": original_variant["parameters"], "storeys": original_variant["storeys"], "floor_to_floor_m": original_variant["floor_to_floor_m"]});
    let mut edited_parameters = original_parameters.clone();
    edited_parameters["parameters"]["width_m"] =
        serde_json::json!(original_variant["parameters"]["width_m"].as_f64().unwrap() + 0.25);
    fs::write(
        first_root.join("edit.json"),
        serde_json::to_vec(&edited_parameters).unwrap(),
    )
    .unwrap();
    let edit = command(
        &first_project,
        &[
            "generate",
            "--from-run",
            receipt_id(&first),
            "--variant",
            selected_id,
            "--params",
            "edit.json",
        ],
    );
    assert_ne!(edit["variant_ids"][0], selected_id);
    fs::write(
        first_root.join("restore.json"),
        serde_json::to_vec(&original_parameters).unwrap(),
    )
    .unwrap();
    let restore = command(
        &first_project,
        &[
            "generate",
            "--from-run",
            receipt_id(&edit),
            "--variant",
            edit["variant_ids"][0].as_str().unwrap(),
            "--params",
            "restore.json",
        ],
    );
    assert_eq!(restore["variant_ids"][0], selected_id);
    let runs = [
        ("generate", &first),
        ("evaluate_one", &one),
        ("evaluate_all", &all),
        ("edit", &edit),
        ("restore", &restore),
    ];
    let dynamic_ids: Vec<_> = runs
        .iter()
        .map(|(name, receipt)| (receipt_id(receipt), receipt_id(&receipts[*name])))
        .collect();
    for (name, receipt) in runs {
        let actual_run = run_path(&first_project, receipt);
        let saved_run = run_path(&saved_project, &receipts[name]);
        assert_design_bytes(
            &actual_run,
            &saved_run,
            receipt["variant_ids"].as_array().unwrap(),
        );
        assert_report(&actual_run, &saved_run, &dynamic_ids);
        assert_eq!(
            verify_run(&first_project, receipt_id(receipt))
                .unwrap()
                .manifest()
                .operation,
            evidence["accepted_runs"][name]["operation"]
        );
    }
    assert_eq!(
        fs::read(first_generate.join("manifest.json")).unwrap(),
        original_manifest
    );
    for (relative, bytes) in original_files {
        assert_eq!(fs::read(first_generate.join(relative)).unwrap(), bytes);
    }
    fs::remove_dir_all(root).unwrap();
}
