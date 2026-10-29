use archy_cli::{
    EvaluateInput, GenerateInput, GenerationMode, MutationOptions, VariantSelection,
    WorkflowReceipt,
};
use archy_core::{
    ArchyError, ErrorCode, InputFormat, LockedProject, ProjectStore, RevisionRecord, RunManifest,
    parse_project_v1, sha256_hex,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("archy-cli-workflow-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.json");
    fs::write(
        &project,
        include_bytes!("../../../tests/fixtures/briefs/courtyard.json"),
    )
    .unwrap();
    (root, project)
}

fn retry_project_lock<T>(
    mut operation: impl FnMut() -> Result<T, ArchyError>,
) -> Result<T, ArchyError> {
    for attempt in 0..8 {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error)
                if attempt < 7
                    && error.code == ErrorCode::RevisionConflict
                    && error.retriable
                    && error.field == "storage"
                    && error.message == "project lock held by another process" =>
            {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("bounded loop returns on the final attempt")
}

fn generate(input: GenerateInput) -> Result<WorkflowReceipt, ArchyError> {
    retry_project_lock(|| archy_cli::generate(input.clone()))
}

fn evaluate(input: EvaluateInput) -> Result<WorkflowReceipt, ArchyError> {
    retry_project_lock(|| archy_cli::evaluate(input.clone()))
}

fn verify_run(project: &Path, run_id: &str) -> Result<archy_cli::TrustedRun, ArchyError> {
    retry_project_lock(|| archy_cli::verify_run(project, run_id))
}

fn lock_project(store: &ProjectStore) -> Result<LockedProject<'_>, ArchyError> {
    retry_project_lock(|| store.lock())
}

