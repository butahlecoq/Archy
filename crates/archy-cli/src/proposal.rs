//! Project-history proposal runs. These are untrusted review suggestions, never decisions.
use crate::{
    BoundProjectRead, MutationOptions, TrustedRun, artifact_invalid, recorded_manifest_for_key,
    render_verified_comparison, request_conflict,
};
use archy_core::{
    AcceptedHistory, ArchyError, ErrorCode, MAX_ARTIFACT_BYTES, RUN_REQUEST_SCHEMA_VERSION,
    RunMetadata, RunReceipt, RunRequest, VariantOutcome, VerifiedRun,
    decision_contract::{ConstraintEvidence, PROPOSAL_SCHEMA_VERSION, Proposal, ReviewEvidence},
    sha256_hex,
};
use archy_report::ComparisonArtifacts;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PROPOSAL_RUN_VERSION: &str = "archy-proposal-run-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalReceipt {
    pub run_id: String,
    pub revision: u64,
    pub path: String,
    pub manifest_sha256: String,
    pub proposal_path: String,
    pub proposal_sha256: String,
    pub source_run_id: String,
    pub variant_id: String,
}

fn digest(run: &TrustedRun, path: &str) -> Result<String, ArchyError> {
    run.manifest()
        .artifacts
        .iter()
        .find(|a| a.path == path)
        .map(|a| a.sha256.clone())
        .ok_or_else(|| artifact_invalid(format!("missing {path} digest")))
}

fn selected(source: &TrustedRun, variant_id: &str) -> Result<(Vec<u8>, Vec<u8>), ArchyError> {
    let index = source
        .variants()
        .iter()
        .position(|v| v.id == variant_id)
        .ok_or_else(|| ArchyError::schema("variant_id", "Variant is absent from source run"))?;
    let variant = serde_json::to_vec(&source.variants()[index])
        .map_err(|e| artifact_invalid(e.to_string()))?;
    let evaluation = serde_json::to_vec(&source.evaluations()[index])
        .map_err(|e| artifact_invalid(e.to_string()))?;
    Ok((variant, evaluation))
}

fn build(
    source: &TrustedRun,
    source_manifest_sha256: &str,
    variant_id: &str,
    reason: String,
    revision: u64,
    head: &str,
) -> Result<(Proposal, ComparisonArtifacts, Vec<u8>, Vec<u8>), ArchyError> {
    if !matches!(source.manifest().operation.as_str(), "generate" | "edit") {
        return Err(ArchyError::schema(
            "source_run",
            "Proposal source must be generation or edit",
        ));
    }
    let (variant, evaluation) = selected(source, variant_id)?;
    let comparison =
        render_verified_comparison(source, source_manifest_sha256, revision, Some(head.into()))?;
    let evidence = ReviewEvidence {
        project_id: source.spec().project_id.clone(),
        source_run_id: source.manifest().run_id.clone(),
        variant_id: variant_id.into(),
        expected_project_revision: revision,
        expected_head_sha256: head.into(),
        input_sha256: digest(
            source,
            if source.manifest().source_format.as_deref() == Some("json") {
                "input.json"
            } else {
                "input.yaml"
            },
        )?,
        normalized_sha256: digest(source, "normalized.json")?,
        variant_sha256: digest(source, &format!("variants/{variant_id}.json"))?,
        evaluation_sha256: digest(source, &format!("evaluations/{variant_id}.json"))?,
        comparison_sha256: sha256_hex(&comparison.json),
        constraints: source
            .spec()
            .constraints
            .iter()
            .map(ConstraintEvidence::from_normalized)
            .collect(),
        checks: source.evaluations()[source
            .variants()
            .iter()
            .position(|v| v.id == variant_id)
            .expect("selected")]
        .checks
        .clone(),
    };
    let proposal = Proposal {
        schema_version: PROPOSAL_SCHEMA_VERSION.into(),
        evidence,
        suggested_reason: reason,
    };
    proposal.validate()?;
    let normalized = match source.canonical() {
        Some(canonical) => serde_json::to_vec(canonical),
        None => serde_json::to_vec(source.spec()),
    }
    .map_err(|e| artifact_invalid(e.to_string()))?;
    proposal
        .evidence
        .claimed_checks_passed_against(&normalized, &variant, &evaluation)?;
    Ok((proposal, comparison, variant, evaluation))
}

