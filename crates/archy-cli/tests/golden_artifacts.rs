use archy_cli::verify_run;
use archy_core::decision_contract::{ConstraintEvidence, ReviewEvidence};
use archy_core::{ErrorCode, ProjectStore, RevisionRecord, RunManifest, sha256_hex};
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

fn assert_tree_bytes_match(saved: &Path, copy: &Path) {
    let mut saved_entries: Vec<_> = fs::read_dir(saved)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    let mut copy_entries: Vec<_> = fs::read_dir(copy)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    if copy_entries.iter().any(|name| name == "staging")
        && !saved_entries.iter().any(|name| name == "staging")
    {
        let staging = copy.join("staging");
        assert!(staging.is_dir());
        assert_eq!(fs::read_dir(staging).unwrap().count(), 0);
        copy_entries.retain(|name| name != "staging");
    }
    saved_entries.sort();
    copy_entries.sort();
    assert_eq!(copy_entries, saved_entries);
    for name in saved_entries {
        let original = saved.join(&name);
        let copied = copy.join(&name);
        if original.is_dir() {
            assert_tree_bytes_match(&original, &copied);
        } else {
            assert_eq!(
                fs::read(&copied).unwrap(),
                fs::read(&original).unwrap(),
                "{}",
                original.display()
            );
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
        let relative = format!("variants/{variant_id}.json");
        assert_eq!(
            fs::read(actual.join(&relative)).unwrap(),
            fs::read(saved.join(relative)).unwrap()
        );
        let relative = format!("evaluations/{variant_id}.json");
        let mut current: Value =
            serde_json::from_slice(&fs::read(actual.join(&relative)).unwrap()).unwrap();
        let mut historical: Value =
            serde_json::from_slice(&fs::read(saved.join(relative)).unwrap()).unwrap();
        assert_eq!(current["evaluation_version"], "planar-massing-authority-v2");
        assert_eq!(historical["evaluation_version"], "planar-massing-v1");
        current
            .as_object_mut()
            .unwrap()
            .remove("evaluation_version");
        historical
            .as_object_mut()
            .unwrap()
            .remove("evaluation_version");
        assert_eq!(current, historical);
    }
}

fn assert_report(actual: &Path, saved: &Path) {
    let actual_json: Value =
        serde_json::from_slice(&fs::read(actual.join("report.json")).unwrap()).unwrap();
    assert_eq!(actual_json["schema_version"], "archy-report-v2");
    assert_eq!(actual_json["spec"]["schema_version"], "0.2.0");
    assert_eq!(actual_json["spec"]["id"], "courtyard-demo");
    assert_eq!(actual_json["spec"]["constraints"][0]["unit"], "m");
    assert!(
        fs::read_to_string(actual.join("report.md"))
            .unwrap()
            .contains("uncomputed and do not affect generation")
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
    let baseline = root.join("baseline");
    let saved_copy = root.join("saved");
    copy_tree(&fixture, &baseline);
    copy_tree(&fixture, &saved_copy);
    assert_tree_bytes_match(&baseline, &fixture);
    assert_tree_bytes_match(&baseline, &saved_copy);
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
        assert_eq!(
            verify_run(&saved_project, receipt_id(&receipts[operation]))
                .unwrap()
                .spec()
                .normalization_version,
            "si-rectangle-v1"
        );
    }
    let lock = saved_copy.join(".archy/lock");
    assert!(fs::symlink_metadata(&lock).unwrap().file_type().is_file());
    assert!(fs::read(&lock).unwrap().is_empty());
    fs::remove_file(lock).unwrap();
    let staging = saved_copy.join(".archy/staging");
    assert!(fs::symlink_metadata(&staging).unwrap().file_type().is_dir());
    fs::remove_dir(staging).unwrap();
    assert_tree_bytes_match(&baseline, &fixture);
    assert_tree_bytes_match(&baseline, &saved_copy);

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
        let saved_evaluation_bytes =
            fs::read(saved_generate.join(format!("evaluations/{variant_id}.json"))).unwrap();
        assert_eq!(
            sha256_hex(&saved_evaluation_bytes),
            design["evaluation_sha256"]
        );
        let evaluation: Value = serde_json::from_slice(&evaluation_bytes).unwrap();
        assert_eq!(
            evaluation["evaluation_version"],
            "planar-massing-authority-v2"
        );
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
    for (name, receipt) in runs {
        let actual_run = run_path(&first_project, receipt);
        let saved_run = run_path(&saved_project, &receipts[name]);
        assert_design_bytes(
            &actual_run,
            &saved_run,
            receipt["variant_ids"].as_array().unwrap(),
        );
        assert_report(&actual_run, &saved_run);
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

#[test]
fn pre_a04_claimed_marker_chain_verifies_and_replays_exact_saved_bytes() {
    const GENERATE: &str = "e80391ef-a7ec-4900-8d05-9409b250094e";
    const EVALUATE: &str = "a112afe0-ab7b-414d-8f35-11d5f97814c9";
    const VARIANT: &str =
        "variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b";
    let fixture = repository_path("tests/fixtures/historical-claimed-v1");
    let root = temporary_root();
    let saved = root.join("saved");
    copy_tree(&fixture, &saved);
    make_managed_directories_private(&saved.join(".archy"));
    make_managed_directories_private(&saved.join("runs"));
    assert_tree_bytes_match(&fixture, &saved);
    let project = saved.join("project.json");

    for (run_id, report_hash) in [
        (
            GENERATE,
            "cfef835ba3545371dd171b190f7f6b648f78d8119ac32a7727a2284eef0cd655",
        ),
        (
            EVALUATE,
            "935bf86e70f372f341552fdf6fce60029d8402053678b6eb29c1592da83737a2",
        ),
    ] {
        let verified = verify_run(&project, run_id).unwrap();
        assert_eq!(verified.spec().constraints[0].source, "claimed_regulation");
        let evaluation = &verified.evaluations()[0];
        assert_eq!(evaluation.evaluation_version, "planar-massing-v1");
        assert!(evaluation.required_checks_passed);
        assert!(
            evaluation
                .checks
                .iter()
                .all(|check| !check.constraint_id.starts_with("authority:"))
        );
        let run = saved.join("runs").join(run_id);
        let evaluation_bytes = fs::read(run.join(format!("evaluations/{VARIANT}.json"))).unwrap();
        assert_eq!(
            sha256_hex(&evaluation_bytes),
            "9a7d6181a2b63e5facb6a1886914e60188e1a95e03c78569c4f275dadbed348d"
        );
        let report_bytes = fs::read(run.join("report.json")).unwrap();
        assert_eq!(sha256_hex(&report_bytes), report_hash);
        let report: Value = serde_json::from_slice(&report_bytes).unwrap();
        assert_eq!(report["schema_version"], "archy-report-v1");
    }
    assert_eq!(
        ProjectStore::open(&project)
            .unwrap()
            .read_history()
            .unwrap()
            .revision(),
        2
    );

    let old = verify_run(&project, GENERATE).unwrap();
    let normalized = fs::read(saved.join("runs").join(GENERATE).join("normalized.json")).unwrap();
    let variant_bytes = fs::read(
        saved
            .join("runs")
            .join(GENERATE)
            .join(format!("variants/{VARIANT}.json")),
    )
    .unwrap();
    let evaluation_bytes = fs::read(
        saved
            .join("runs")
            .join(GENERATE)
            .join(format!("evaluations/{VARIANT}.json")),
    )
    .unwrap();
    let evidence = ReviewEvidence {
        project_id: old.spec().project_id.clone(),
        variant_id: VARIANT.into(),
        source_run_id: GENERATE.into(),
        expected_project_revision: 2,
        expected_head_sha256: ProjectStore::open(&project)
            .unwrap()
            .read_history()
            .unwrap()
            .head_sha256()
            .unwrap()
            .into(),
        input_sha256: sha256_hex(old.input()),
        normalized_sha256: sha256_hex(&normalized),
        variant_sha256: sha256_hex(&variant_bytes),
        evaluation_sha256: sha256_hex(&evaluation_bytes),
        comparison_sha256: "a".repeat(64),
        constraints: old
            .spec()
            .constraints
            .iter()
            .map(ConstraintEvidence::from_normalized)
            .collect(),
        checks: old.evaluations()[0].checks.clone(),
    };
    assert!(
        !evidence
            .claimed_checks_passed_against(&normalized, &variant_bytes, &evaluation_bytes)
            .unwrap()
    );

    let compared = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(["compare", "--run", GENERATE])
        .arg("--project")
        .arg(&project)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!compared.status.success());
    let compare_error: Value = serde_json::from_slice(&compared.stdout).unwrap();
    assert_eq!(compare_error["code"], "INVALID_SCHEMA");
    assert!(
        compare_error["message"]
            .as_str()
            .unwrap()
            .contains("authority verification")
    );

    let generation = command(
        &project,
        &[
            "generate",
            "--count",
            "1",
            "--seed",
            "42",
            "--idempotency-key",
            "historical-marker-generate-7728da0",
            "--expected-project-revision",
            "0",
        ],
    );
    assert_eq!(receipt_id(&generation), GENERATE);
    assert_eq!(generation["revision"], 1);
    let evaluated = command(
        &project,
        &[
            "evaluate",
            "--run",
            GENERATE,
            "--all",
            "--idempotency-key",
            "historical-marker-evaluate-7728da0",
            "--expected-project-revision",
            "1",
        ],
    );
    assert_eq!(receipt_id(&evaluated), EVALUATE);
    assert_eq!(evaluated["revision"], 2);
    assert_eq!(
        ProjectStore::open(&project)
            .unwrap()
            .read_history()
            .unwrap()
            .revision(),
        2
    );
    assert_tree_bytes_match(&fixture, &saved);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_children_of_authentic_claimed_v1_history_migrate_authority_profile() {
    const GENERATE: &str = "e80391ef-a7ec-4900-8d05-9409b250094e";
    const VARIANT: &str =
        "variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b";
    let fixture = repository_path("tests/fixtures/historical-claimed-v1");
    for operation in ["evaluate", "evaluate-default", "edit"] {
        let root = temporary_root();
        let saved = root.join("saved");
        copy_tree(&fixture, &saved);
        make_managed_directories_private(&saved.join(".archy"));
        make_managed_directories_private(&saved.join("runs"));
        let project = saved.join("project.json");
        let arguments = if operation == "evaluate" {
            vec![
                "evaluate",
                "--run",
                GENERATE,
                "--all",
                "--idempotency-key",
                "new-evaluation",
                "--expected-project-revision",
                "2",
            ]
        } else if operation == "evaluate-default" {
            vec![
                "evaluate",
                "--all",
                "--idempotency-key",
                "new-default-evaluation",
                "--expected-project-revision",
                "2",
            ]
        } else {
            let variant: Value = serde_json::from_slice(
                &fs::read(
                    saved
                        .join("runs")
                        .join(GENERATE)
                        .join(format!("variants/{VARIANT}.json")),
                )
                .unwrap(),
            )
            .unwrap();
            fs::write(saved.join("edit.json"), serde_json::to_vec(&serde_json::json!({"parameters":variant["parameters"],"storeys":2,"floor_to_floor_m":variant["floor_to_floor_m"]})).unwrap()).unwrap();
            vec![
                "generate",
                "--from-run",
                GENERATE,
                "--variant",
                VARIANT,
                "--params",
                "edit.json",
                "--idempotency-key",
                "new-edit",
                "--expected-project-revision",
                "2",
            ]
        };
        let receipt = command(&project, &arguments);
        assert_eq!(receipt["revision"], 3, "{operation}");
        let child = verify_run(&project, receipt_id(&receipt)).unwrap();
        assert_eq!(child.manifest().parent_run_id.as_deref(), Some(GENERATE));
        assert_eq!(
            child.input(),
            fs::read(fixture.join("project.json")).unwrap()
        );
        assert_eq!(
            serde_json::to_value(child.canonical().unwrap()).unwrap()["normalization_version"],
            "si-rectangle-v2"
        );
        assert_eq!(
            child.evaluations()[0].evaluation_version,
            "planar-massing-authority-v2"
        );
        assert!(!child.evaluations()[0].required_checks_passed);
        let authority = child.evaluations()[0]
            .checks
            .iter()
            .find(|c| c.constraint_id == "authority:height")
            .unwrap();
        assert_eq!(authority.result, archy_core::CheckResult::Unknown);
        assert_eq!((authority.measured, authority.limit), (None, None));
        assert!(run_path(&project, &receipt).join("profile.json").is_file());
        assert_eq!(
            receipt_id(&command(&project, &arguments)),
            receipt_id(&receipt)
        );
        if operation == "edit" {
            let compared = command(&project, &["compare", "--run", receipt_id(&receipt)]);
            assert_eq!(
                compared["candidates"][0]["evaluation"]["required_checks_passed"],
                false
            );
            let evaluated = command(
                &project,
                &[
                    "evaluate",
                    "--run",
                    receipt_id(&receipt),
                    "--all",
                    "--idempotency-key",
                    "migrated-edit-evaluation",
                    "--expected-project-revision",
                    "3",
                ],
            );
            let evaluated_run = verify_run(&project, receipt_id(&evaluated)).unwrap();
            assert_eq!(
                evaluated_run.manifest().parent_run_id.as_deref(),
                Some(receipt_id(&receipt))
            );
            assert_eq!(
                evaluated_run.evaluations()[0].evaluation_version,
                "planar-massing-authority-v2"
            );
            assert!(!evaluated_run.evaluations()[0].required_checks_passed);
            let original: Value = serde_json::from_slice(
                &fs::read(
                    fixture
                        .join("runs")
                        .join(GENERATE)
                        .join(format!("variants/{VARIANT}.json")),
                )
                .unwrap(),
            )
            .unwrap();
            fs::write(saved.join("restore.json"), serde_json::to_vec(&serde_json::json!({"parameters":original["parameters"],"storeys":original["storeys"],"floor_to_floor_m":original["floor_to_floor_m"]})).unwrap()).unwrap();
            let edited_variant = receipt["variant_ids"][0].as_str().unwrap();
            let restored = command(
                &project,
                &[
                    "generate",
                    "--from-run",
                    receipt_id(&receipt),
                    "--variant",
                    edited_variant,
                    "--params",
                    "restore.json",
                    "--idempotency-key",
                    "migrated-restore",
                    "--expected-project-revision",
                    "4",
                ],
            );
            let restored_run = verify_run(&project, receipt_id(&restored)).unwrap();
            assert_eq!(
                restored_run.manifest().parent_run_id.as_deref(),
                Some(receipt_id(&receipt))
            );
            assert_eq!(
                restored_run.evaluations()[0].evaluation_version,
                "planar-massing-authority-v2"
            );
            assert!(!restored_run.evaluations()[0].required_checks_passed);
            fs::remove_file(saved.join("restore.json")).unwrap();
        }
        assert_eq!(
            ProjectStore::open(&project)
                .unwrap()
                .read_history()
                .unwrap()
                .revision(),
            if operation == "edit" { 5 } else { 3 }
        );
        if operation == "edit" {
            fs::remove_file(saved.join("edit.json")).unwrap();
        }
        assert_tree_bytes_match(
            &fixture.join("runs").join(GENERATE),
            &saved.join("runs").join(GENERATE),
        );
        assert_tree_bytes_match(
            &fixture
                .join("runs")
                .join("a112afe0-ab7b-414d-8f35-11d5f97814c9"),
            &saved
                .join("runs")
                .join("a112afe0-ab7b-414d-8f35-11d5f97814c9"),
        );
        for revision in ["00000000000000000001.json", "00000000000000000002.json"] {
            assert_eq!(
                fs::read(fixture.join(".archy/revisions").join(revision)).unwrap(),
                fs::read(saved.join(".archy/revisions").join(revision)).unwrap()
            );
        }
        if operation == "evaluate" {
            let run = run_path(&project, &receipt);
            let profile_path = run.join("profile.json");
            let mut profile: Value =
                serde_json::from_slice(&fs::read(&profile_path).unwrap()).unwrap();
            profile["evaluation_version"] = "planar-massing-v1".into();
            let profile_bytes = serde_json::to_vec(&profile).unwrap();
            fs::write(&profile_path, &profile_bytes).unwrap();
            let manifest_path = run.join("manifest.json");
            let mut manifest: RunManifest =
                serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
            let entry = manifest
                .artifacts
                .iter_mut()
                .find(|artifact| artifact.path == "profile.json")
                .unwrap();
            entry.sha256 = sha256_hex(&profile_bytes);
            entry.byte_len = profile_bytes.len() as u64;
            let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
            fs::write(&manifest_path, &manifest_bytes).unwrap();
            let revision_path = saved.join(".archy/revisions/00000000000000000003.json");
            let mut revision: RevisionRecord =
                serde_json::from_slice(&fs::read(&revision_path).unwrap()).unwrap();
            revision.receipt.manifest_sha256 = sha256_hex(&manifest_bytes);
            fs::write(&revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();
            assert_eq!(
                verify_run(&project, receipt_id(&receipt)).unwrap_err().code,
                ErrorCode::ArtifactInvalid
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn non_claimed_historical_v1_can_still_be_evaluated_and_compared() {
    const RUN: &str = "5248af90-8dfe-4743-b056-7d3385c62c97";
    let fixture = repository_path("tests/fixtures/legacy-preference-run");
    let root = temporary_root();
    let saved = root.join("saved");
    fs::create_dir(&saved).unwrap();
    fs::copy(fixture.join("project.json"), saved.join("project.json")).unwrap();
    copy_tree(&fixture.join(".archy"), &saved.join(".archy"));
    copy_tree(&fixture.join("archived-runs"), &saved.join("runs"));
    make_managed_directories_private(&saved.join(".archy"));
    make_managed_directories_private(&saved.join("runs"));
    let project = saved.join("project.json");
    let evaluated = command(
        &project,
        &[
            "evaluate",
            "--run",
            RUN,
            "--all",
            "--idempotency-key",
            "new-normal-v1",
            "--expected-project-revision",
            "1",
        ],
    );
    let verified = verify_run(&project, receipt_id(&evaluated)).unwrap();
    assert_eq!(
        verified.evaluations()[0].evaluation_version,
        "planar-massing-v1"
    );
    assert!(verified.evaluations()[0].required_checks_passed);
    let compared = command(&project, &["compare", "--run", RUN]);
    assert_eq!(
        compared["candidates"][0]["evaluation"]["required_checks_passed"],
        true
    );
    fs::remove_dir_all(root).unwrap();
}
