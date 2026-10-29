use crate::{artifact_invalid, generate::EditParameters};
use archy_core::{
    AcceptedHistory, ArchyError, EvaluationReport, InputFormat, LockedProject, NormalizedSpec,
    ProjectStore, RunManifest, Variant, parse_project, sha256_hex,
};
use archy_eval::{EVALUATION_VERSION, evaluate_variant};
use archy_generate::{ALGORITHM_VERSION, GenerationResult, generate_variants};
use archy_report::{REPORT_CONTEXT_VERSION, ReportContext, render_report};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug)]
pub struct TrustedRun {
    manifest: RunManifest,
    spec: NormalizedSpec,
    input: Vec<u8>,
    variants: Vec<Variant>,
    evaluations: Vec<EvaluationReport>,
    exhaustion: Vec<String>,
}
impl TrustedRun {
    pub fn manifest(&self) -> &RunManifest {
        &self.manifest
    }
    pub fn spec(&self) -> &NormalizedSpec {
        &self.spec
    }
    pub fn input(&self) -> &[u8] {
        &self.input
    }
    pub fn variants(&self) -> &[Variant] {
        &self.variants
    }
    pub fn evaluations(&self) -> &[EvaluationReport] {
        &self.evaluations
    }
    pub fn exhaustion(&self) -> &[String] {
        &self.exhaustion
    }
}

pub fn verify_run(project: &Path, run_id: &str) -> Result<TrustedRun, ArchyError> {
    let store = ProjectStore::open(project)?;
    let locked = store.lock()?;
    verify_locked(&locked, run_id)
}

pub(crate) fn report_context(
    manifest: &RunManifest,
    generation: Option<&GenerationResult>,
    exhaustion: &[String],
) -> Result<ReportContext, ArchyError> {
    Ok(ReportContext {
        schema_version: REPORT_CONTEXT_VERSION.into(),
        run_id: manifest.run_id.clone(),
        parent_run_id: manifest.parent_run_id.clone(),
        parent_variant_id: manifest.parent_variant_id.clone(),
        operation: manifest.operation.clone(),
        input_sha256: manifest.input_sha256.clone(),
        normalized_sha256: manifest
            .normalized_sha256
            .clone()
            .ok_or_else(|| artifact_invalid("missing normalized digest"))?,
        algorithm_version: manifest.algorithm_version.clone(),
        build_version: manifest.build_version.clone(),
        requested_count: manifest.count,
        attempts: generation.map(|g| g.attempts),
        exhaustion_explanations: exhaustion.to_vec(),
    })
}

