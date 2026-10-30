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
    source_schema_version: Option<&'static str>,
    input: Vec<u8>,
    variants: Vec<Variant>,
    evaluations: Vec<EvaluationReport>,
    exhaustion: Vec<String>,
    render_record: Option<crate::render::RenderRecord>,
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
    pub fn source_schema_version(&self) -> Option<&'static str> {
        self.source_schema_version
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
    pub fn render_record(&self) -> Option<&crate::render::RenderRecord> {
        self.render_record.as_ref()
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
    if manifest.operation == "export-ifc" {
        return verify_export_run(&stored, manifest, spec, canonical, input, parent);
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
        "render" => manifest.algorithm_version == crate::render::RENDER_VERSION,
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
        "edit" | "evaluate" | "render" => {
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
            if !(if manifest.operation == "render" {
                prior.manifest.operation == "evaluate"
            } else {
                matches!(prior.manifest.operation.as_str(), "generate" | "edit")
            }) || prior.spec != spec
                || (prior.canonical != canonical && !authority_upgrade)
                || prior.input != input
            {
                return Err(artifact_invalid("source run and input snapshot differ"));
            }
            Some(prior)
        }
        _ => return Err(artifact_invalid("invalid run parent references")),
    };
    if manifest.operation == "render" {
        return verify_render_run(
            stored,
            spec,
            canonical,
            source_schema_version,
            input,
            parent.expect("checked parent"),
            history
                .receipts()
                .find(|r| r.run_id == run_id)
                .ok_or_else(|| artifact_invalid("missing render receipt"))?
                .path
                .as_str(),
        );
    }
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
        source_schema_version,
        input,
        variants,
        evaluations,
        exhaustion,
        render_record: None,
    })
}