pub(crate) fn verify_proposal_run(
    history: &AcceptedHistory<'_>,
    stored: &VerifiedRun,
    source: Option<&TrustedRun>,
    input: &[u8],
    normalized: &[u8],
) -> Result<(), ArchyError> {
    let manifest = stored.manifest();
    let source = source.ok_or_else(|| artifact_invalid("missing proposal source"))?;
    let variant_id = manifest
        .parent_variant_id
        .as_deref()
        .ok_or_else(|| artifact_invalid("missing proposal variant"))?;
    let head = history
        .previous_head_for_run(&manifest.run_id)
        .ok_or_else(|| artifact_invalid("missing pre-proposal head"))?;
    if manifest.algorithm_version != PROPOSAL_RUN_VERSION
        || manifest.expected_project_revision == 0
        || manifest.seed.is_some()
        || manifest.count.is_some()
        || manifest.source_format != source.manifest().source_format
        || manifest.input_sha256 != source.manifest().input_sha256
        || input != source.input()
        || manifest.normalized_sha256.as_deref() != Some(sha256_hex(normalized).as_str())
        || !matches!(source.manifest().operation.as_str(), "generate" | "edit")
        || manifest.parent_run_id.as_deref() != Some(source.manifest().run_id.as_str())
        || manifest.outcomes.len() != 1
    {
        return Err(artifact_invalid("invalid proposal source or metadata"));
    }
    let source_receipt = history
        .receipts()
        .find(|r| r.run_id == source.manifest().run_id)
        .ok_or_else(|| artifact_invalid("proposal source not accepted"))?;
    if history.revision_for_run(&manifest.run_id)
        != manifest.expected_project_revision.checked_add(1)
        || source_receipt.revision > manifest.expected_project_revision
    {
        return Err(artifact_invalid(
            "proposal revision or source chronology differs",
        ));
    }
    let previous = history
        .receipts()
        .find(|r| r.revision == manifest.expected_project_revision)
        .ok_or_else(|| artifact_invalid("pre-proposal revision is absent"))?;
    crate::run_integrity::verify_history(history, &previous.run_id)?;
    let raw = stored
        .artifact("proposal.json")
        .ok_or_else(|| artifact_invalid("missing proposal artifact"))?;
    let proposal: Proposal = serde_json::from_slice(raw)
        .map_err(|e| artifact_invalid(format!("invalid proposal: {e}")))?;
    let (expected, comparison, variant, evaluation) = build(
        source,
        &source_receipt.manifest_sha256,
        variant_id,
        proposal.suggested_reason.clone(),
        manifest.expected_project_revision,
        head,
    )?;
    let variant_path = format!("variants/{variant_id}.json");
    let evaluation_path = format!("evaluations/{variant_id}.json");
    let outcome = &manifest.outcomes[0];
    let mut paths = BTreeSet::from([
        "proposal.json".to_owned(),
        "comparison.json".into(),
        "normalized.json".into(),
        variant_path.clone(),
        evaluation_path.clone(),
        if manifest.source_format.as_deref() == Some("json") {
            "input.json".into()
        } else {
            "input.yaml".into()
        },
    ]);
    if source.canonical().is_some() {
        paths.insert("profile.json".into());
    }
    if proposal != expected
        || serde_json::to_vec(&expected).map_err(|e| artifact_invalid(e.to_string()))? != raw
        || stored.artifact("comparison.json") != Some(comparison.json.as_slice())
        || stored.artifact(&variant_path) != Some(variant.as_slice())
        || stored.artifact(&evaluation_path) != Some(evaluation.as_slice())
        || outcome.variant_id != variant_id
        || outcome.variant_artifact.as_deref() != Some(&variant_path)
        || outcome.evaluation_artifact.as_deref() != Some(&evaluation_path)
        || outcome.error_artifact.is_some()
        || stored.artifacts().keys().cloned().collect::<BTreeSet<_>>() != paths
    {
        return Err(artifact_invalid(
            "proposal evidence or inventory differs from accepted source",
        ));
    }
    Ok(())
}

