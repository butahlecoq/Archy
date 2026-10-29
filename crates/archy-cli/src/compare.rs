use crate::{TrustedRun, artifact_invalid, run_integrity::verify_history};
use archy_core::{ArchyError, ProjectStore, RunManifest};
use archy_report::{ComparisonArtifacts, ComparisonSource, render_comparison};
use std::path::Path;

fn digest(manifest: &RunManifest, path: &str) -> Result<String, ArchyError> {
    manifest
        .artifacts
        .iter()
        .find(|a| a.path == path)
        .map(|a| a.sha256.clone())
        .ok_or_else(|| artifact_invalid(format!("missing {path} digest")))
}

pub fn compare(project: &Path, run: Option<&str>) -> Result<ComparisonArtifacts, ArchyError> {
    let store = ProjectStore::open(project)?;
    let history = store.read_history()?;
    let selected = match run {
        Some(id) => {
            if id.is_empty() || id.len() > 128 {
                return Err(ArchyError::schema("run", "Invalid run selector"));
            }
            let stored = history.read_run(id)?;
            if !matches!(stored.manifest().operation.as_str(), "generate" | "edit") {
                return Err(ArchyError::schema(
                    "run",
                    "Comparison source must be a generation or edit run",
                ));
            }
            id.to_owned()
        }
        None => {
            let mut found = None;
            for receipt in history.receipts().rev() {
                let stored = history.read_run(&receipt.run_id)?;
                if matches!(stored.manifest().operation.as_str(), "generate" | "edit") {
                    found = Some(receipt.run_id.clone());
                    break;
                }
            }
            found.ok_or_else(|| ArchyError::schema("run", "No accepted generation or edit run"))?
        }
    };
    let trusted = verify_history(&history, &selected)?;
    let receipt = history
        .receipts()
        .find(|r| r.run_id == selected)
        .ok_or_else(|| artifact_invalid("missing source receipt"))?;
    render_verified_comparison(
        &trusted,
        &receipt.manifest_sha256,
        history.revision(),
        history.head_sha256().map(str::to_owned),
    )
}

pub fn render_verified_comparison(
    trusted: &TrustedRun,
    manifest_sha256: &str,
    revision: u64,
    head_sha256: Option<String>,
) -> Result<ComparisonArtifacts, ArchyError> {
    let manifest = trusted.manifest();
    if !matches!(manifest.operation.as_str(), "generate" | "edit") {
        return Err(ArchyError::schema(
            "run_ids",
            "Comparison source must be a generation or edit run",
        ));
    }
    let source = ComparisonSource {
        run_id: manifest.run_id.clone(),
        manifest_sha256: manifest_sha256.to_owned(),
        input_sha256: manifest.input_sha256.clone(),
        normalized_sha256: manifest
            .normalized_sha256
            .clone()
            .ok_or_else(|| artifact_invalid("missing normalized digest"))?,
        variant_sha256: trusted
            .variants()
            .iter()
            .map(|v| digest(manifest, &format!("variants/{}.json", v.id)))
            .collect::<Result<Vec<_>, _>>()?,
        evaluation_sha256: trusted
            .variants()
            .iter()
            .map(|v| digest(manifest, &format!("evaluations/{}.json", v.id)))
            .collect::<Result<Vec<_>, _>>()?,
    };
    render_comparison(
        trusted.spec(),
        trusted.variants(),
        trusted.evaluations(),
        &source,
        revision,
        head_sha256,
    )
}