#[cfg(target_os = "linux")]
fn verify_render_run(
    stored: archy_core::VerifiedRun,
    spec: NormalizedSpec,
    canonical: Option<v2::NormalizedSpec>,
    source_schema_version: Option<&'static str>,
    input: Vec<u8>,
    parent: &TrustedRun,
    receipt_path: &str,
) -> Result<TrustedRun, ArchyError> {
    use crate::render::{RENDER_VERSION, RenderRecord, RenderResult};
    let manifest = stored.manifest().clone();
    if manifest.seed.is_some()
        || manifest.count.is_some()
        || manifest.parent_variant_id.is_some()
        || manifest.outcomes.is_empty()
    {
        return Err(artifact_invalid("invalid render metadata"));
    }
    let bytes = stored
        .artifact("render.json")
        .ok_or_else(|| artifact_invalid("missing render record"))?;
    let record: RenderRecord = serde_json::from_slice(bytes)
        .map_err(|e| artifact_invalid(format!("render record: {e}")))?;
    if record.schema_version != RENDER_VERSION
        || record.source_run_id != parent.manifest.run_id
        || record.source_manifest_sha256
            != sha256_hex(
                &serde_json::to_vec(&parent.manifest)
                    .map_err(|e| artifact_invalid(e.to_string()))?,
            )
        || serde_json::to_vec(&record).map_err(|e| artifact_invalid(e.to_string()))? != bytes
        || record.variants.len() != manifest.outcomes.len()
    {
        return Err(artifact_invalid("render source or record mismatch"));
    }
    let destination = receipt_path
        .strip_suffix(&format!("/{}", manifest.run_id))
        .ok_or_else(|| artifact_invalid("render receipt path differs"))?;
    let request_for = |selection: serde_json::Value| -> Result<String, ArchyError> {
        archy_core::digest_request(&archy_core::RunRequest {
            schema_version: manifest.request_schema_version.clone(),
            context: archy_core::MutationContext {
                idempotency_key: manifest.idempotency_key.clone(),
                expected_project_revision: manifest.expected_project_revision,
                destination: destination.to_owned(),
            },
            operation: "render".into(),
            request_bytes: serde_json::to_vec(&selection)
                .map_err(|e| artifact_invalid(e.to_string()))?,
            input_sha256: manifest.input_sha256.clone(),
            source_run: manifest.parent_run_id.clone(),
        })
        .map_err(|e| artifact_invalid(format!("render request fingerprint: {e}")))
    };
    let selected_ids: Vec<&str> = if request_for(serde_json::json!({"selector":"all"}))?
        == manifest.request_sha256
    {
        parent.variants.iter().map(|v| v.id.as_str()).collect()
    } else {
        let mut matched = None;
        for variant in &parent.variants {
            if request_for(serde_json::json!({"selector":"one","variant":variant.id}))?
                == manifest.request_sha256
            {
                matched = Some(variant.id.as_str());
                break;
            }
        }
        vec![matched.ok_or_else(|| artifact_invalid("render request selector digest differs"))?]
    };
    if record
        .variants
        .iter()
        .map(|v| v.variant_id.as_str())
        .collect::<Vec<_>>()
        != selected_ids
    {
        return Err(artifact_invalid(
            "render results differ from requested variants",
        ));
    }
    let input_path = match manifest.source_format.as_deref() {
        Some("json") => "input.json",
        Some("yaml") => "input.yaml",
        _ => return Err(artifact_invalid("render source format")),
    };
    let mut expected = BTreeSet::from([
        input_path.to_owned(),
        "normalized.json".into(),
        "render.json".into(),
    ]);
    if canonical.is_some() {
        expected.insert("profile.json".into());
    }
    let mut variants = Vec::new();
    let mut evaluations = Vec::new();
    for (item, outcome) in record.variants.iter().zip(&manifest.outcomes) {
        if item.variant_id != outcome.variant_id {
            return Err(artifact_invalid("render result identity"));
        }
        let variant_path = format!("variants/{}.json", item.variant_id);
        let eval_path = format!("evaluations/{}.json", item.variant_id);
        let error_path = format!("errors/{}.json", item.variant_id);
        if outcome.variant_artifact.as_deref() != Some(&variant_path) {
            return Err(artifact_invalid("render failure outcome mismatch"));
        }
        let (variant, evaluation) = parent
            .variants
            .iter()
            .zip(&parent.evaluations)
            .find(|(v, _)| v.id == item.variant_id)
            .ok_or_else(|| artifact_invalid("rendered variant absent from evaluation"))?;
        if variants.iter().any(|v: &Variant| v.id == variant.id)
            || item.required_checks_passed != evaluation.required_checks_passed
            || stored.artifact(&variant_path)
                != Some(
                    serde_json::to_vec(variant)
                        .map_err(|e| artifact_invalid(e.to_string()))?
                        .as_slice(),
                )
            || stored.artifact(&eval_path)
                != Some(
                    serde_json::to_vec(evaluation)
                        .map_err(|e| artifact_invalid(e.to_string()))?
                        .as_slice(),
                )
        {
            return Err(artifact_invalid(
                "render snapshots differ from evaluated source",
            ));
        }
        expected.extend([variant_path, eval_path.clone()]);
        match &item.result {
            RenderResult::Failed {
                error,
                worker_response_sha256,
            } => {
                if outcome.evaluation_artifact.is_some()
                    || outcome.error_artifact.as_deref() != Some(&error_path)
                    || !matches!(
                        error.code,
                        archy_core::ErrorCode::BackendUnavailable
                            | archy_core::ErrorCode::BackendTimeout
                            | archy_core::ErrorCode::ArtifactInvalid
                            | archy_core::ErrorCode::InternalError
                            | archy_core::ErrorCode::InvalidGeometry
                            | archy_core::ErrorCode::InvalidSchema
                    )
                    || worker_response_sha256.as_ref().is_some_and(|h| {
                        h.len() != 64
                            || !h
                                .bytes()
                                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    })
                    || stored.artifact(&error_path)
                        != Some(
                            serde_json::to_vec(item)
                                .map_err(|e| artifact_invalid(e.to_string()))?
                                .as_slice(),
                        )
                {
                    return Err(artifact_invalid("render failure record differs"));
                }
                expected.insert(error_path);
            }
            RenderResult::Verified {
                verifier_version,
                artifacts,
                preview_foreground_pixels,
                preview_symbol,
            } => {
                if verifier_version != crate::render_verify::VERIFIER_VERSION
                    || outcome.evaluation_artifact.as_deref() != Some(&eval_path)
                    || outcome.error_artifact.is_some()
                    || artifacts.len() != 6
                {
                    return Err(artifact_invalid("render verification metadata"));
                }
                let prefix = format!("renders/{}/", item.variant_id);
                let worker_request = archy_adapters::BlenderRequest::new(
                    &spec,
                    variant,
                    &manifest.run_id,
                    &parent.manifest.run_id,
                    &sha256_hex(&input),
                    "output",
                )
                .map_err(|e| artifact_invalid(format!("render request reconstruction: {e}")))?;
                let response_bytes = stored
                    .artifact(&format!("{prefix}response.json"))
                    .ok_or_else(|| artifact_invalid("missing render worker response"))?;
                let worker_response =
                    archy_adapters::decode_blender_response(response_bytes, &worker_request)
                        .map_err(|e| artifact_invalid(format!("render worker response: {e}")))?;
                let claims = match worker_response {
                    archy_adapters::BlenderResponse::Ok(success) => {
                        let views = success.screen_width_edge_views();
                        let expected_symbol =
                            (!views.is_empty()).then(|| crate::render::RenderPreviewSymbol {
                                views,
                                notice: crate::render::SCREEN_WIDTH_EDGE_NOTICE.into(),
                            });
                        if *preview_symbol != expected_symbol {
                            return Err(artifact_invalid(
                                "render preview symbol differs from worker response",
                            ));
                        }
                        success.artifacts
                    }
                    archy_adapters::BlenderResponse::Error(_) => {
                        return Err(artifact_invalid(
                            "verified render has worker error response",
                        ));
                    }
                };
                let names = [
                    "response.json",
                    "scene.blend",
                    "model.glb",
                    "view-top.png",
                    "view-front.png",
                    "view-iso.png",
                ];
                for (entry, name) in artifacts.iter().zip(names) {
                    let expected_media = match name {
                        "response.json" => "application/json",
                        "scene.blend" => "application/x-blender",
                        "model.glb" => "model/gltf-binary",
                        _ => "image/png",
                    };
                    if entry.path != format!("{prefix}{name}")
                        || entry.media_type != expected_media
                        || entry.producer != "blender-worker-v1"
                        || stored.artifact(&entry.path).is_none_or(|bytes| {
                            bytes.len() as u64 != entry.byte_len
                                || sha256_hex(bytes) != entry.sha256
                        })
                        || !manifest.artifacts.iter().any(|a| {
                            a.path == entry.path
                                && a.sha256 == entry.sha256
                                && a.byte_len == entry.byte_len
                                && a.media_type == entry.media_type
                                && a.producer == entry.producer
                        })
                    {
                        return Err(artifact_invalid("render artifact bytes or provenance"));
                    }
                    expected.insert(entry.path.clone());
                }
                for (claim, entry) in claims.iter().zip(&artifacts[1..]) {
                    if claim.path != entry.path.strip_prefix(&prefix).unwrap_or("")
                        || claim.sha256 != entry.sha256
                        || claim.size_bytes != entry.byte_len
                        || claim.media_type != entry.media_type
                        || claim.producer != entry.producer
                    {
                        return Err(artifact_invalid(
                            "render worker claim differs from captured bytes",
                        ));
                    }
                }
                let checked = crate::render_verify::verify_bytes(
                    |name| stored.artifact(&format!("{prefix}{name}")),
                    variant,
                    evaluation,
                )?;
                if &checked != preview_foreground_pixels {
                    return Err(artifact_invalid("render verification result changed"));
                }
            }
        }
        variants.push(variant.clone());
        evaluations.push(evaluation.clone());
    }
    if stored.artifacts().keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(artifact_invalid("render artifact inventory differs"));
    }
    Ok(TrustedRun {
        manifest,
        spec,
        canonical,
        source_schema_version,
        input,
        variants,
        evaluations,
        exhaustion: Vec::new(),
        render_record: Some(record),
    })
}