fn with_locked<T>(project: &Path, read: impl FnOnce(&LockedProject<'_>) -> T) -> T {
    let store = ProjectStore::open(project).unwrap();
    let locked = lock_project(&store).unwrap();
    read(&locked)
}

#[test]
fn seeded_runs_keep_identical_design_bytes_and_distinct_revisions() {
    let (root, project) = fixture();
    let options = GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 3, seed: 42 },
        mutation: MutationOptions::default(),
    };
    let first = generate(options.clone()).unwrap();
    let second = generate(options).unwrap();
    assert_eq!((first.revision, second.revision), (1, 2));
    assert_ne!(first.run_id, second.run_id);
    assert_eq!(first.variant_ids, second.variant_ids);
    let left = verify_run(&project, &first.run_id).unwrap();
    let right = verify_run(&project, &second.run_id).unwrap();
    assert_eq!(left.variants(), right.variants());
    assert_eq!(left.evaluations(), right.evaluations());
    let store = ProjectStore::open(&project).unwrap();
    let locked = lock_project(&store).unwrap();
    let a = locked.read_run(&first.run_id).unwrap();
    let b = locked.read_run(&second.run_id).unwrap();
    for id in &first.variant_ids {
        for path in [
            format!("variants/{id}.json"),
            format!("evaluations/{id}.json"),
        ] {
            assert_eq!(a.artifact(&path), b.artifact(&path));
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn evaluate_uses_selected_immutable_run_after_current_project_edit() {
    let (root, project) = fixture();
    let generated = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 3, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    fs::write(
        &project,
        b"changed current file; source run remains immutable",
    )
    .unwrap();
    let one = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(generated.run_id.clone()),
        selection: VariantSelection::One(generated.variant_ids[0].clone()),
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let all = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: None,
        selection: VariantSelection::All,
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_eq!(one.variant_ids.len(), 1);
    assert_eq!(all.variant_ids, generated.variant_ids);
    assert_eq!(
        verify_run(&project, &one.run_id)
            .unwrap()
            .manifest()
            .parent_run_id
            .as_deref(),
        Some(generated.run_id.as_str())
    );
    assert_eq!(
        verify_run(&project, &all.run_id)
            .unwrap()
            .manifest()
            .parent_run_id
            .as_deref(),
        Some(generated.run_id.as_str())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_mutation_replays_and_rejects_stale_or_changed_requests() {
    let (root, project) = fixture();
    let explicit = MutationOptions {
        idempotency_key: Some("stable-key".into()),
        expected_project_revision: Some(0),
        destination: "runs".into(),
    };
    let first = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: explicit.clone(),
    })
    .unwrap();
    let _later = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 43 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let replay = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: explicit.clone(),
    })
    .unwrap();
    assert_eq!(
        (replay.run_id, replay.revision),
        (first.run_id, first.revision)
    );
    let changed = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 44 },
        mutation: explicit,
    })
    .unwrap_err();
    assert_eq!(changed.code, ErrorCode::RevisionConflict);
    let stale = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions {
            idempotency_key: Some("new-stale".into()),
            expected_project_revision: Some(0),
            destination: "runs".into(),
        },
    })
    .unwrap_err();
    assert_eq!(stale.code, ErrorCode::RevisionConflict);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn parameter_edit_and_restore_create_new_runs_without_changing_prior_evidence() {
    let (root, project) = fixture();
    let original = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let first = verify_run(&project, &original.run_id).unwrap();
    let variant = &first.variants()[0];
    let original_id = variant.id.clone();
    let original_parameters = serde_json::to_value(&variant.parameters).unwrap();
    let mut edited_parameters = original_parameters.clone();
    edited_parameters["width_m"] =
        serde_json::json!(edited_parameters["width_m"].as_f64().unwrap() + 0.25);
    let edit_file = root.join("edit.json");
    fs::write(&edit_file, serde_json::to_vec(&serde_json::json!({"parameters":edited_parameters,"storeys":variant.storeys,"floor_to_floor_m":variant.floor_to_floor_m})).unwrap()).unwrap();
    let store = ProjectStore::open(&project).unwrap();
    let locked = lock_project(&store).unwrap();
    let prior_hashes: Vec<_> = locked
        .read_run(&original.run_id)
        .unwrap()
        .manifest()
        .artifacts
        .iter()
        .map(|a| (a.path.clone(), a.sha256.clone()))
        .collect();
    drop(locked);
    let edited = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Edit {
            from_run: original.run_id.clone(),
            variant: original_id.clone(),
            params: "edit.json".into(),
        },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_ne!(edited.run_id, original.run_id);
    assert_ne!(edited.variant_ids[0], original_id);
    let edited_view = verify_run(&project, &edited.run_id).unwrap();
    assert_eq!(
        edited_view.manifest().parent_run_id.as_deref(),
        Some(original.run_id.as_str())
    );
    assert_eq!(
        edited_view.manifest().parent_variant_id.as_deref(),
        Some(original_id.as_str())
    );
    let restore_file = root.join("restore.json");
    fs::write(&restore_file, serde_json::to_vec(&serde_json::json!({"parameters":original_parameters,"storeys":variant.storeys,"floor_to_floor_m":variant.floor_to_floor_m})).unwrap()).unwrap();
    let restored = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Edit {
            from_run: edited.run_id.clone(),
            variant: edited.variant_ids[0].clone(),
            params: "restore.json".into(),
        },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_ne!(restored.run_id, original.run_id);
    assert_eq!(restored.variant_ids, vec![original_id]);
    let locked = lock_project(&store).unwrap();
    let after_hashes: Vec<_> = locked
        .read_run(&original.run_id)
        .unwrap()
        .manifest()
        .artifacts
        .iter()
        .map(|a| (a.path.clone(), a.sha256.clone()))
        .collect();
    assert_eq!(prior_hashes, after_hashes);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_failures_and_exhaustion_remain_persisted_without_success_status() {
    let (root, project) = fixture();
    let mut source: serde_json::Value =
        serde_json::from_slice(&fs::read(&project).unwrap()).unwrap();
    source["constraints"][0]["value_m"] = serde_json::json!(1.0);
    fs::write(&project, serde_json::to_vec(&source).unwrap()).unwrap();
    let failed = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 3, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_eq!(failed.variant_ids.len(), 3);
    assert!(!failed.all_required_checks_passed);
    assert!(
        verify_run(&project, &failed.run_id)
            .unwrap()
            .evaluations()
            .iter()
            .any(|e| !e.required_checks_passed)
    );
    source["constraints"][1]["value_m"] = serde_json::json!(9.9999999);
    fs::write(&project, serde_json::to_vec(&source).unwrap()).unwrap();
    let exhausted = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 3, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert!(exhausted.variant_ids.is_empty());
    assert!(!exhausted.all_required_checks_passed);
    assert!(!exhausted.exhaustion_explanations.is_empty());
    assert!(
        verify_run(&project, &exhausted.run_id)
            .unwrap()
            .variants()
            .is_empty()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn edit_file_rejects_forged_derived_fields() {
    let (root, project) = fixture();
    let original = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let view = verify_run(&project, &original.run_id).unwrap();
    let variant = &view.variants()[0];
    let bytes = serde_json::to_vec(&serde_json::json!({"parameters":variant.parameters,"storeys":variant.storeys,"floor_to_floor_m":variant.floor_to_floor_m,"id":"forged","required_checks_passed":true})).unwrap();
    fs::write(root.join("bad-edit.json"), bytes).unwrap();
    let error = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Edit {
            from_run: original.run_id.clone(),
            variant: variant.id.clone(),
            params: "bad-edit.json".into(),
        },
        mutation: MutationOptions::default(),
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidSchema);
    let store = ProjectStore::open(&project).unwrap();
    assert_eq!(lock_project(&store).unwrap().revision().unwrap(), 1);
    fs::remove_dir_all(root).unwrap();
}

fn rehash_forgery(
    root: &std::path::Path,
    receipt_path: &str,
    artifact_path: &str,
    edit: impl FnOnce(&mut serde_json::Value),
) {
    let run_dir = root.join(receipt_path);
    let path = run_dir.join(artifact_path);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    edit(&mut value);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    let manifest_path = run_dir.join("manifest.json");
    let mut manifest: RunManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let artifact = manifest
        .artifacts
        .iter_mut()
        .find(|a| a.path == artifact_path)
        .unwrap();
    artifact.sha256 = sha256_hex(&bytes);
    artifact.byte_len = bytes.len() as u64;
    if artifact_path.starts_with("input.") {
        manifest.input_sha256 = artifact.sha256.clone();
    }
    if artifact_path == "normalized.json" {
        manifest.normalized_sha256 = Some(artifact.sha256.clone());
    }
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(&manifest_path, &manifest_bytes).unwrap();
    let revision_path = root.join(".archy/revisions/00000000000000000001.json");
    let mut revision: RevisionRecord =
        serde_json::from_slice(&fs::read(&revision_path).unwrap()).unwrap();
    revision.receipt.manifest_sha256 = sha256_hex(&manifest_bytes);
    fs::write(&revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();
}

#[test]
fn consistently_rehashed_forgery_never_grants_trusted_status() {
    for class in [
        "metric",
        "check",
        "tolerance",
        "parity",
        "summary",
        "variant-id",
        "variant-footprint",
        "project-binding",
        "normalized",
        "report",
    ] {
        let (root, project) = fixture();
        let receipt = generate(GenerateInput {
            project: project.clone(),
            mode: GenerationMode::Sample { count: 1, seed: 42 },
            mutation: MutationOptions::default(),
        })
        .unwrap();
        let variant_id = &receipt.variant_ids[0];
        let artifact = match class {
            "metric" | "check" | "tolerance" | "parity" | "summary" => {
                format!("evaluations/{variant_id}.json")
            }
            "variant-id" | "variant-footprint" | "project-binding" => {
                format!("variants/{variant_id}.json")
            }
            "normalized" => "normalized.json".into(),
            "report" => "report.json".into(),
            _ => unreachable!(),
        };
        rehash_forgery(&root, &receipt.path, &artifact, |value| match class {
            "metric" => value["metrics"][0]["value"] = serde_json::json!(999999.0),
            "check" => value["checks"][0]["measured"] = serde_json::json!(999999.0),
            "tolerance" => value["tolerance"]["length_m"] = serde_json::json!(2.0),
            "parity" => {
                value["geometry_parity_budget"]["footprint_area_m2"] = serde_json::json!(2.0)
            }
            "summary" => {
                value["required_checks_passed"] =
                    serde_json::json!(!value["required_checks_passed"].as_bool().unwrap())
            }
            "variant-id" => value["id"] = serde_json::json!("variant-forged"),
            "variant-footprint" => value["footprint_xy_m"][0][0] = serde_json::json!(500.0),
            "project-binding" => value["project_id"] = serde_json::json!("another-project"),
            "normalized" => value["preferences"] = serde_json::json!(["forged"]),
            "report" => value["uncomputed"] = serde_json::json!(["approved"]),
            _ => unreachable!(),
        });
        let store = ProjectStore::open(&project).unwrap();
        assert!(
            lock_project(&store)
                .unwrap()
                .read_run(&receipt.run_id)
                .is_ok(),
            "core byte reader should accept consistent hashes: {class}"
        );
        let error = verify_run(&project, &receipt.run_id).unwrap_err();
        assert_eq!(error.code, ErrorCode::ArtifactInvalid, "{class}");
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn legacy_preferences_keep_exact_v1_bytes_and_survive_source_edits() {
    let root = std::env::temp_dir().join(format!("archy-legacy-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.json");
    let source =
        include_bytes!("../../../tests/fixtures/briefs/legacy-adversarial-preferences.json");
    fs::write(&project, source).unwrap();
    let spec = parse_project_v1(std::str::from_utf8(source).unwrap(), InputFormat::Json).unwrap();
    let normalized = serde_json::to_vec(&spec).unwrap();
    assert_eq!(spec.preferences.len(), 4);
    assert_eq!(spec.normalization_version, "si-rectangle-v1");
    assert_eq!(
        sha256_hex(source),
        "d3b51c16a980a422b60b972de9e47ca176f9ed2d937706743efac191950fdd75"
    );
    assert_eq!(
        sha256_hex(&normalized),
        "5665a318acf3abe884945905371fab72415405276bcb99783bc78fec2ea10bf3"
    );

    let mutation = MutationOptions {
        idempotency_key: Some("legacy-pref-key".into()),
        expected_project_revision: Some(0),
        ..MutationOptions::default()
    };
    let first = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: mutation.clone(),
    })
    .unwrap();
    let run = root.join(&first.path);
    assert_eq!(fs::read(run.join("input.json")).unwrap(), source);
    assert_eq!(fs::read(run.join("normalized.json")).unwrap(), normalized);
    let report = fs::read(run.join("report.json")).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&report).unwrap()["spec"]["preferences"],
        serde_json::json!(spec.preferences)
    );
    verify_run(&project, &first.run_id).unwrap();

    fs::write(&project, b"{bad json").unwrap();
    verify_run(&project, &first.run_id).unwrap();
    let changed = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: mutation.clone(),
    })
    .unwrap_err();
    assert_eq!(changed.code, ErrorCode::RevisionConflict);
    fs::write(&project, source).unwrap();
    let replay = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation,
    })
    .unwrap();
    assert_eq!(
        (replay.run_id, replay.revision),
        (first.run_id.clone(), first.revision)
    );
    assert_eq!(fs::read(run.join("report.json")).unwrap(), report);

    rehash_forgery(&root, &first.path, "normalized.json", |value| {
        value["normalization_version"] = serde_json::json!("si-rectangle-v2");
    });
    assert_eq!(
        verify_run(&project, &first.run_id).unwrap_err().code,
        ErrorCode::ArtifactInvalid
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unsupported_recorded_algorithm_is_rejected_even_with_rehashed_revision() {
    let (root, project) = fixture();
    let receipt = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let manifest_path = root.join(&receipt.path).join("manifest.json");
    let mut manifest: RunManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest.algorithm_version = "future-unsupported".into();
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(&manifest_path, &manifest_bytes).unwrap();
    let revision_path = root.join(".archy/revisions/00000000000000000001.json");
    let mut revision: RevisionRecord =
        serde_json::from_slice(&fs::read(&revision_path).unwrap()).unwrap();
    revision.receipt.manifest_sha256 = sha256_hex(&manifest_bytes);
    fs::write(&revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();
    assert!(with_locked(&project, |locked| locked
        .read_run(&receipt.run_id)
        .is_ok()));
    assert_eq!(
        verify_run(&project, &receipt.run_id).unwrap_err().code,
        ErrorCode::ArtifactInvalid
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_persists_failed_checks_with_zero_exit_and_false_outcome() {
    let (root, project) = fixture();
    let mut source: serde_json::Value =
        serde_json::from_slice(&fs::read(&project).unwrap()).unwrap();
    source["constraints"][0]["value_m"] = serde_json::json!(1.0);
    fs::write(&project, serde_json::to_vec(&source).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "generate",
            "--project",
            project.to_str().unwrap(),
            "--count",
            "1",
            "--seed",
            "42",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: archy_cli::WorkflowReceipt = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!receipt.all_required_checks_passed);
    assert_eq!(receipt.revision, 1);
    assert!(
        verify_run(&project, &receipt.run_id)
            .unwrap()
            .evaluations()
            .iter()
            .any(|e| !e.required_checks_passed)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_generate_and_evaluate_emit_one_json_receipt_each() {
    let (root, project) = fixture();
    let generated = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "generate",
            "--project",
            project.to_str().unwrap(),
            "--count",
            "2",
            "--seed",
            "42",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    assert!(generated.stderr.is_empty());
    let first: archy_cli::WorkflowReceipt = serde_json::from_slice(&generated.stdout).unwrap();
    assert_eq!(first.variant_ids.len(), 2);
    let repeated = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "generate",
            "--project",
            project.to_str().unwrap(),
            "--count",
            "2",
            "--seed",
            "42",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        repeated.status.success(),
        "{}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    let same_design: archy_cli::WorkflowReceipt = serde_json::from_slice(&repeated.stdout).unwrap();
    assert_ne!(same_design.run_id, first.run_id);
    assert_eq!(same_design.revision, 2);
    assert_eq!(same_design.variant_ids, first.variant_ids);
    let evaluated = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "evaluate",
            "--project",
            project.to_str().unwrap(),
            "--all",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        evaluated.status.success(),
        "{}",
        String::from_utf8_lossy(&evaluated.stderr)
    );
    assert!(evaluated.stderr.is_empty());
    let second: archy_cli::WorkflowReceipt = serde_json::from_slice(&evaluated.stdout).unwrap();
    assert_eq!(second.variant_ids, first.variant_ids);
    assert_eq!(second.revision, 3);
    let one = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "evaluate",
            "--project",
            project.to_str().unwrap(),
            "--run",
            &first.run_id,
            "--variant",
            &first.variant_ids[0],
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        one.status.success(),
        "{}",
        String::from_utf8_lossy(&one.stderr)
    );
    let selected: archy_cli::WorkflowReceipt = serde_json::from_slice(&one.stdout).unwrap();
    assert_eq!(selected.variant_ids, vec![first.variant_ids[0].clone()]);
    assert_eq!(
        verify_run(&project, &selected.run_id)
            .unwrap()
            .manifest()
            .parent_run_id
            .as_deref(),
        Some(first.run_id.as_str())
    );
    let conflict = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "generate",
            "--project",
            project.to_str().unwrap(),
            "--count",
            "2",
            "--seed",
            "42",
            "--from-run",
            &first.run_id,
            "--variant",
            &first.variant_ids[0],
            "--params",
            "edit.json",
        ])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("cannot be used with"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn evaluate_default_source_replays_original_selection_after_newer_generation() {
    let (root, project) = fixture();
    let first = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let key = MutationOptions {
        idempotency_key: Some("evaluate-default".into()),
        expected_project_revision: Some(1),
        destination: "runs".into(),
    };
    let original = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: None,
        selection: VariantSelection::All,
        mutation: key.clone(),
    })
    .unwrap();
    let newer = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 43 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let replay = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: None,
        selection: VariantSelection::All,
        mutation: key,
    })
    .unwrap();
    assert_eq!(replay, original);
    assert_eq!(
        verify_run(&project, &original.run_id)
            .unwrap()
            .manifest()
            .parent_run_id
            .as_deref(),
        Some(first.run_id.as_str())
    );
    assert_ne!(newer.variant_ids, replay.variant_ids);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn changed_absent_evaluation_selector_conflicts_before_validation() {
    let (root, project) = fixture();
    let source = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let key = MutationOptions {
        idempotency_key: Some("evaluation-selector-replay".into()),
        expected_project_revision: Some(1),
        destination: "runs".into(),
    };
    let accepted = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(source.run_id.clone()),
        selection: VariantSelection::One(source.variant_ids[0].clone()),
        mutation: key.clone(),
    })
    .unwrap();
    let _later = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 43 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let changed = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(source.run_id.clone()),
        selection: VariantSelection::One("absent-variant".into()),
        mutation: key.clone(),
    })
    .unwrap_err();
    assert_eq!(changed.code, ErrorCode::RevisionConflict);
    let absent_source = uuid::Uuid::new_v4().to_string();
    let changed_source = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(absent_source.clone()),
        selection: VariantSelection::One(source.variant_ids[0].clone()),
        mutation: key.clone(),
    })
    .unwrap_err();
    assert_eq!(changed_source.code, ErrorCode::RevisionConflict);
    let fresh_missing_source = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(absent_source.clone()),
        selection: VariantSelection::One(source.variant_ids[0].clone()),
        mutation: MutationOptions::default(),
    })
    .unwrap_err();
    assert_eq!(fresh_missing_source.code, ErrorCode::ArtifactInvalid);
    let binary = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "evaluate",
            "--project",
            project.to_str().unwrap(),
            "--run",
            &source.run_id,
            "--variant",
            "absent-variant",
            "--idempotency-key",
            "evaluation-selector-replay",
            "--expected-project-revision",
            "1",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!binary.status.success());
    assert!(binary.stderr.is_empty());
    let envelope: serde_json::Value = serde_json::from_slice(&binary.stdout).unwrap();
    assert_eq!(envelope["code"], "REVISION_CONFLICT");
    assert_eq!(envelope["retriable"], false);
    let source_binary = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "evaluate",
            "--project",
            project.to_str().unwrap(),
            "--run",
            &absent_source,
            "--variant",
            &source.variant_ids[0],
            "--idempotency-key",
            "evaluation-selector-replay",
            "--expected-project-revision",
            "1",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!source_binary.status.success());
    let source_envelope: serde_json::Value = serde_json::from_slice(&source_binary.stdout).unwrap();
    assert_eq!(source_envelope["code"], "REVISION_CONFLICT");
    let replay = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(source.run_id),
        selection: VariantSelection::One(source.variant_ids[0].clone()),
        mutation: key,
    })
    .unwrap();
    assert_eq!(replay, accepted);
    assert_eq!(
        with_locked(&project, |locked| locked.revision().unwrap()),
        3
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn changed_malformed_replay_inputs_conflict_before_parsing() {
    let (root, project) = fixture();
    let sample_key = MutationOptions {
        idempotency_key: Some("raw-sample".into()),
        expected_project_revision: Some(0),
        destination: "runs".into(),
    };
    let first = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: sample_key.clone(),
    })
    .unwrap();
    let variant = verify_run(&project, &first.run_id).unwrap().variants()[0].clone();
    let mut parameters = serde_json::to_value(&variant.parameters).unwrap();
    parameters["width_m"] = serde_json::json!(parameters["width_m"].as_f64().unwrap() + 0.25);
    let params_file = root.join("edit.json");
    fs::write(&params_file, serde_json::to_vec(&serde_json::json!({"parameters":parameters,"storeys":variant.storeys,"floor_to_floor_m":variant.floor_to_floor_m})).unwrap()).unwrap();
    let edit_key = MutationOptions {
        idempotency_key: Some("raw-edit".into()),
        expected_project_revision: Some(1),
        destination: "runs".into(),
    };
    let edit_mode = GenerationMode::Edit {
        from_run: first.run_id.clone(),
        variant: variant.id.clone(),
        params: "edit.json".into(),
    };
    let edited = generate(GenerateInput {
        project: project.clone(),
        mode: edit_mode.clone(),
        mutation: edit_key.clone(),
    })
    .unwrap();
    let _later = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 43 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let absent_parent = uuid::Uuid::new_v4().to_string();
    let changed_parent = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Edit {
            from_run: absent_parent.clone(),
            variant: variant.id.clone(),
            params: "edit.json".into(),
        },
        mutation: edit_key.clone(),
    })
    .unwrap_err();
    assert_eq!(changed_parent.code, ErrorCode::RevisionConflict);
    let fresh_missing_parent = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Edit {
            from_run: absent_parent,
            variant: variant.id.clone(),
            params: "edit.json".into(),
        },
        mutation: MutationOptions::default(),
    })
    .unwrap_err();
    assert_eq!(fresh_missing_parent.code, ErrorCode::ArtifactInvalid);
    assert_eq!(
        generate(GenerateInput {
            project: project.clone(),
            mode: edit_mode.clone(),
            mutation: edit_key.clone()
        })
        .unwrap()
        .run_id,
        edited.run_id
    );
    fs::write(&params_file, b"{bad json").unwrap();
    assert_eq!(
        generate(GenerateInput {
            project: project.clone(),
            mode: edit_mode,
            mutation: edit_key
        })
        .unwrap_err()
        .code,
        ErrorCode::RevisionConflict
    );
    fs::write(&project, b"{bad json").unwrap();
    assert_eq!(
        generate(GenerateInput {
            project: project.clone(),
            mode: GenerationMode::Sample { count: 1, seed: 42 },
            mutation: sample_key
        })
        .unwrap_err()
        .code,
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        with_locked(&project, |locked| locked.revision().unwrap()),
        3
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn edit_history_beyond_sixty_four_runs_stays_readable_and_replayable() {
    let (root, project) = fixture();
    let mut current = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let original = current.clone();
    for index in 0..65 {
        let source = verify_run(&project, &current.run_id).unwrap();
        let variant = &source.variants()[0];
        let mut parameters = serde_json::to_value(&variant.parameters).unwrap();
        parameters["width_m"] = serde_json::json!(parameters["width_m"].as_f64().unwrap() + 0.01);
        fs::write(root.join("deep-edit.json"), serde_json::to_vec(&serde_json::json!({"parameters":parameters,"storeys":variant.storeys,"floor_to_floor_m":variant.floor_to_floor_m})).unwrap()).unwrap();
        let mode = GenerationMode::Edit {
            from_run: current.run_id.clone(),
            variant: variant.id.clone(),
            params: "deep-edit.json".into(),
        };
        let next = generate(GenerateInput {
            project: project.clone(),
            mode: mode.clone(),
            mutation: MutationOptions::default(),
        })
        .unwrap();
        assert_eq!(next.revision, index + 2);
        current = next;
    }
    assert_eq!(
        verify_run(&project, &current.run_id).unwrap().variants()[0].id,
        current.variant_ids[0]
    );
    assert_eq!(
        verify_run(&project, &original.run_id).unwrap().variants()[0].id,
        original.variant_ids[0]
    );
    assert_eq!(
        with_locked(&project, |locked| locked.receipts().unwrap().len()),
        66
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_evaluation_tracks_latest_generation_or_edit_through_mixed_history() {
    let (root, project) = fixture();
    let first = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 2, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let first_eval = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: None,
        selection: VariantSelection::All,
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_eq!(
        verify_run(&project, &first_eval.run_id)
            .unwrap()
            .manifest()
            .parent_run_id
            .as_deref(),
        Some(first.run_id.as_str())
    );
    let second = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 43 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let one = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: None,
        selection: VariantSelection::One(second.variant_ids[0].clone()),
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_eq!(
        verify_run(&project, &one.run_id)
            .unwrap()
            .manifest()
            .parent_run_id
            .as_deref(),
        Some(second.run_id.as_str())
    );
    let parent = verify_run(&project, &second.run_id).unwrap();
    let variant = &parent.variants()[0];
    let mut parameters = serde_json::to_value(&variant.parameters).unwrap();
    parameters["width_m"] = serde_json::json!(parameters["width_m"].as_f64().unwrap() + 0.25);
    fs::write(root.join("mixed-edit.json"), serde_json::to_vec(&serde_json::json!({"parameters":parameters,"storeys":variant.storeys,"floor_to_floor_m":variant.floor_to_floor_m})).unwrap()).unwrap();
    let edit = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Edit {
            from_run: second.run_id.clone(),
            variant: second.variant_ids[0].clone(),
            params: "mixed-edit.json".into(),
        },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let historical = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(first.run_id.clone()),
        selection: VariantSelection::One(first.variant_ids[0].clone()),
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_eq!(
        verify_run(&project, &historical.run_id)
            .unwrap()
            .manifest()
            .parent_run_id
            .as_deref(),
        Some(first.run_id.as_str())
    );
    let latest = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: None,
        selection: VariantSelection::All,
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_eq!(latest.variant_ids, edit.variant_ids);
    assert_eq!(
        verify_run(&project, &latest.run_id)
            .unwrap()
            .manifest()
            .parent_run_id
            .as_deref(),
        Some(edit.run_id.as_str())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_reports_structured_operation_errors_and_distinct_usage_errors() {
    let (root, project) = fixture();
    let operation = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "generate",
            "--project",
            project.to_str().unwrap(),
            "--count",
            "1",
            "--seed",
            "42",
            "--idempotency-key",
            "manual",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!operation.status.success());
    assert!(operation.stderr.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&operation.stdout).unwrap();
    assert_eq!(error["code"], "INVALID_SCHEMA");
    assert_eq!(error["field"], "expected_project_revision");
    let usage = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args([
            "evaluate",
            "--project",
            project.to_str().unwrap(),
            "--all",
            "--variant",
            "invalid",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!usage.status.success());
    assert!(usage.stdout.is_empty());
    assert!(String::from_utf8_lossy(&usage.stderr).contains("cannot be used with"));
    fs::remove_dir_all(root).unwrap();
}