fn receipt(
    history: &AcceptedHistory<'_>,
    accepted: RunReceipt,
) -> Result<ProposalReceipt, ArchyError> {
    let stored = history.read_run(&accepted.run_id)?;
    let manifest = stored.manifest();
    let artifact = manifest
        .artifacts
        .iter()
        .find(|a| a.path == "proposal.json")
        .ok_or_else(|| artifact_invalid("proposal artifact missing"))?;
    Ok(ProposalReceipt {
        run_id: accepted.run_id,
        revision: accepted.revision,
        path: accepted.path.clone(),
        manifest_sha256: accepted.manifest_sha256,
        proposal_path: format!("{}/proposal.json", accepted.path),
        proposal_sha256: artifact.sha256.clone(),
        source_run_id: manifest
            .parent_run_id
            .clone()
            .ok_or_else(|| artifact_invalid("proposal source missing"))?,
        variant_id: manifest
            .parent_variant_id
            .clone()
            .ok_or_else(|| artifact_invalid("proposal variant missing"))?,
    })
}

fn verify_requested(
    history: &AcceptedHistory<'_>,
    id: &str,
    source_run: &str,
    variant_id: &str,
    reason: &str,
) -> Result<(), ArchyError> {
    crate::run_integrity::verify_history(history, id)?;
    let stored = history.read_run(id)?;
    let manifest = stored.manifest();
    let proposal: Proposal = serde_json::from_slice(
        stored
            .artifact("proposal.json")
            .ok_or_else(|| artifact_invalid("missing proposal artifact"))?,
    )
    .map_err(|e| artifact_invalid(format!("invalid proposal: {e}")))?;
    if manifest.operation != "propose"
        || manifest.parent_run_id.as_deref() != Some(source_run)
        || manifest.parent_variant_id.as_deref() != Some(variant_id)
        || proposal.suggested_reason != reason
    {
        return Err(request_conflict());
    }
    Ok(())
}