fn verify_export_run(
    stored: &archy_core::VerifiedRun,
    manifest: RunManifest,
    spec: NormalizedSpec,
    canonical: Option<v2::NormalizedSpec>,
    input: Vec<u8>,
    parent: Option<&TrustedRun>,
) -> Result<TrustedRun, ArchyError> {
    let prior = parent.ok_or_else(|| artifact_invalid("missing verified IFC source"))?;
    if canonical != prior.canonical
        || manifest.source_format != prior.manifest.source_format
        || !matches!(prior.manifest.operation.as_str(), "generate" | "edit")
        || manifest.parent_run_id.as_deref() != Some(&prior.manifest.run_id)
        || prior.input != input
        || prior.spec != spec
        || manifest.algorithm_version != crate::DESIGN_MANIFEST_VERSION
        || manifest.seed.is_some()
        || manifest.count.is_some()
    {
        return Err(artifact_invalid("invalid IFC source binding"));
    }
    let variant_id = manifest
        .parent_variant_id
        .as_deref()
        .ok_or_else(|| artifact_invalid("missing IFC variant"))?;
    let variant = prior
        .variants
        .iter()
        .find(|v| v.id == variant_id)
        .ok_or_else(|| artifact_invalid("IFC variant absent from source"))?;
    let evaluation = if canonical.is_some() {
        evaluate_variant_v2(&spec, variant)?
    } else {
        archy_eval::evaluate_variant(&spec, variant)?
    };
    let variant_path = format!("variants/{variant_id}.json");
    let evaluation_path = format!("evaluations/{variant_id}.json");
    let variant_bytes = serde_json::to_vec(variant).map_err(|e| artifact_invalid(e.to_string()))?;
    let evaluation_bytes =
        serde_json::to_vec(&evaluation).map_err(|e| artifact_invalid(e.to_string()))?;
    if stored.artifact(&variant_path) != Some(variant_bytes.as_slice())
        || stored.artifact(&evaluation_path) != Some(evaluation_bytes.as_slice())
        || manifest.outcomes.len() != 1
        || manifest.outcomes[0].variant_id != variant_id
        || manifest.outcomes[0].variant_artifact.as_deref() != Some(&variant_path)
        || manifest.outcomes[0].evaluation_artifact.as_deref() != Some(&evaluation_path)
        || manifest.outcomes[0].error_artifact.is_some()
    {
        return Err(artifact_invalid("IFC variant or checks differ from source"));
    }
    let design_bytes = stored
        .artifact("design-manifest.json")
        .ok_or_else(|| artifact_invalid("missing design manifest"))?;
    let design: crate::DesignManifest = serde_json::from_slice(design_bytes)
        .map_err(|e| artifact_invalid(format!("invalid design manifest: {e}")))?;
    let model = stored
        .artifact("ifc/model.ifc")
        .ok_or_else(|| artifact_invalid("missing IFC model"))?;
    let validation = stored
        .artifact("ifc/validation.json")
        .ok_or_else(|| artifact_invalid("missing IFC validation"))?;
    let response = stored
        .artifact("ifc/response.json")
        .ok_or_else(|| artifact_invalid("missing IFC response"))?;
    let response_json: serde_json::Value =
        serde_json::from_slice(response).map_err(|e| artifact_invalid(e.to_string()))?;
    let validation_json: serde_json::Value =
        serde_json::from_slice(validation).map_err(|e| artifact_invalid(e.to_string()))?;
    let request_id = response_json["request_id"]
        .as_str()
        .ok_or_else(|| artifact_invalid("missing worker request ID"))?;
    let request = archy_adapters::IfcRequest::new(
        &spec,
        variant,
        request_id,
        &prior.manifest.run_id,
        &sha256_hex(&input),
        "output/ifc",
        Vec::new(),
    )?;
    let decoded = archy_adapters::decode_ifc_response(response, &request)?;
    let archy_adapters::IfcResponse::Ok(success) = decoded else {
        return Err(artifact_invalid("IFC response is not successful"));
    };
    for (path, bytes) in [("model.ifc", model), ("validation.json", validation)] {
        let claim = success
            .artifacts
            .iter()
            .find(|artifact| artifact.path == path)
            .ok_or_else(|| artifact_invalid("missing IFC artifact claim"))?;
        if claim.sha256 != sha256_hex(bytes) || claim.size_bytes != bytes.len() as u64 {
            return Err(artifact_invalid(
                "IFC artifact claim differs from stored bytes",
            ));
        }
    }
    if response_json["validation"] != validation_json
        || design.schema_version != crate::DESIGN_MANIFEST_VERSION
        || design.scope != "SCHEMATIC_MASSING_ONLY"
        || design.approval_state != "DRAFT"
        || design.limitations != crate::export_ifc::DESIGN_LIMITATIONS
        || design.project_id != spec.project_id
        || design.source_run_id != prior.manifest.run_id
        || design.export_run_id != manifest.run_id
        || design.variant_id != variant_id
        || design.source_input_sha256 != sha256_hex(&input)
        || design.source_normalized_sha256
            != manifest.normalized_sha256.as_deref().unwrap_or_default()
        || design.variant_sha256 != sha256_hex(&variant_bytes)
        || design.evaluation_sha256 != sha256_hex(&evaluation_bytes)
        || design.required_checks_passed != evaluation.required_checks_passed
        || design.evaluation != evaluation
        || design.worker_response_sha256 != sha256_hex(response)
        || design.model_sha256 != sha256_hex(model)
        || design.validation_sha256 != sha256_hex(validation)
        || design.worker_version != response_json["worker_version"].as_str().unwrap_or_default()
        || design.backend_version
            != response_json["backend_version"]
                .as_str()
                .unwrap_or_default()
        || design.python_version != response_json["python_version"].as_str().unwrap_or_default()
    {
        return Err(artifact_invalid(
            "IFC design manifest differs from verified bytes",
        ));
    }
    let expected: BTreeSet<String> = [
        if manifest.source_format.as_deref() == Some("json") {
            "input.json"
        } else {
            "input.yaml"
        },
        "normalized.json",
        "design-manifest.json",
        "ifc/model.ifc",
        "ifc/validation.json",
        "ifc/response.json",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain([variant_path, evaluation_path])
    .collect();
    let mut expected = expected;
    if canonical.is_some() {
        expected.insert("profile.json".into());
    }
    if stored.artifacts().keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(artifact_invalid("IFC artifact inventory differs"));
    }
    crate::ifc_verifier::verify(
        model,
        &input,
        &spec,
        canonical.as_ref(),
        manifest.source_format.as_deref().unwrap_or_default(),
        variant,
        &prior.manifest.run_id,
    )?;
    Ok(TrustedRun {
        manifest,
        spec,
        canonical,
        source_schema_version: prior.source_schema_version,
        input,
        variants: vec![variant.clone()],
        evaluations: vec![evaluation],
        exhaustion: Vec::new(),
        render_record: None,
    })
}
