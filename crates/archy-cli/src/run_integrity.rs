use crate::{artifact_invalid, generate::EditParameters};
use archy_core::project::v2;
use archy_core::{
    AcceptedHistory, ArchyError, CurrentProject, EvaluationReport, InputFormat, LockedProject,
    NormalizedSpec, ProjectStore, RunManifest, Variant, parse_project_current, parse_project_v1,
    sha256_hex,
};
use archy_eval::{
    AUTHORITY_EVALUATION_VERSION, EVALUATION_VERSION, evaluate_variant_v1, evaluate_variant_v2,
};
use archy_generate::{
    ALGORITHM_VERSION, DIAGNOSTIC_ALGORITHM_VERSION, GenerationResult, generate_variants,
    generate_variants_v1,
};
use archy_report::{REPORT_CONTEXT_VERSION, ReportContext, render_report, render_report_v2};
use std::{collections::BTreeSet, path::Path};

/// The inventoried profile fixes the supported tuple for every canonical v2 run.
/// Historical v1 runs have no such artifact and retain their exact inventory.
pub(crate) fn profile_bytes(
    operation: &str,
    algorithm: &str,
    source_schema_version: &str,
) -> Result<Vec<u8>, ArchyError> {
    serde_json::to_vec(&serde_json::json!({
        "schema_version": "archy-run-profile-v1",
        "source_schema_version": source_schema_version,
        "normalized_schema_version": "0.2.0",
        "normalization_version": "si-rectangle-v2",
        "operation": operation,
        "algorithm_version": algorithm,
        "evaluation_version": AUTHORITY_EVALUATION_VERSION,
        "report_version": archy_report::AUTHORITY_REPORT_SCHEMA_VERSION,
    }))
    .map_err(|e| artifact_invalid(format!("serialize run profile: {e}")))
}

