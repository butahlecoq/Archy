use archy_cli::{GenerateInput, GenerationMode, MutationOptions};
use archy_core::{ProjectStore, RevisionRecord, RunManifest, sha256_hex};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("archy-compare-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.json");
    fs::write(
        &project,
        include_bytes!("../../../tests/fixtures/briefs/courtyard.json"),
    )
    .unwrap();
    (root, project)
}
fn inventory(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(&path).unwrap(),
                ));
            }
        }
    }
    let mut out = Vec::new();
    visit(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
fn cli(project: &Path, run: Option<&str>, json: bool) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_archy"));
    cmd.arg("compare").arg("--project").arg(project);
    if let Some(run) = run {
        cmd.arg("--run").arg(run);
    }
    if json {
        cmd.arg("--json");
    }
    cmd.output().unwrap()
}
#[test]
fn historical_latest_and_edited_current_input_are_passive() {
    let (root, project) = fixture();
    let first = archy_cli::generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 3, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let second = archy_cli::generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 3, seed: 43 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let evaluated = archy_cli::evaluate(archy_cli::EvaluateInput {
        project: project.clone(),
        source_run: Some(second.run_id.clone()),
        selection: archy_cli::VariantSelection::All,
        mutation: MutationOptions::default(),
    })
    .unwrap();
    assert_eq!(evaluated.revision, 3);
    fs::write(&project, b"<script>changed current input").unwrap();
    let before = inventory(&root);
    let latest = cli(&project, None, true);
    assert!(
        latest.status.success(),
        "{}",
        String::from_utf8_lossy(&latest.stdout)
    );
    let historical = cli(&project, Some(&first.run_id), true);
    assert!(historical.status.success());
    let markdown = cli(&project, Some(&first.run_id), false);
    assert!(markdown.status.success());
    let latest_doc: serde_json::Value = serde_json::from_slice(&latest.stdout).unwrap();
    let prior_doc: serde_json::Value = serde_json::from_slice(&historical.stdout).unwrap();
    assert_eq!(latest_doc["source_run_id"], second.run_id);
    assert_eq!(prior_doc["source_run_id"], first.run_id);
    assert_eq!(
        prior_doc["candidates"].as_array().unwrap().len(),
        first.variant_ids.len()
    );
    assert!(!String::from_utf8_lossy(&markdown.stdout).contains("<script>"));
    assert_eq!(before, inventory(&root));
    assert_eq!(
        ProjectStore::open(&project)
            .unwrap()
            .read_history()
            .unwrap()
            .revision(),
        3
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn no_history_and_wrong_operation_do_not_create_state() {
    let (root, project) = fixture();
    let before = inventory(&root);
    let output = cli(&project, None, true);
    assert!(!output.status.success());
    assert_eq!(before, inventory(&root));
    let source = archy_cli::generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 3, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let evaluated = archy_cli::evaluate(archy_cli::EvaluateInput {
        project: project.clone(),
        source_run: Some(source.run_id),
        selection: archy_cli::VariantSelection::All,
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let before = inventory(&root);
    let output = cli(&project, Some(&evaluated.run_id), true);
    assert!(!output.status.success());
    assert_eq!(before, inventory(&root));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rehashed_metric_forgery_is_rejected() {
    let (root, project) = fixture();
    let receipt = archy_cli::generate(GenerateInput {
        project: project.clone(),
        mode: GenerationMode::Sample { count: 1, seed: 42 },
        mutation: MutationOptions::default(),
    })
    .unwrap();
    let path = format!("evaluations/{}.json", receipt.variant_ids[0]);
    let artifact_path = root.join(&receipt.path).join(&path);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
    value["metrics"][0]["value"] = serde_json::json!(999999.0);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&artifact_path, &bytes).unwrap();
    let manifest_path = root.join(&receipt.path).join("manifest.json");
    let mut manifest: RunManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let artifact = manifest
        .artifacts
        .iter_mut()
        .find(|a| a.path == path)
        .unwrap();
    artifact.sha256 = sha256_hex(&bytes);
    artifact.byte_len = bytes.len() as u64;
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(&manifest_path, &manifest_bytes).unwrap();
    let revision_path = root.join(".archy/revisions/00000000000000000001.json");
    let mut revision: RevisionRecord =
        serde_json::from_slice(&fs::read(&revision_path).unwrap()).unwrap();
    revision.receipt.manifest_sha256 = sha256_hex(&manifest_bytes);
    fs::write(&revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();
    let before = inventory(&root);
    let output = cli(&project, None, true);
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["code"], "ARTIFACT_INVALID");
    assert_eq!(before, inventory(&root));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_marks_directional_controls_in_markdown_and_preserves_json_and_history() {
    let (root, project) = fixture();
    let mut brief: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard-v2.json"
    ))
    .unwrap();
    let authority = "user\u{202e}:devorppa";
    let source = "source\u{2066}isolate\u{2069} [link](url)|<script>";
    let preference = "café العربية 👩\u{200d}💻 \u{202d}override\u{202c}\u{061c}\u{001b}";
    brief["constraints"][0]["authority"] = authority.into();
    brief["constraints"][0]["source"] = source.into();
    brief["notes"] = serde_json::json!([preference]);
    fs::write(&project, serde_json::to_vec(&brief).unwrap()).unwrap();
    let generated = Command::new(env!("CARGO_BIN_EXE_archy"))
        .arg("generate")
        .arg("--project")
        .arg(&project)
        .args(["--count", "1", "--seed", "42", "--json"])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stdout)
    );
    let before = inventory(&root);
    let output = cli(&project, None, false);
    assert!(output.status.success());
    let markdown = String::from_utf8(output.stdout).unwrap();
    for code in [0x202e, 0x2066, 0x2069] {
        assert!(
            !markdown.contains(char::from_u32(code).unwrap()),
            "raw U+{code:04X}"
        );
        assert!(
            markdown.contains(&format!("\\[U+{code:04X}\\]")),
            "missing visible U+{code:04X}"
        );
    }
    assert!(markdown.contains("user\\[U+202E\\]:devorppa"));
    assert!(markdown.contains("\\[link\\]\\(url\\)\\|&lt;script&gt;"));
    assert!(markdown.contains("courtyard"));
    let output = cli(&project, None, true);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["constraints"][0]["authority"], authority);
    assert_eq!(json["constraints"][0]["source"], source);
    assert_eq!(json["subjective_preferences"][0], "courtyard");
    let generated: serde_json::Value = serde_json::from_slice(&generated.stdout).unwrap();
    let run = root.join(generated["path"].as_str().unwrap());
    let normalized: serde_json::Value =
        serde_json::from_slice(&fs::read(run.join("normalized.json")).unwrap()).unwrap();
    assert_eq!(normalized["notes"][0], preference);
    let report_markdown = fs::read_to_string(run.join("report.md")).unwrap();
    assert!(report_markdown.contains("café العربية 👩\u{200d}💻"));
    assert!(!report_markdown.contains('\u{202d}'));
    for code in [0x202d, 0x202c, 0x061c, 0x001b] {
        assert!(report_markdown.contains(&format!("\\[U+{code:04X}\\]")));
    }
    assert_eq!(before, inventory(&root));
    fs::remove_dir_all(root).unwrap();
}