pub(crate) fn propose(
    reader: &BoundProjectRead,
    source_run: String,
    variant_id: String,
    reason: String,
    mutation: MutationOptions,
) -> Result<ProposalReceipt, ArchyError> {
    reader.check_binding()?;
    let locked = reader.lock()?;
    let context = mutation.resolve(&locked)?;
    let recorded = recorded_manifest_for_key(&locked, mutation.idempotency_key.as_deref())?;
    if recorded.as_ref().is_some_and(|m| {
        m.operation != "propose"
            || m.parent_run_id.as_deref() != Some(&source_run)
            || m.parent_variant_id.as_deref() != Some(&variant_id)
    }) {
        return Err(request_conflict());
    }
    let history = locked.accepted_history()?;
    if let Some(head) = history.receipts().last() {
        crate::run_integrity::verify_history(&history, &head.run_id)?;
    }
    let source = crate::run_integrity::verify_history(&history, &source_run)?;
    if source.spec().project_id != reader.project_id() {
        return Err(artifact_invalid(
            "proposal source belongs to another project",
        ));
    }
    let source_receipt = history
        .receipts()
        .find(|r| r.run_id == source_run)
        .ok_or_else(|| artifact_invalid("source run is not accepted"))?;
    let request_bytes = serde_json::to_vec(
        &serde_json::json!({"source_run":source_run,"variant_id":variant_id,"reason":reason}),
    )
    .map_err(|e| artifact_invalid(e.to_string()))?;
    let request = RunRequest {
        schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
        context,
        operation: "propose".into(),
        request_bytes,
        input_sha256: source.manifest().input_sha256.clone(),
        source_run: Some(source_run.clone()),
    };
    let validate_orphan = |candidate: &AcceptedHistory<'_>, id: &str| {
        verify_requested(candidate, id, &source_run, &variant_id, &reason)
    };
    if recorded.is_some() {
        return match locked.begin_checked_recovery(
            request,
            &|| reader.check_binding(),
            &validate_orphan,
        )? {
            archy_core::BeginResult::Replay(accepted) => {
                let history = locked.accepted_history()?;
                verify_requested(
                    &history,
                    &accepted.run_id,
                    &source_run,
                    &variant_id,
                    &reason,
                )?;
                reader.check_binding()?;
                receipt(&history, accepted)
            }
            archy_core::BeginResult::Pending(_) => {
                Err(artifact_invalid("recorded proposal unexpectedly pending"))
            }
        };
    }
    let current_revision = history.revision();
    let head = history.head_sha256().ok_or_else(|| {
        ArchyError::new(
            ErrorCode::RevisionConflict,
            "revision",
            "Proposal requires an accepted head",
        )
    })?;
    if mutation.expected_project_revision != Some(current_revision) {
        return Err(ArchyError::new(
            ErrorCode::RevisionConflict,
            "revision",
            "stale project revision",
        ));
    }
    let (proposal, comparison, variant, evaluation) = build(
        &source,
        &source_receipt.manifest_sha256,
        &variant_id,
        reason.clone(),
        current_revision,
        head,
    )?;
    let proposal_bytes =
        serde_json::to_vec(&proposal).map_err(|e| artifact_invalid(e.to_string()))?;
    if proposal_bytes.len() > MAX_ARTIFACT_BYTES || comparison.json.len() > MAX_ARTIFACT_BYTES {
        return Err(ArchyError::schema(
            "proposal",
            "Proposal evidence exceeds artifact bound",
        ));
    }
    let mut pending = match locked.begin_checked_recovery(
        request,
        &|| reader.check_binding(),
        &validate_orphan,
    )? {
        archy_core::BeginResult::Pending(pending) => pending,
        archy_core::BeginResult::Replay(accepted) => {
            let history = locked.accepted_history()?;
            verify_requested(
                &history,
                &accepted.run_id,
                &source_run,
                &variant_id,
                &reason,
            )?;
            return receipt(&history, accepted);
        }
    };
    let format = source
        .manifest()
        .source_format
        .as_deref()
        .ok_or_else(|| artifact_invalid("missing source format"))?;
    pending.write(
        if format == "json" {
            "input.json"
        } else {
            "input.yaml"
        },
        source.input(),
        "application/octet-stream",
        "archy-cli",
    )?;
    let normalized = match source.canonical() {
        Some(c) => serde_json::to_vec(c),
        None => serde_json::to_vec(source.spec()),
    }
    .map_err(|e| artifact_invalid(e.to_string()))?;
    pending.write(
        "normalized.json",
        &normalized,
        "application/json",
        "archy-core",
    )?;
    if let Some(version) = source.source_schema_version() {
        pending.write(
            "profile.json",
            &crate::run_integrity::profile_bytes("propose", PROPOSAL_RUN_VERSION, version)?,
            "application/json",
            "archy-cli",
        )?;
    }
    let variant_path = format!("variants/{variant_id}.json");
    let evaluation_path = format!("evaluations/{variant_id}.json");
    pending.write(&variant_path, &variant, "application/json", "archy-core")?;
    pending.write(
        &evaluation_path,
        &evaluation,
        "application/json",
        "archy-eval",
    )?;
    pending.write(
        "comparison.json",
        &comparison.json,
        "application/json",
        "archy-report",
    )?;
    pending.write(
        "proposal.json",
        &proposal_bytes,
        "application/json",
        "archy-cli",
    )?;
    pending.record_outcome(VariantOutcome {
        variant_id: variant_id.clone(),
        variant_artifact: Some(variant_path),
        evaluation_artifact: Some(evaluation_path),
        error_artifact: None,
    })?;
    let metadata = RunMetadata {
        project_id: source.spec().project_id.clone(),
        normalized_sha256: Some(sha256_hex(&normalized)),
        source_format: Some(format.into()),
        seed: None,
        count: None,
        parent_variant_id: Some(variant_id.clone()),
        algorithm_version: PROPOSAL_RUN_VERSION.into(),
    };
    let accepted = pending.commit_checked(metadata, &|| reader.check_binding())?;
    let history = locked.accepted_history()?;
    verify_requested(
        &history,
        &accepted.run_id,
        &source_run,
        &variant_id,
        &reason,
    )?;
    reader.check_binding()?;
    receipt(&history, accepted)
}