#[derive(Debug)]
pub struct TrustedRun {
    manifest: RunManifest,
    spec: NormalizedSpec,
    canonical: Option<v2::NormalizedSpec>,
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
    pub fn canonical(&self) -> Option<&v2::NormalizedSpec> {
        self.canonical.as_ref()
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

fn has_claimed_regulation(spec: &NormalizedSpec) -> bool {
    spec.constraints
        .iter()
        .any(|constraint| constraint.source == "claimed_regulation")
}

pub(crate) fn migrate_claimed_v1_source(
    source: &TrustedRun,
) -> Result<Option<CurrentProject>, ArchyError> {
    if source.canonical.is_some() || !has_claimed_regulation(&source.spec) {
        return Ok(None);
    }
    let format = match source.manifest.source_format.as_deref() {
        Some("json") => InputFormat::Json,
        Some("yaml") => InputFormat::Yaml,
        _ => return Err(artifact_invalid("unsupported source format")),
    };
    let text = std::str::from_utf8(&source.input)
        .map_err(|_| artifact_invalid("historical source is not UTF-8"))?;
    let current = parse_project_current(text, format).map_err(|error| {
        ArchyError::schema(
            error.field,
            format!(
                "Cannot migrate historical claimed-regulation source to v2 authority profile: {}",
                error.message
            ),
        )
    })?;
    if current.numeric != source.spec {
        return Err(artifact_invalid(
            "v2 authority migration changed historical numeric source",
        ));
    }
    Ok(Some(current))
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
        attempt_diagnostics: generation.and_then(|g| g.attempt_diagnostics.clone()),
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

pub fn verify_history(
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
    let recorded_normalized = stored
        .artifact("normalized.json")
        .ok_or_else(|| artifact_invalid("missing normalized snapshot"))?;
    let recorded: serde_json::Value = serde_json::from_slice(recorded_normalized)
        .map_err(|_| artifact_invalid("invalid normalized snapshot"))?;
    let profile = (
        recorded.get("schema_version").and_then(|v| v.as_str()),
        recorded
            .get("normalization_version")
            .and_then(|v| v.as_str()),
    );
    let (spec, canonical, source_schema_version) = match profile {
        (Some("0.1.0"), Some("si-rectangle-v1")) => (
            parse_project_v1(source, format)
                .map_err(|e| artifact_invalid(format!("stored source invalid: {e}")))?,
            None,
            None,
        ),
        (Some("0.2.0"), Some("si-rectangle-v2")) => {
            let current = parse_project_current(source, format)
                .map_err(|e| artifact_invalid(format!("stored source invalid: {e}")))?;
            (
                current.numeric,
                Some(current.canonical),
                Some(current.source_schema_version),
            )
        }
        _ => {
            return Err(artifact_invalid(
                "unsupported recorded normalization profile",
            ));
        }
    };
    if spec.project_id != manifest.project_id {
        return Err(artifact_invalid("project ID mismatch"));
    }
    let normalized = match &canonical {
        Some(value) => serde_json::to_vec(value),
        None => serde_json::to_vec(&spec),
    }
    .map_err(|e| artifact_invalid(format!("serialize normalized: {e}")))?;
    if stored.artifact("normalized.json") != Some(normalized.as_slice())
        || manifest.normalized_sha256.as_deref() != Some(sha256_hex(&normalized).as_str())
    {
        return Err(artifact_invalid("source and normalized snapshot differ"));
    }
    if let Some(source_schema_version) = source_schema_version {
        let expected = profile_bytes(
            &manifest.operation,
            &manifest.algorithm_version,
            source_schema_version,
        )?;
        if stored.artifact("profile.json") != Some(expected.as_slice()) {
            return Err(artifact_invalid(
                "run profile disagrees with operation and source",
            ));
        }
    } else if stored.artifact("profile.json").is_some() {
        return Err(artifact_invalid("historical v1 run has a current profile"));
    }
    let supported = match manifest.operation.as_str() {
        "generate" => matches!(
            manifest.algorithm_version.as_str(),
            ALGORITHM_VERSION | DIAGNOSTIC_ALGORITHM_VERSION
        ),
        "edit" => manifest.algorithm_version == ALGORITHM_VERSION,
        "evaluate" => matches!(
            manifest.algorithm_version.as_str(),
            EVALUATION_VERSION | AUTHORITY_EVALUATION_VERSION
        ),
        _ => return Err(artifact_invalid("unsupported run operation")),
    };
    if !supported {
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
            let authority_upgrade = prior.canonical.is_none()
                && canonical.is_some()
                && has_claimed_regulation(&prior.spec);
            if !matches!(prior.manifest.operation.as_str(), "generate" | "edit")
                || prior.spec != spec
                || (prior.canonical != canonical && !authority_upgrade)
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
        let expected = if manifest.algorithm_version == ALGORITHM_VERSION {
            generate_variants_v1(&spec, count, seed)
        } else {
            generate_variants(&spec, count, seed)
        }
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
    if canonical.is_some() {
        expected_paths.insert("profile.json".into());
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
        if manifest.operation == "evaluate"
            && evaluation.evaluation_version != manifest.algorithm_version
        {
            return Err(artifact_invalid("evaluation version differs from manifest"));
        }
        let expected_evaluation_version = if canonical.is_some() {
            AUTHORITY_EVALUATION_VERSION
        } else {
            EVALUATION_VERSION
        };
        if evaluation.evaluation_version != expected_evaluation_version {
            return Err(artifact_invalid(
                "evaluation profile disagrees with source profile",
            ));
        }
        let recomputed = if canonical.is_some() {
            evaluate_variant_v2(&spec, &variant)
        } else {
            evaluate_variant_v1(&spec, &variant)
        }
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
    if manifest.operation == "evaluate"
        && manifest.algorithm_version
            != if canonical.is_some() {
                AUTHORITY_EVALUATION_VERSION
            } else {
                EVALUATION_VERSION
            }
    {
        return Err(artifact_invalid(
            "operation algorithm disagrees with source profile",
        ));
    }
    let report = match &canonical {
        Some(value) => render_report_v2(&spec, value, &variants, &evaluations, &context),
        None => render_report(&spec, &variants, &evaluations, &context),
    }
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
        canonical,
        input,
        variants,
        evaluations,
        exhaustion,
    })
}