pub(crate) fn event_bytes(
    operation: &str,
    variants: &[Variant],
    evaluations: &[EvaluationReport],
) -> Result<Vec<u8>, ArchyError> {
    let mut bytes = serde_json::to_vec(&serde_json::json!({
        "event": "run_completed",
        "operation": operation,
        "variant_ids": variants.iter().map(|v| &v.id).collect::<Vec<_>>(),
        "required_checks_passed": evaluations.iter().map(|e| e.required_checks_passed).collect::<Vec<_>>()
    })).map_err(|e| artifact_invalid(format!("serialize event: {e}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(crate) fn verify_locked(
    locked: &LockedProject<'_>,
    run_id: &str,
) -> Result<TrustedRun, ArchyError> {
    let history = locked.accepted_history()?;
    verify_history(&history, run_id)
}

pub(crate) fn verify_history(
    history: &AcceptedHistory<'_>,
    run_id: &str,
) -> Result<TrustedRun, ArchyError> {
    let revision_count = history.len();
    let mut seen = BTreeSet::new();
    let mut ancestry = Vec::new();
    let mut next = Some(run_id.to_owned());
    while let Some(id) = next {
        if ancestry.len() >= revision_count || !seen.insert(id.clone()) {
            return Err(artifact_invalid("run parent cycle or unaccepted ancestor"));
        }
        let stored = history.read_run(&id)?;
        next = stored.manifest().parent_run_id.clone();
        ancestry.push(id);
    }
    let mut verified = None;
    for id in ancestry.into_iter().rev() {
        verified = Some(verify_one(history, &id, verified.as_ref())?);
    }
    verified.ok_or_else(|| artifact_invalid("missing run"))
}

fn verify_one(
    history: &AcceptedHistory<'_>,
    run_id: &str,
    parent: Option<&TrustedRun>,
) -> Result<TrustedRun, ArchyError> {
    let stored = history.read_run(run_id)?;
    let manifest = stored.manifest().clone();
    let (format, input_path) = match manifest.source_format.as_deref() {
        Some("json") => (InputFormat::Json, "input.json"),
        Some("yaml") => (InputFormat::Yaml, "input.yaml"),
        _ => return Err(artifact_invalid("unsupported source format")),
    };
    let input = stored
        .artifact(input_path)
        .ok_or_else(|| artifact_invalid("missing input snapshot"))?
        .to_vec();
    if sha256_hex(&input) != manifest.input_sha256 {
        return Err(artifact_invalid("input digest mismatch"));
    }
    let source = std::str::from_utf8(&input).map_err(|_| artifact_invalid("input is not UTF-8"))?;
    let spec = parse_project(source, format)
        .map_err(|e| artifact_invalid(format!("stored source invalid: {e}")))?;
    if spec.project_id != manifest.project_id {
        return Err(artifact_invalid("project ID mismatch"));
    }
    let normalized = serde_json::to_vec(&spec)
        .map_err(|e| artifact_invalid(format!("serialize normalized: {e}")))?;
    if stored.artifact("normalized.json") != Some(normalized.as_slice())
        || manifest.normalized_sha256.as_deref() != Some(sha256_hex(&normalized).as_str())
    {
        return Err(artifact_invalid("source and normalized snapshot differ"));
    }
    let expected_algorithm = match manifest.operation.as_str() {
        "generate" | "edit" => ALGORITHM_VERSION,
        "evaluate" => EVALUATION_VERSION,
        _ => return Err(artifact_invalid("unsupported run operation")),
    };
    if manifest.algorithm_version != expected_algorithm {
        return Err(artifact_invalid("unsupported recorded algorithm version"));
    }
    let parent = match manifest.operation.as_str() {
        "generate" if manifest.parent_run_id.is_none() && manifest.parent_variant_id.is_none() => {
            if parent.is_some() {
                return Err(artifact_invalid("unexpected generation ancestor"));
            }
            None
        }
        "edit" | "evaluate" => {
            let id = manifest
                .parent_run_id
                .as_deref()
                .ok_or_else(|| artifact_invalid("missing source run"))?;
            let prior = parent.ok_or_else(|| artifact_invalid("missing verified source run"))?;
            if prior.manifest.run_id != id {
                return Err(artifact_invalid("wrong verified source run"));
            }
            if !matches!(prior.manifest.operation.as_str(), "generate" | "edit")
                || prior.spec != spec
                || prior.input != input
            {
                return Err(artifact_invalid("source run and input snapshot differ"));
            }
            Some(prior)
        }
        _ => return Err(artifact_invalid("invalid run parent references")),
    };
    let generation = if manifest.operation == "generate" {
        let count = manifest
            .count
            .ok_or_else(|| artifact_invalid("missing requested count"))?;
        let seed = manifest
            .seed
            .ok_or_else(|| artifact_invalid("missing seed"))?;
        let expected = generate_variants(&spec, count, seed)
            .map_err(|e| artifact_invalid(format!("generation recomputation failed: {e}")))?;
        let bytes = stored
            .artifact("generation.json")
            .ok_or_else(|| artifact_invalid("missing generation result"))?;
        let observed: GenerationResult = serde_json::from_slice(bytes)
            .map_err(|e| artifact_invalid(format!("invalid generation result: {e}")))?;
        if observed != expected
            || serde_json::to_vec(&expected).map_err(|e| artifact_invalid(e.to_string()))? != bytes
        {
            return Err(artifact_invalid(
                "generation result differs from supported algorithm",
            ));
        }
        Some(expected)
    } else {
        None
    };
    let mut variants = Vec::new();
    let mut evaluations = Vec::new();
    let mut expected_paths = BTreeSet::from([
        input_path.to_owned(),
        "normalized.json".into(),
        "report.json".into(),
        "report.md".into(),
        "events.jsonl".into(),
    ]);
    if generation.is_some() {
        expected_paths.insert("generation.json".into());
    }
    if manifest.operation == "edit" {
        expected_paths.insert("params.json".into());
    }
    if manifest.outcomes.len() > 1000 {
        return Err(artifact_invalid("too many outcomes"));
    }
    for outcome in &manifest.outcomes {
        let variant_path = format!("variants/{}.json", outcome.variant_id);
        let eval_path = format!("evaluations/{}.json", outcome.variant_id);
        if outcome.variant_artifact.as_deref() != Some(&variant_path)
            || outcome.evaluation_artifact.as_deref() != Some(&eval_path)
            || outcome.error_artifact.is_some()
        {
            return Err(artifact_invalid("unexpected outcome references"));
        }
        expected_paths.insert(variant_path.clone());
        expected_paths.insert(eval_path.clone());
        let variant_bytes = stored
            .artifact(&variant_path)
            .ok_or_else(|| artifact_invalid("missing variant artifact"))?;
        let variant: Variant = serde_json::from_slice(variant_bytes)
            .map_err(|e| artifact_invalid(format!("invalid stored variant: {e}")))?;
        if variant.id != outcome.variant_id
            || variant.project_id != spec.project_id
            || variant.generation_algorithm != ALGORITHM_VERSION
            || serde_json::to_vec(&variant).map_err(|e| artifact_invalid(e.to_string()))?
                != variant_bytes
        {
            return Err(artifact_invalid(
                "variant identity or canonical bytes differ",
            ));
        }
        let evaluation_bytes = stored
            .artifact(&eval_path)
            .ok_or_else(|| artifact_invalid("missing evaluation artifact"))?;
        let evaluation: EvaluationReport = serde_json::from_slice(evaluation_bytes)
            .map_err(|e| artifact_invalid(format!("invalid stored evaluation: {e}")))?;
        let recomputed = evaluate_variant(&spec, &variant)
            .map_err(|e| artifact_invalid(format!("evaluation recomputation failed: {e}")))?;
        if evaluation != recomputed
            || serde_json::to_vec(&recomputed).map_err(|e| artifact_invalid(e.to_string()))?
                != evaluation_bytes
        {
            return Err(artifact_invalid(
                "stored evaluation differs from full recomputation",
            ));
        }
        variants.push(variant);
        evaluations.push(recomputed);
    }
    let exhaustion = match &generation {
        Some(g) => g.exhaustion_explanations.clone(),
        None => Vec::new(),
    };
    match manifest.operation.as_str() {
        "generate" => {
            let result = generation.as_ref().expect("checked operation");
            if result.variants != variants {
                return Err(artifact_invalid(
                    "generation variants differ from artifacts",
                ));
            }
        }
        "edit" => {
            let prior = parent.as_ref().expect("checked parent");
            let parent_id = manifest
                .parent_variant_id
                .as_deref()
                .ok_or_else(|| artifact_invalid("missing parent variant"))?;
            let parent_variant = prior
                .variants
                .iter()
                .find(|v| v.id == parent_id)
                .ok_or_else(|| artifact_invalid("parent variant absent"))?;
            let bytes = stored
                .artifact("params.json")
                .ok_or_else(|| artifact_invalid("missing edit parameters"))?;
            if bytes.len() > archy_core::validation::MAX_INPUT_BYTES {
                return Err(artifact_invalid("edit parameters exceed limit"));
            }
            let params: EditParameters = serde_json::from_slice(bytes)
                .map_err(|e| artifact_invalid(format!("invalid edit parameters: {e}")))?;
            let expected = Variant::new(
                spec.project_id.clone(),
                parent_variant.seed,
                ALGORITHM_VERSION.into(),
                params.parameters,
                params.storeys,
                params.floor_to_floor_m,
            )
            .map_err(|e| artifact_invalid(format!("edit geometry invalid: {e}")))?;
            if variants.len() != 1
                || variants[0] != expected
                || variants[0].id == parent_variant.id
                || manifest.seed != Some(parent_variant.seed)
                || manifest.count != Some(1)
            {
                return Err(artifact_invalid("edit derivation differs from parent"));
            }
        }
        "evaluate" => {
            let prior = parent.as_ref().expect("checked parent");
            if manifest.parent_variant_id.is_some()
                || manifest.seed.is_some()
                || manifest.count.is_some()
                || variants.is_empty()
            {
                return Err(artifact_invalid("invalid evaluation selectors"));
            }
            let mut ids = BTreeSet::new();
            for variant in &variants {
                if !ids.insert(&variant.id) || !prior.variants.contains(variant) {
                    return Err(artifact_invalid("evaluation variant differs from source"));
                }
            }
        }
        _ => unreachable!(),
    }
    let actual_paths: BTreeSet<String> = stored.artifacts().keys().cloned().collect();
    if actual_paths != expected_paths {
        return Err(artifact_invalid("operation artifact inventory differs"));
    }
    let context = report_context(&manifest, generation.as_ref(), &exhaustion)?;
    let report = render_report(&spec, &variants, &evaluations, &context)
        .map_err(|e| artifact_invalid(format!("report recomputation failed: {e}")))?;
    if stored.artifact("report.json") != Some(report.json.as_slice())
        || stored.artifact("report.md") != Some(report.markdown.as_slice())
        || stored.artifact("events.jsonl")
            != Some(event_bytes(&manifest.operation, &variants, &evaluations)?.as_slice())
    {
        return Err(artifact_invalid(
            "report or event bytes differ from recomputation",
        ));
    }
    Ok(TrustedRun {
        manifest,
        spec,
        input,
        variants,
        evaluations,
        exhaustion,
    })
}
