use super::{ClaimedArtifact, artifacts, bad, decode, finite, near, same};
use crate::request::IfcRequest;
use archy_core::ArchyError;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum IfcResponse {
    Ok(Box<IfcSuccess>),
    Error(Box<IfcFailure>),
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OkStatus {
    Ok,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ErrorStatus {
    Error,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IfcSuccess {
    #[serde(rename = "status")]
    _status: OkStatus,
    schema_version: String,
    request_id: String,
    project_id: String,
    variant_id: String,
    run_id: String,
    source_digest: String,
    worker_version: String,
    backend_version: String,
    python_version: String,
    warnings: Vec<String>,
    pub validation: IfcValidation,
    pub artifacts: Vec<ClaimedArtifact>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IfcFailure {
    #[serde(rename = "status")]
    _status: ErrorStatus,
    schema_version: String,
    request_id: String,
    project_id: String,
    variant_id: String,
    run_id: String,
    source_digest: String,
    pub error: ArchyError,
    pub artifacts: Vec<ClaimedArtifact>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IfcValidation {
    status: String,
    schema: String,
    storeys: u32,
    footprint_area_m2: f64,
    expected_footprint_area_m2: f64,
    approximate_gfa_m2: f64,
    height_m: f64,
    tessellation: Vec<Tessellation>,
    draft: bool,
    engine: String,
    fresh_process_reopen: bool,
    reopen_process: ReopenProcess,
    express_validation: ExpressValidation,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReopenProcess {
    returncode: i32,
    stderr: String,
    // A successful response must contain the worker's explicit null failure marker.
    #[serde(rename = "failure")]
    _failure: (),
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Tessellation {
    bounds_m: [f64; 6],
    volume_m3: f64,
    triangles: u32,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExpressValidation {
    exit_code: i32,
    findings: Vec<String>,
    stderr: String,
    #[serde(rename = "failure")]
    _failure: (),
}

pub fn decode_ifc_response(bytes: &[u8], request: &IfcRequest) -> Result<IfcResponse, ArchyError> {
    let response: IfcResponse = decode(bytes)?;
    let wire = request.wire();
    match &response {
        IfcResponse::Error(failure) => {
            for (got, expected, field) in [
                (&failure.schema_version, "ifc-worker-v1", "schema_version"),
                (&failure.request_id, wire.request_id.as_str(), "request_id"),
                (&failure.project_id, wire.project_id.as_str(), "project_id"),
                (&failure.variant_id, wire.variant_id.as_str(), "variant_id"),
                (&failure.run_id, wire.run_id.as_str(), "run_id"),
                (
                    &failure.source_digest,
                    wire.source_digest.as_str(),
                    "source_digest",
                ),
            ] {
                same(got, expected, field)?;
            }
            if !failure.artifacts.is_empty() {
                return Err(bad("artifacts", "error lists partial artifacts"));
            }
        }
        IfcResponse::Ok(success) => {
            for (got, expected, field) in [
                (&success.schema_version, "ifc-worker-v1", "schema_version"),
                (&success.request_id, wire.request_id.as_str(), "request_id"),
                (&success.project_id, wire.project_id.as_str(), "project_id"),
                (&success.variant_id, wire.variant_id.as_str(), "variant_id"),
                (&success.run_id, wire.run_id.as_str(), "run_id"),
                (
                    &success.source_digest,
                    wire.source_digest.as_str(),
                    "source_digest",
                ),
                (&success.worker_version, "ifc-worker-v1", "worker_version"),
                (&success.backend_version, "0.9.0.post1", "backend_version"),
                (&success.python_version, "3.12.16", "python_version"),
            ] {
                same(got, expected, field)?;
            }
            if success.warnings.len() > 16 {
                return Err(bad("warnings", "too many worker warnings"));
            }
            artifacts(
                &success.artifacts,
                &[
                    ("model.ifc", "application/x-step"),
                    ("validation.json", "application/json"),
                ],
                "ifc-worker-v1",
            )?;
            validate(&success.validation, request)?;
        }
    }
    Ok(response)
}

fn validate(v: &IfcValidation, request: &IfcRequest) -> Result<(), ArchyError> {
    same(&v.status, "ok", "validation.status")?;
    same(&v.schema, "IFC4", "validation.schema")?;
    same(
        &v.engine,
        "IfcOpenShell reopen+tessellation; not independent A13",
        "validation.engine",
    )?;
    if !v.draft
        || !v.fresh_process_reopen
        || v.storeys != request.wire().variant.storeys
        || v.tessellation.len() != v.storeys as usize
    {
        return Err(bad(
            "validation",
            "missing draft/reopen or storey count mismatch",
        ));
    }
    if v.reopen_process.returncode != 0
        || v.reopen_process.stderr.len() > 4096
        || v.express_validation.exit_code != 0
        || !v.express_validation.findings.is_empty()
        || v.express_validation.stderr.len() > 4096
    {
        return Err(bad("validation", "reopen or EXPRESS validation not clean"));
    }
    finite(
        [
            v.footprint_area_m2,
            v.expected_footprint_area_m2,
            v.approximate_gfa_m2,
            v.height_m,
        ],
        "validation",
    )?;
    let area = request
        .evaluation()
        .metrics
        .iter()
        .find(|m| m.name == "footprint_area")
        .ok_or_else(|| bad("validation", "missing canonical area metric"))?
        .value;
    let height = request.wire().variant.floor_to_floor_m * f64::from(v.storeys);
    let budget = &request.evaluation().geometry_parity_budget;
    near(
        v.footprint_area_m2,
        area,
        budget.footprint_area_m2,
        "validation.footprint_area_m2",
    )?;
    near(
        v.expected_footprint_area_m2,
        area,
        budget.footprint_area_m2,
        "validation.expected_footprint_area_m2",
    )?;
    near(
        v.approximate_gfa_m2,
        area * f64::from(v.storeys),
        budget.gross_floor_area_m2,
        "validation.approximate_gfa_m2",
    )?;
    near(v.height_m, height, 1e-6, "validation.height_m")?;
    let ring = &request.wire().variant.footprint_xy_m;
    let xy = [
        ring.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
        ring.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
        ring.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max),
        ring.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max),
    ];
    for (i, solid) in v.tessellation.iter().enumerate() {
        finite(
            solid.bounds_m.into_iter().chain([solid.volume_m3]),
            "validation.tessellation",
        )?;
        if solid.triangles == 0 || solid.volume_m3 <= 0. {
            return Err(bad("validation.tessellation", "empty mass"));
        }
        let z0 = i as f64 * request.wire().variant.floor_to_floor_m;
        let expected = [
            xy[0],
            xy[1],
            z0,
            xy[2],
            xy[3],
            z0 + request.wire().variant.floor_to_floor_m,
        ];
        for (got, want) in solid.bounds_m.iter().zip(expected) {
            near(*got, want, 1e-6, "validation.tessellation.bounds_m")?;
        }
        let volume_budget =
            budget.footprint_area_m2 * request.wire().variant.floor_to_floor_m + area * 1e-6;
        near(
            solid.volume_m3,
            area * request.wire().variant.floor_to_floor_m,
            volume_budget,
            "validation.tessellation.volume_m3",
        )?;
    }
    Ok(())
}
