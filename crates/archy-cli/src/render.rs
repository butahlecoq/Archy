//! Render orchestration keeps the project lock from source selection through publication.
use crate::{
    MutationOptions, VariantSelection, artifact_invalid, recorded_manifest_for_key,
    request_conflict, run_integrity::verify_locked,
};
use archy_adapters::{BlenderRequest, CaptureOutcome};
use archy_core::{
    ArchyError, ErrorCode, LockedProject, ProjectStore, RUN_REQUEST_SCHEMA_VERSION, RunMetadata,
    RunReceipt, RunRequest, sha256_hex,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::atomic::AtomicBool};

pub const RENDER_VERSION: &str = "archy-render-v1";
pub const SCREEN_WIDTH_EDGE_NOTICE: &str =
    "Screen-width massing edges are nonphysical preview symbols; GLB dimensions remain exact.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderPreviewSymbol {
    pub views: Vec<String>,
    pub notice: String,
}

#[derive(Debug, Clone)]
pub struct RenderInput {
    pub project: PathBuf,
    pub source_run: Option<String>,
    pub selection: VariantSelection,
    pub mutation: MutationOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderVariant {
    pub variant_id: String,
    pub required_checks_passed: bool,
    pub result: RenderResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RenderResult {
    Verified {
        verifier_version: String,
        artifacts: Vec<RenderArtifact>,
        preview_foreground_pixels: Vec<(String, u32)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preview_symbol: Option<RenderPreviewSymbol>,
    },
    Failed {
        error: ArchyError,
        worker_response_sha256: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderArtifact {
    pub path: String,
    pub sha256: String,
    pub byte_len: u64,
    pub media_type: String,
    pub producer: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderRecord {
    pub schema_version: String,
    pub source_run_id: String,
    pub source_manifest_sha256: String,
    pub variants: Vec<RenderVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderReceipt {
    pub run_id: String,
    pub revision: u64,
    pub path: String,
    pub manifest_sha256: String,
    pub record: RenderRecord,
}

fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, ArchyError> {
    serde_json::to_vec(value).map_err(|e| artifact_invalid(format!("render serialization: {e}")))
}

fn receipt(
    locked: &archy_core::LockedProject<'_>,
    receipt: RunReceipt,
) -> Result<RenderReceipt, ArchyError> {
    verify_locked(locked, &receipt.run_id)?;
    let run = locked.read_run(&receipt.run_id)?;
    let bytes = run
        .artifact("render.json")
        .ok_or_else(|| artifact_invalid("missing render record"))?;
    let record: RenderRecord = serde_json::from_slice(bytes)
        .map_err(|e| artifact_invalid(format!("render record: {e}")))?;
    if run.manifest().operation != "render" || serialize(&record)? != bytes {
        return Err(artifact_invalid(
            "render receipt does not match canonical record",
        ));
    }
    Ok(RenderReceipt {
        run_id: receipt.run_id,
        revision: receipt.revision,
        path: receipt.path,
        manifest_sha256: receipt.manifest_sha256,
        record,
    })
}

pub fn render(input: RenderInput) -> Result<RenderReceipt, ArchyError> {
    render_with_worker(input, crate::accepted_worker::run_blender_request)
}

fn render_with_worker(
    input: RenderInput,
    worker: impl FnMut(BlenderRequest, &AtomicBool) -> Result<CaptureOutcome, ArchyError>,
) -> Result<RenderReceipt, ArchyError> {
    let store = ProjectStore::open(&input.project)?;
    let locked = store.lock()?;
    render_locked_with_worker(
        &locked,
        input.source_run,
        input.selection,
        input.mutation,
        &|| Ok(()),
        worker,
    )
}

pub(crate) fn render_locked(
    locked: &LockedProject<'_>,
    source_run: String,
    variant_id: String,
    mutation: MutationOptions,
    check_binding: &dyn Fn() -> Result<(), ArchyError>,
) -> Result<RenderReceipt, ArchyError> {
    render_locked_with_worker(
        locked,
        Some(source_run),
        VariantSelection::One(variant_id),
        mutation,
        check_binding,
        crate::accepted_worker::run_blender_request,
    )
}

fn render_locked_with_worker(
    locked: &LockedProject<'_>,
    source_run: Option<String>,
    selection: VariantSelection,
    mutation: MutationOptions,
    check_binding: &dyn Fn() -> Result<(), ArchyError>,
    mut worker: impl FnMut(BlenderRequest, &AtomicBool) -> Result<CaptureOutcome, ArchyError>,
) -> Result<RenderReceipt, ArchyError> {
    check_binding()?;
    let context = mutation.resolve(locked)?;
    let recorded = recorded_manifest_for_key(locked, mutation.idempotency_key.as_deref())?;
    if recorded.as_ref().is_some_and(|m| m.operation != "render") {
        return Err(request_conflict());
    }
    let source_id =
        match source_run.or_else(|| recorded.as_ref().and_then(|m| m.parent_run_id.clone())) {
            Some(id) => id,
            None => {
                let mut latest = None;
                for item in locked.receipts()?.iter().rev() {
                    if locked.read_run(&item.run_id)?.manifest().operation == "evaluate" {
                        latest = Some(item.run_id.clone());
                        break;
                    }
                }
                latest.ok_or_else(|| ArchyError::schema("run", "No accepted evaluation run"))?
            }
        };
    if recorded
        .as_ref()
        .is_some_and(|m| m.parent_run_id.as_deref() != Some(&source_id))
    {
        return Err(request_conflict());
    }
    let selector = match &selection {
        VariantSelection::All => serde_json::json!({"selector":"all"}),
        VariantSelection::One(id) => serde_json::json!({"selector":"one","variant":id}),
    };
    let request = RunRequest {
        schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
        context,
        operation: "render".into(),
        request_bytes: serialize(&selector)?,
        input_sha256: recorded.as_ref().map_or_else(
            || Ok::<_, ArchyError>(locked.read_run(&source_id)?.manifest().input_sha256.clone()),
            |m| Ok(m.input_sha256.clone()),
        )?,
        source_run: Some(source_id.clone()),
    };
    if let Some(recorded) = &recorded
        && recorded.request_sha256 != archy_core::digest_request(&request)?
    {
        return Err(request_conflict());
    }
    let source = verify_locked(locked, &source_id)?;
    if source.manifest().operation != "evaluate" {
        return Err(artifact_invalid(
            "render source must be an accepted evaluation run",
        ));
    }
    if source.variants().is_empty() || source.variants().len() != source.evaluations().len() {
        return Err(artifact_invalid("evaluation run has no complete variants"));
    }
    let selected: Vec<_> = source
        .variants()
        .iter()
        .zip(source.evaluations())
        .filter(|(v, _)| match &selection {
            VariantSelection::All => true,
            VariantSelection::One(id) => v.id == *id,
        })
        .collect();
    if selected.is_empty() {
        return Err(ArchyError::schema(
            "variant",
            "Variant is absent from evaluated run",
        ));
    }
    let mut pending = match locked.begin_checked(request, check_binding)? {
        archy_core::BeginResult::Replay(replayed) => return receipt(locked, replayed),
        archy_core::BeginResult::Pending(pending) => pending,
    };
    let normalized = match source.canonical() {
        Some(canonical) => serialize(canonical)?,
        None => serialize(source.spec())?,
    };
    let format = source
        .manifest()
        .source_format
        .as_deref()
        .ok_or_else(|| artifact_invalid("missing source format"))?;
    let input_path = match format {
        "json" => "input.json",
        "yaml" => "input.yaml",
        _ => return Err(artifact_invalid("unsupported source format")),
    };
    pending.write(
        input_path,
        source.input(),
        "application/octet-stream",
        "archy-cli",
    )?;
    pending.write(
        "normalized.json",
        &normalized,
        "application/json",
        "archy-core",
    )?;
    if source.canonical().is_some() {
        let format = if format == "json" {
            archy_core::InputFormat::Json
        } else {
            archy_core::InputFormat::Yaml
        };
        let parsed = archy_core::parse_project_current(
            std::str::from_utf8(source.input())
                .map_err(|_| artifact_invalid("source is not UTF-8"))?,
            format,
        )?;
        let profile = crate::run_integrity::profile_bytes(
            "render",
            RENDER_VERSION,
            parsed.source_schema_version,
        )?;
        pending.write("profile.json", &profile, "application/json", "archy-cli")?;
    }
    let mut results = Vec::new();
    let mut stop = false;
    let cancel = AtomicBool::new(false);
    for (variant, evaluation) in selected {
        let variant_path = format!("variants/{}.json", variant.id);
        let evaluation_path = format!("evaluations/{}.json", variant.id);
        let error_path = format!("errors/{}.json", variant.id);
        pending.write(
            &variant_path,
            &serialize(variant)?,
            "application/json",
            "archy-core",
        )?;
        pending.write(
            &evaluation_path,
            &serialize(evaluation)?,
            "application/json",
            "archy-eval",
        )?;
        let result = if stop {
            RenderResult::Failed {
                error: ArchyError::new(
                    ErrorCode::BackendUnavailable,
                    "render",
                    "previous worker teardown or verification failed",
                ),
                worker_response_sha256: None,
            }
        } else {
            let request = BlenderRequest::new(
                source.spec(),
                variant,
                pending.run_id(),
                &source_id,
                &sha256_hex(source.input()),
                "output",
            )?;
            match worker(request.clone(), &cancel) {
                Ok(CaptureOutcome::WorkerError(failure)) => RenderResult::Failed {
                    error: failure.diagnostic_error().clone(),
                    worker_response_sha256: Some(failure.response_sha256().to_owned()),
                },
                Ok(CaptureOutcome::Success(captured)) => {
                    match crate::render_verify::verify_captured(&captured, variant, evaluation) {
                        Ok(previews) => {
                            let response = archy_adapters::decode_blender_response(
                                captured.response_bytes(),
                                &request,
                            )?;
                            let views = match response {
                                archy_adapters::BlenderResponse::Ok(success) => {
                                    success.screen_width_edge_views()
                                }
                                archy_adapters::BlenderResponse::Error(_) => {
                                    return Err(artifact_invalid("verified worker returned error"));
                                }
                            };
                            let preview_symbol = (!views.is_empty()).then(|| RenderPreviewSymbol {
                                views,
                                notice: SCREEN_WIDTH_EDGE_NOTICE.into(),
                            });
                            let mut artifacts = Vec::new();
                            let prefix = format!("renders/{}/", variant.id);
                            let response_path = format!("{prefix}response.json");
                            pending.write(
                                &response_path,
                                captured.response_bytes(),
                                "application/json",
                                "blender-worker-v1",
                            )?;
                            artifacts.push(RenderArtifact {
                                path: response_path,
                                sha256: captured.response_sha256().into(),
                                byte_len: captured.response_bytes().len() as u64,
                                media_type: "application/json".into(),
                                producer: "blender-worker-v1".into(),
                            });
                            for artifact in captured.artifacts() {
                                let path = format!("{prefix}{}", artifact.path());
                                pending.write(
                                    &path,
                                    artifact.bytes(),
                                    artifact.media_type(),
                                    artifact.producer(),
                                )?;
                                artifacts.push(RenderArtifact {
                                    path,
                                    sha256: artifact.sha256().into(),
                                    byte_len: artifact.bytes().len() as u64,
                                    media_type: artifact.media_type().into(),
                                    producer: artifact.producer().into(),
                                });
                            }
                            RenderResult::Verified {
                                verifier_version: crate::render_verify::VERIFIER_VERSION.into(),
                                artifacts,
                                preview_foreground_pixels: previews,
                                preview_symbol,
                            }
                        }
                        Err(error) => {
                            stop = true;
                            RenderResult::Failed {
                                error,
                                worker_response_sha256: Some(captured.response_sha256().into()),
                            }
                        }
                    }
                }
                Err(error) => {
                    stop = true;
                    RenderResult::Failed {
                        error,
                        worker_response_sha256: None,
                    }
                }
            }
        };
        let item = RenderVariant {
            variant_id: variant.id.clone(),
            required_checks_passed: evaluation.required_checks_passed,
            result,
        };
        let outcome = match &item.result {
            RenderResult::Verified { .. } => archy_core::VariantOutcome {
                variant_id: variant.id.clone(),
                variant_artifact: Some(variant_path),
                evaluation_artifact: Some(evaluation_path),
                error_artifact: None,
            },
            RenderResult::Failed { .. } => {
                pending.write(
                    &error_path,
                    &serialize(&item)?,
                    "application/json",
                    "archy-cli",
                )?;
                archy_core::VariantOutcome {
                    variant_id: variant.id.clone(),
                    variant_artifact: Some(variant_path),
                    evaluation_artifact: None,
                    error_artifact: Some(error_path),
                }
            }
        };
        pending.record_outcome(outcome)?;
        results.push(item);
    }
    let record = RenderRecord {
        schema_version: RENDER_VERSION.into(),
        source_run_id: source_id,
        source_manifest_sha256: sha256_hex(&serialize(source.manifest())?),
        variants: results,
    };
    pending.write(
        "render.json",
        &serialize(&record)?,
        "application/json",
        "archy-cli",
    )?;
    check_binding()?;
    let committed = pending.commit_checked(
        RunMetadata {
            project_id: source.spec().project_id.clone(),
            normalized_sha256: Some(sha256_hex(&normalized)),
            source_format: Some(format.into()),
            seed: None,
            count: None,
            parent_variant_id: None,
            algorithm_version: RENDER_VERSION.into(),
        },
        check_binding,
    )?;
    receipt(locked, committed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvaluateInput, GenerateInput, GenerationMode, evaluate, generate};
    use archy_adapters::capture_blender;
    use std::cell::Cell;
    use std::{fs, fs::File, process::Command};

    #[test]
    fn real_thin_render_discloses_symbols_in_receipt_desktop_and_replay() {
        let Ok(blender) = std::env::var("ARCHY_BLENDER_BIN") else {
            return;
        };
        let root = std::env::temp_dir().join(format!("archy-real-thin-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let project = root.join("project.json");
        let mut source: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../tests/fixtures/briefs/courtyard.json"
        ))
        .unwrap();
        source["site"]["width_m"] = 10.into();
        source["site"]["depth_m"] = serde_json::json!(0.001);
        source["brief"]["storeys"] = 1.into();
        source["brief"]["target_gfa_m2"] = serde_json::json!(0.003);
        source["constraints"] = serde_json::json!([]);
        source["generation"]["shapes"] = serde_json::json!(["rectangle"]);
        fs::write(&project, serde_json::to_vec(&source).unwrap()).unwrap();
        let generated = generate(GenerateInput {
            project: project.clone(),
            mode: GenerationMode::Sample { count: 1, seed: 42 },
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
        let input = RenderInput {
            project: project.clone(),
            source_run: Some(evaluated.run_id),
            selection: VariantSelection::All,
            mutation: MutationOptions {
                idempotency_key: Some(uuid::Uuid::new_v4().to_string()),
                expected_project_revision: Some(evaluated.revision),
                ..MutationOptions::default()
            },
        };
        let receipt = render_with_worker(input.clone(), |request, _| {
            let output = root.join(format!("worker-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&output).unwrap();
            let request_path = root.join(format!("request-{}.json", uuid::Uuid::new_v4()));
            fs::write(&request_path, request.to_bytes()?).unwrap();
            let worker = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../workers/blender/worker.py");
            let result = Command::new(&blender)
                .args([
                    "--background",
                    "--factory-startup",
                    "--disable-autoexec",
                    "--offline-mode",
                    "--python-exit-code",
                    "20",
                    "--threads",
                    "1",
                    "--python",
                ])
                .arg(worker)
                .args(["--", "--request"])
                .arg(&request_path)
                .arg("--response")
                .arg(output.join("response.json"))
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            let captured = capture_blender(&File::open(&output).unwrap(), &request)?;
            fs::remove_dir_all(output).unwrap();
            fs::remove_file(request_path).unwrap();
            Ok(captured)
        })
        .unwrap();
        let symbol = match &receipt.record.variants[0].result {
            RenderResult::Verified {
                preview_symbol: Some(symbol),
                ..
            } => symbol,
            other => panic!("expected disclosed verified render: {other:?}"),
        };
        assert!(!symbol.views.is_empty());
        assert_eq!(symbol.notice, SCREEN_WIDTH_EDGE_NOTICE);
        assert_eq!(render(input).unwrap(), receipt);
        let desktop = crate::desktop_status(crate::DesktopStatusInput {
            project: project.clone(),
            run: Some(receipt.run_id.clone()),
            limit: 10,
            cursor: None,
        })
        .unwrap();
        let summary = &desktop.selected_run.unwrap().summary;
        assert_eq!(summary.preview_symbols.len(), 1);
        assert_eq!(
            summary.preview_symbols[0].variant_id,
            receipt.record.variants[0].variant_id
        );
        assert_eq!(summary.preview_symbols[0].views, symbol.views);
        assert_eq!(summary.preview_symbols[0].notice, SCREEN_WIDTH_EDGE_NOTICE);
        fs::remove_dir_all(root).unwrap();
    }

    fn invisible_png() -> Vec<u8> {
        let mut pixels = vec![0; 512 * 512 * 4];
        for (i, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            pixel[..3].copy_from_slice(&[(i / 512) as u8, i as u8, 128]);
        }
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
        encoded
    }

    fn one_pixel_png() -> Vec<u8> {
        let mut pixels = vec![255; 512 * 512 * 4];
        pixels[..3].fill(0);
        let mut encoded = Vec::new();
        let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        encoded
    }

    fn varying_background_png(bands: bool) -> Vec<u8> {
        let mut pixels = vec![255; 512 * 512 * 4];
        for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = index % 512;
            pixel[..3].fill(if bands {
                255 - 10 * u8::from(x >= 256)
            } else {
                255 - (x / 8) as u8
            });
        }
        let mut encoded = Vec::new();
        let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        encoded
    }

    fn fixture_capture(request: BlenderRequest, replacement_png: Option<&[u8]>) -> CaptureOutcome {
        let root = std::env::temp_dir().join(format!("archy-captured-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/a10-render-real");
        let mut response: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture.join("response.json")).unwrap()).unwrap();
        let wire: serde_json::Value = serde_json::from_slice(&request.to_bytes().unwrap()).unwrap();
        response["request_id"] = wire["request_id"].clone();
        response["run_id"] = wire["run_id"].clone();
        response["request_digest"] = wire["input_digest"].clone();
        for artifact in response["artifacts"].as_array_mut().unwrap() {
            let name = artifact["path"].as_str().unwrap().to_owned();
            let bytes = if let Some(png) = replacement_png.filter(|_| name.ends_with(".png")) {
                png.to_vec()
            } else {
                fs::read(fixture.join(&name)).unwrap()
            };
            artifact["sha256"] = sha256_hex(&bytes).into();
            artifact["size_bytes"] = (bytes.len() as u64).into();
            fs::write(root.join(name), bytes).unwrap();
        }
        fs::write(
            root.join("response.json"),
            serde_json::to_vec(&response).unwrap(),
        )
        .unwrap();
        let captured = capture_blender(&File::open(&root).unwrap(), &request).unwrap();
        fs::remove_dir_all(root).unwrap();
        captured
    }

    #[test]
    fn canonical_v2_render_keeps_source_profile_and_verified_artifacts() {
        let root = std::env::temp_dir().join(format!("archy-render-v2-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let project = root.join("project.json");
        fs::write(
            &project,
            include_bytes!("../../../tests/fixtures/briefs/courtyard-v2.json"),
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
            source_run: Some(generated.run_id),
            selection: VariantSelection::All,
            mutation: MutationOptions::default(),
        })
        .unwrap();
        let rendered = render_with_worker(
            RenderInput {
                project: project.clone(),
                source_run: Some(evaluated.run_id.clone()),
                selection: VariantSelection::All,
                mutation: MutationOptions::default(),
            },
            |request, _| Ok(fixture_capture(request, None)),
        )
        .unwrap();
        let verified = crate::verify_run(&project, &rendered.run_id).unwrap();
        assert!(verified.canonical().is_some());
        assert_eq!(
            verified.manifest().parent_run_id.as_deref(),
            Some(evaluated.run_id.as_str())
        );
        assert!(matches!(
            rendered.record.variants[0].result,
            RenderResult::Verified { .. }
        ));
        assert!(root.join(&rendered.path).join("profile.json").is_file());
        let scene = format!(
            "renders/{}/scene.blend",
            rendered.record.variants[0].variant_id
        );
        let wrong_run = crate::desktop_open(crate::DesktopOpenInput {
            project: project.clone(),
            run: evaluated.run_id,
            artifact: scene.clone(),
            kind: crate::DesktopOpenKind::Blender,
        })
        .unwrap_err();
        assert_eq!(wrong_run.code, ErrorCode::ArtifactInvalid);
        let missing = crate::desktop_open(crate::DesktopOpenInput {
            project: project.clone(),
            run: rendered.run_id.clone(),
            artifact: format!("renders/variant-{}/view-top.png", "0".repeat(64)),
            kind: crate::DesktopOpenKind::Preview,
        })
        .unwrap_err();
        assert_eq!(missing.code, ErrorCode::ArtifactInvalid);
        let forged = crate::desktop_open(crate::DesktopOpenInput {
            project,
            run: rendered.run_id,
            artifact: "renders/../scene.blend".into(),
            kind: crate::DesktopOpenKind::Blender,
        })
        .unwrap_err();
        assert_eq!(forged.code, ErrorCode::PermissionDenied);
    }

    #[test]
    fn binding_rejection_at_render_publication_boundaries_preserves_retry() {
        // Render checks at entry and before commit; storage checks before the run
        // rename and after run publication are calls three and four.
        for reject_at in [3, 4] {
            let root =
                std::env::temp_dir().join(format!("archy-render-binding-{}", uuid::Uuid::new_v4()));
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
            let variant_id = crate::verify_run(&project, &evaluated.run_id)
                .unwrap()
                .variants()[0]
                .id
                .clone();
            let store = ProjectStore::open(&project).unwrap();
            let locked = store.lock().unwrap();
            let mutation = MutationOptions {
                idempotency_key: Some(uuid::Uuid::new_v4().to_string()),
                expected_project_revision: Some(evaluated.revision),
                ..MutationOptions::default()
            };
            let calls = Cell::new(0);
            let check = || {
                calls.set(calls.get() + 1);
                if calls.get() == reject_at {
                    return Err(ArchyError::new(
                        ErrorCode::RevisionConflict,
                        "project",
                        "binding changed",
                    ));
                }
                Ok(())
            };
            let failure = render_locked_with_worker(
                &locked,
                Some(evaluated.run_id.clone()),
                VariantSelection::One(variant_id.clone()),
                mutation.clone(),
                &check,
                |request, _| Ok(fixture_capture(request, None)),
            )
            .unwrap_err();
            assert_eq!(calls.get(), reject_at);
            assert_eq!(
                failure.code,
                if reject_at == 3 {
                    ErrorCode::RevisionConflict
                } else {
                    ErrorCode::InternalError
                }
            );
            assert_eq!(failure.retriable, reject_at == 4);
            assert_eq!(locked.revision().unwrap(), evaluated.revision);
            assert_eq!(locked.receipts().unwrap().len() as u64, evaluated.revision);
            let unaccepted_runs: Vec<_> = fs::read_dir(root.join("runs"))
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .filter(|name| name != &generated.run_id && name != &evaluated.run_id)
                .collect();
            assert_eq!(unaccepted_runs.len(), usize::from(reject_at == 4));

            if reject_at == 4 {
                let retry_checks = Cell::new(0);
                let retry_worker_calls = Cell::new(0);
                let rejected_retry = render_locked_with_worker(
                    &locked,
                    Some(evaluated.run_id.clone()),
                    VariantSelection::One(variant_id.clone()),
                    mutation.clone(),
                    &|| {
                        retry_checks.set(retry_checks.get() + 1);
                        if retry_checks.get() == 2 {
                            Err(ArchyError::new(
                                ErrorCode::RevisionConflict,
                                "project",
                                "binding changed during orphan recovery",
                            ))
                        } else {
                            Ok(())
                        }
                    },
                    |request, _| {
                        retry_worker_calls.set(retry_worker_calls.get() + 1);
                        Ok(fixture_capture(request, None))
                    },
                )
                .unwrap_err();
                assert!(rejected_retry.retriable);
                assert_eq!(retry_checks.get(), 2);
                assert_eq!(retry_worker_calls.get(), 0);
                assert_eq!(locked.revision().unwrap(), evaluated.revision);
            }

            let replay_worker_calls = Cell::new(0);
            let retried = render_locked_with_worker(
                &locked,
                Some(evaluated.run_id.clone()),
                VariantSelection::One(variant_id),
                mutation,
                &|| Ok(()),
                |request, _| {
                    replay_worker_calls.set(replay_worker_calls.get() + 1);
                    Ok(fixture_capture(request, None))
                },
            )
            .unwrap();
            assert_eq!(retried.revision, evaluated.revision + 1);
            assert_eq!(replay_worker_calls.get(), usize::from(reject_at == 3));
            if reject_at == 4 {
                assert_eq!(retried.run_id, unaccepted_runs[0]);
            }
            assert_eq!(locked.receipts().unwrap().len() as u64, retried.revision);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn invalid_captures_fail_publication_and_rehashed_replay() {
        let root =
            std::env::temp_dir().join(format!("archy-render-alpha-{}", uuid::Uuid::new_v4()));
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
            source_run: Some(generated.run_id),
            selection: VariantSelection::All,
            mutation: MutationOptions::default(),
        })
        .unwrap();
        let new_input = |revision| RenderInput {
            project: project.clone(),
            source_run: Some(evaluated.run_id.clone()),
            selection: VariantSelection::All,
            mutation: MutationOptions {
                idempotency_key: Some(uuid::Uuid::new_v4().to_string()),
                expected_project_revision: Some(revision),
                ..MutationOptions::default()
            },
        };
        let invisible = invisible_png();
        let rejected = render_with_worker(new_input(evaluated.revision), |request, _| {
            Ok(fixture_capture(request, Some(&invisible)))
        })
        .unwrap();
        assert!(
            matches!(rejected.record.variants[0].result, RenderResult::Failed { ref error, .. } if error.code == ErrorCode::ArtifactInvalid)
        );
        assert!(crate::verify_run(&project, &rejected.run_id).is_ok());
        let failed_status = crate::desktop_status(crate::DesktopStatusInput {
            project: project.clone(),
            run: Some(rejected.run_id.clone()),
            limit: 20,
            cursor: None,
        })
        .unwrap();
        assert_eq!(
            failed_status.selected_run.unwrap().summary.render_outcome,
            Some(crate::desktop_status::DesktopRenderOutcome::Failed)
        );

        let outlier = one_pixel_png();
        let rejected_outlier = render_with_worker(new_input(rejected.revision), |request, _| {
            Ok(fixture_capture(request, Some(&outlier)))
        })
        .unwrap();
        assert!(matches!(
            rejected_outlier.record.variants[0].result,
            RenderResult::Failed { ref error, .. } if error.code == ErrorCode::ArtifactInvalid && error.message.contains("blank PNG preview")
        ));
        assert!(crate::verify_run(&project, &rejected_outlier.run_id).is_ok());

        let mut revision = rejected_outlier.revision;
        for bands in [false, true] {
            let probe = varying_background_png(bands);
            let rejected = render_with_worker(new_input(revision), |request, _| {
                Ok(fixture_capture(request, Some(&probe)))
            })
            .unwrap();
            assert!(matches!(
                rejected.record.variants[0].result,
                RenderResult::Failed { ref error, .. } if error.code == ErrorCode::ArtifactInvalid && error.message.contains("background")
            ));
            assert!(crate::verify_run(&project, &rejected.run_id).is_ok());
            revision = rejected.revision;
        }

        let input = new_input(revision);
        let accepted = render_with_worker(input.clone(), |request, _| {
            Ok(fixture_capture(request, None))
        })
        .unwrap();
        assert!(matches!(
            accepted.record.variants[0].result,
            RenderResult::Verified { .. }
        ));
        assert_eq!(render(input.clone()).unwrap(), accepted);
        assert!(crate::verify_run(&project, &accepted.run_id).is_ok());
        assert!(matches!(
            &accepted.record.variants[0].result,
            RenderResult::Verified {
                preview_symbol: None,
                ..
            }
        ));
        let status = crate::desktop_status(crate::DesktopStatusInput {
            project: project.clone(),
            run: Some(accepted.run_id.clone()),
            limit: 20,
            cursor: None,
        })
        .unwrap();
        let selected = status.selected_run.unwrap();
        assert!(selected.summary.preview_symbols.is_empty());
        assert_eq!(
            selected.summary.render_outcome,
            Some(crate::desktop_status::DesktopRenderOutcome::Verified)
        );
        for view in ["view-top.png", "view-front.png", "view-iso.png"] {
            let path = format!(
                "renders/{}/{}",
                accepted.record.variants[0].variant_id, view
            );
            assert!(selected.artifacts.iter().any(|a| a.id == path
                && matches!(a.kind, crate::desktop_status::DesktopArtifactKind::Preview)));
            assert!(crate::desktop_open::is_openable_path(
                crate::desktop_open::DesktopOpenKind::Preview,
                &path
            ));
        }
        let scene = format!(
            "renders/{}/scene.blend",
            accepted.record.variants[0].variant_id
        );
        assert!(selected.artifacts.iter().any(|a| a.id == scene
            && matches!(a.kind, crate::desktop_status::DesktopArtifactKind::Blender)));
        assert!(crate::desktop_open::is_openable_path(
            crate::desktop_open::DesktopOpenKind::Blender,
            &scene
        ));
        for forged in [
            "renders/../scene.blend",
            "renders/variant-bad/scene.blend",
            "renders/variant-0000000000000000000000000000000000000000000000000000000000000000/evil.blend",
        ] {
            assert!(!crate::desktop_open::is_openable_path(
                crate::desktop_open::DesktopOpenKind::Blender,
                forged
            ));
        }

        let run_dir = root.join(&accepted.path);
        let record_path = run_dir.join("render.json");
        let manifest_path = run_dir.join("manifest.json");
        let revision_path = root.join(format!(".archy/revisions/{:020}.json", accepted.revision));
        let original_record = fs::read(&record_path).unwrap();
        let original_manifest = fs::read(&manifest_path).unwrap();
        let original_revision = fs::read(&revision_path).unwrap();
        let mut forged = accepted.record.clone();
        if let RenderResult::Verified { preview_symbol, .. } = &mut forged.variants[0].result {
            *preview_symbol = Some(RenderPreviewSymbol {
                views: vec!["top".into()],
                notice: SCREEN_WIDTH_EDGE_NOTICE.into(),
            });
        }
        let forged_bytes = serialize(&forged).unwrap();
        fs::write(&record_path, &forged_bytes).unwrap();
        let mut manifest: serde_json::Value = serde_json::from_slice(&original_manifest).unwrap();
        let record_claim = manifest["artifacts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|a| a["path"] == "render.json")
            .unwrap();
        record_claim["sha256"] = sha256_hex(&forged_bytes).into();
        record_claim["byte_len"] = (forged_bytes.len() as u64).into();
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        fs::write(&manifest_path, &manifest_bytes).unwrap();
        let mut revision: serde_json::Value = serde_json::from_slice(&original_revision).unwrap();
        revision["receipt"]["manifest_sha256"] = sha256_hex(&manifest_bytes).into();
        fs::write(&revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();
        assert_eq!(
            crate::verify_run(&project, &accepted.run_id)
                .unwrap_err()
                .code,
            ErrorCode::ArtifactInvalid
        );
        assert_eq!(
            render(input.clone()).unwrap_err().code,
            ErrorCode::ArtifactInvalid
        );
        fs::write(&record_path, original_record).unwrap();
        fs::write(&manifest_path, original_manifest).unwrap();
        fs::write(&revision_path, original_revision).unwrap();
        let prefix = format!("renders/{}/", accepted.record.variants[0].variant_id);
        let png_path = format!("{prefix}view-top.png");
        let response_path = format!("{prefix}response.json");
        for (forged, expected) in [
            (outlier, "blank PNG preview"),
            (varying_background_png(false), "background"),
        ] {
            fs::write(run_dir.join(&png_path), &forged).unwrap();
            let mut response: serde_json::Value =
                serde_json::from_slice(&fs::read(run_dir.join(&response_path)).unwrap()).unwrap();
            let claim = response["artifacts"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|a| a["path"] == "view-top.png")
                .unwrap();
            claim["sha256"] = sha256_hex(&forged).into();
            claim["size_bytes"] = (forged.len() as u64).into();
            let response_bytes = serde_json::to_vec(&response).unwrap();
            fs::write(run_dir.join(&response_path), &response_bytes).unwrap();
            let mut record = accepted.record.clone();
            if let RenderResult::Verified { artifacts, .. } = &mut record.variants[0].result {
                for item in artifacts {
                    if item.path == png_path {
                        item.sha256 = sha256_hex(&forged);
                        item.byte_len = forged.len() as u64;
                    }
                    if item.path == response_path {
                        item.sha256 = sha256_hex(&response_bytes);
                        item.byte_len = response_bytes.len() as u64;
                    }
                }
            }
            let record_bytes = serialize(&record).unwrap();
            fs::write(run_dir.join("render.json"), &record_bytes).unwrap();
            let manifest_path = run_dir.join("manifest.json");
            let mut manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
            for item in manifest["artifacts"].as_array_mut().unwrap() {
                let path = item["path"].as_str().unwrap();
                let bytes = if path == png_path {
                    Some(&forged)
                } else if path == response_path {
                    Some(&response_bytes)
                } else if path == "render.json" {
                    Some(&record_bytes)
                } else {
                    None
                };
                if let Some(bytes) = bytes {
                    item["sha256"] = sha256_hex(bytes).into();
                    item["byte_len"] = (bytes.len() as u64).into();
                }
            }
            let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
            fs::write(&manifest_path, &manifest_bytes).unwrap();
            let revision_path =
                root.join(format!(".archy/revisions/{:020}.json", accepted.revision));
            let mut revision: serde_json::Value =
                serde_json::from_slice(&fs::read(&revision_path).unwrap()).unwrap();
            revision["receipt"]["manifest_sha256"] = sha256_hex(&manifest_bytes).into();
            fs::write(revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();
            let replay_error = crate::verify_run(&project, &accepted.run_id).unwrap_err();
            assert_eq!(replay_error.code, ErrorCode::ArtifactInvalid);
            assert!(replay_error.message.contains(expected), "{replay_error:?}");
            let render_error = render(input.clone()).unwrap_err();
            assert_eq!(render_error.code, ErrorCode::ArtifactInvalid);
            assert!(render_error.message.contains(expected), "{render_error:?}");
        }
        fs::remove_dir_all(root).unwrap();
    }
}
