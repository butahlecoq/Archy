#![cfg(target_os = "linux")]

use archy_cli::{
    EvaluateInput, GenerateInput, GenerationMode, MutationOptions, RenderInput, VariantSelection,
    evaluate, generate, render,
};
use archy_core::{ErrorCode, ProjectStore, sha256_hex};
use std::fs;
use std::process::Command;

#[test]
fn absent_installed_gate_publishes_replayable_failure_without_changing_source() {
    if std::path::Path::new("/usr/lib/archy/archy-service-gate").exists() {
        return;
    }
    let root = std::env::temp_dir().join(format!("archy-render-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.json");
    fs::write(
        &project,
        include_bytes!("../../../tests/fixtures/briefs/courtyard.json"),
    )
    .unwrap();
    let generated = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let evaluated = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(generated.run_id.clone()),
        selection: VariantSelection::All,
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let source_before = fs::read(root.join(&evaluated.path).join("manifest.json")).unwrap();
    let mutation = MutationOptions {
        idempotency_key: Some(uuid::Uuid::new_v4().to_string()),
        expected_project_revision: Some(evaluated.revision),
        destination: "runs".into(),
    };
    let input = RenderInput {
        project: project.clone(),
        source_run: Some(evaluated.run_id.clone()),
        selection: VariantSelection::All,
        mutation: mutation.clone(),
    };
    let first = render(input.clone()).unwrap();
    assert_eq!(first.record.source_run_id, evaluated.run_id);
    assert_eq!(first.record.variants.len(), 1);
    let result = serde_json::to_value(&first.record.variants[0].result).unwrap();
    assert_eq!(result["status"], "failed");
    assert_eq!(result["error"]["code"], "BACKEND_UNAVAILABLE");
    assert_eq!(
        fs::read(root.join(&evaluated.path).join("manifest.json")).unwrap(),
        source_before
    );
    let replayed = render(input).unwrap();
    assert_eq!(replayed, first);
    let conflict = render(RenderInput {
        project: project.clone(),
        source_run: Some(evaluated.run_id),
        selection: VariantSelection::One("variant-other".into()),
        mutation,
    })
    .unwrap_err();
    assert_eq!(conflict.code, ErrorCode::RevisionConflict);
    let store = ProjectStore::open(&project).unwrap();
    assert_eq!(store.lock().unwrap().receipts().unwrap().len(), 3);
    let render_record = root.join(&first.path).join("render.json");
    fs::write(&render_record, b"{}").unwrap();
    assert_eq!(
        archy_cli::verify_run(&project, &first.run_id)
            .unwrap_err()
            .code,
        ErrorCode::ArtifactInvalid
    );
    assert_eq!(
        fs::read(root.join(&evaluated.path).join("manifest.json")).unwrap(),
        source_before
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn render_head_is_readable_and_failed_outputs_are_reported_separately() {
    if std::path::Path::new("/usr/lib/archy/archy-service-gate").exists() {
        return;
    }
    let root = std::env::temp_dir().join(format!("archy-render-status-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.json");
    fs::write(
        &project,
        include_bytes!("../../../tests/fixtures/briefs/courtyard.json"),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_archy");
    let project_arg = project.to_str().unwrap();
    let cli = |args: &[&str]| {
        let output = Command::new(binary).args(args).output().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        (output.status.code().unwrap(), value)
    };
    let (code, generated) = cli(&[
        "generate",
        "--project",
        project_arg,
        "--count",
        "2",
        "--seed",
        "42",
        "--json",
    ]);
    assert_eq!(code, 0);
    let (code, evaluated) = cli(&[
        "evaluate",
        "--project",
        project_arg,
        "--all",
        "--run",
        generated["run_id"].as_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, 0);
    let (code, rendered) = cli(&[
        "render",
        "--project",
        project_arg,
        "--all",
        "--run",
        evaluated["run_id"].as_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, 0);
    assert_eq!(rendered["record"]["variants"].as_array().unwrap().len(), 2);
    let run_id = rendered["run_id"].as_str().unwrap();
    for selected in [None, Some(run_id)] {
        let mut args = vec!["desktop", "status", "--project", project_arg, "--json"];
        if let Some(id) = selected {
            args.extend(["--run", id]);
        }
        let (code, status) = cli(&args);
        assert_eq!(code, 0, "{status}");
        assert_eq!(status["runs"][0]["operation"], "render");
        assert_eq!(status["selected_run"]["summary"]["operation"], "render");
        assert_eq!(
            status["selected_run"]["summary"]["render_outcome"],
            "failed"
        );
        assert_eq!(
            status["selected_run"]["summary"]["all_required_checks_passed"],
            true
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rehashed_all_render_missing_variant_is_rejected_but_one_failure_replays() {
    if std::path::Path::new("/usr/lib/archy/archy-service-gate").exists() {
        return;
    }
    let root = std::env::temp_dir().join(format!("archy-render-complete-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.json");
    fs::write(
        &project,
        include_bytes!("../../../tests/fixtures/briefs/courtyard.json"),
    )
    .unwrap();
    let generated = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 2, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let evaluated = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(generated.run_id),
        selection: VariantSelection::All,
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let mutation = MutationOptions {
        idempotency_key: Some(uuid::Uuid::new_v4().to_string()),
        expected_project_revision: Some(evaluated.revision),
        destination: "runs".into(),
    };
    let input = RenderInput {
        project: project.clone(),
        source_run: Some(evaluated.run_id.clone()),
        selection: VariantSelection::All,
        mutation: mutation.clone(),
    };
    let first = render(input.clone()).unwrap();
    assert_eq!(first.record.variants.len(), 2);
    assert_eq!(render(input.clone()).unwrap(), first);
    let revision = evaluated.revision.to_string();
    let cli_args = [
        "render",
        "--project",
        project.to_str().unwrap(),
        "--all",
        "--run",
        evaluated.run_id.as_str(),
        "--idempotency-key",
        mutation.idempotency_key.as_deref().unwrap(),
        "--expected-project-revision",
        revision.as_str(),
        "--json",
    ];
    let replay = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(cli_args)
        .output()
        .unwrap();
    assert!(replay.status.success());
    let replayed: serde_json::Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(replayed["run_id"], first.run_id);
    assert_eq!(replayed["record"]["variants"].as_array().unwrap().len(), 2);
    let run_dir = root.join(&first.path);
    let record_path = run_dir.join("render.json");
    let original = fs::read(&record_path).unwrap();
    let removed = first.record.variants[1].variant_id.clone();
    let second = format!("{{\"variant_id\":\"{removed}\"");
    let start = original
        .windows(second.len())
        .position(|w| w == second.as_bytes())
        .unwrap();
    assert_eq!(original[start - 1], b',');
    let mut record_bytes = original[..start - 1].to_vec();
    record_bytes.extend_from_slice(b"]}");
    fs::write(&record_path, &record_bytes).unwrap();
    let manifest_path = run_dir.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["outcomes"]
        .as_array_mut()
        .unwrap()
        .retain(|x| x["variant_id"] != removed);
    manifest["artifacts"]
        .as_array_mut()
        .unwrap()
        .retain(|x| !x["path"].as_str().unwrap().contains(&removed));
    for entry in manifest["artifacts"].as_array_mut().unwrap() {
        if entry["path"] == "render.json" {
            entry["sha256"] = sha256_hex(&record_bytes).into();
            entry["byte_len"] = record_bytes.len().into();
        }
    }
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(&manifest_path, &manifest_bytes).unwrap();
    let revision_path = root.join(".archy/revisions/00000000000000000003.json");
    let mut revision: serde_json::Value =
        serde_json::from_slice(&fs::read(&revision_path).unwrap()).unwrap();
    revision["receipt"]["manifest_sha256"] = sha256_hex(&manifest_bytes).into();
    fs::write(&revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();
    let replay_error = render(input).unwrap_err();
    assert_eq!(
        replay_error.code,
        ErrorCode::ArtifactInvalid,
        "{replay_error:?}"
    );
    assert!(
        replay_error.message.contains("requested variants"),
        "{replay_error:?}"
    );
    let verify_error = archy_cli::verify_run(&project, &first.run_id).unwrap_err();
    assert_eq!(
        verify_error.code,
        ErrorCode::ArtifactInvalid,
        "{verify_error:?}"
    );
    let forged_replay = Command::new(env!("CARGO_BIN_EXE_archy"))
        .args(cli_args)
        .output()
        .unwrap();
    assert_eq!(forged_replay.status.code(), Some(1));
    let cli_error: serde_json::Value = serde_json::from_slice(&forged_replay.stdout).unwrap();
    assert_eq!(cli_error["code"], "ARTIFACT_INVALID");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn selected_single_variant_failure_is_complete_and_replayable() {
    if std::path::Path::new("/usr/lib/archy/archy-service-gate").exists() {
        return;
    }
    let root = std::env::temp_dir().join(format!("archy-render-one-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.json");
    fs::write(
        &project,
        include_bytes!("../../../tests/fixtures/briefs/courtyard.json"),
    )
    .unwrap();
    let generated = generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 2, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let evaluated = evaluate(EvaluateInput {
        project: project.clone(),
        source_run: Some(generated.run_id),
        selection: VariantSelection::All,
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let source = archy_cli::verify_run(&project, &evaluated.run_id).unwrap();
    assert_eq!(source.variants().len(), 2);
    let selected = source.variants()[1].id.clone();
    let input = RenderInput {
        project: project.clone(),
        source_run: Some(evaluated.run_id),
        selection: VariantSelection::One(selected.clone()),
        mutation: MutationOptions {
            idempotency_key: Some(uuid::Uuid::new_v4().to_string()),
            expected_project_revision: Some(evaluated.revision),
            destination: "runs".into(),
        },
    };
    let first = render(input.clone()).unwrap();
    assert_eq!(first.record.variants.len(), 1);
    assert_eq!(first.record.variants[0].variant_id, selected);
    assert_eq!(
        serde_json::to_value(&first.record.variants[0].result).unwrap()["status"],
        "failed"
    );
    assert_eq!(render(input).unwrap(), first);
    assert_eq!(
        archy_cli::verify_run(&project, &first.run_id)
            .unwrap()
            .variants()
            .len(),
        1
    );
    fs::remove_dir_all(root).unwrap();
}
