use archy_core::{
    ArchyError, ErrorCode, EvaluationReport, NormalizedSite, NormalizedSpec, ShapeParameters,
    Variant, site_bounds,
};
use archy_eval::evaluate_variant;
use schemars::JsonSchema;
use serde::Serialize;

const MAX_BYTES: usize = 1024 * 1024;

fn invalid(field: &str, message: &str) -> ArchyError {
    ArchyError::schema(field, message)
}

fn capacity(field: &str, message: &str) -> ArchyError {
    ArchyError::new(ErrorCode::BackendUnavailable, field, message)
}

fn identifier(value: &str, field: &str) -> Result<(), ArchyError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 128
        || !bytes[0].is_ascii_alphanumeric()
        || !bytes
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    {
        return Err(invalid(field, "expected a bounded ASCII identifier"));
    }
    Ok(())
}

fn digest(value: &str, field: &str) -> Result<(), ArchyError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(invalid(field, "expected lowercase SHA-256 hex"));
    }
    Ok(())
}

fn logical_path(value: &str, field: &str) -> Result<(), ArchyError> {
    if value.is_empty()
        || value.len() > 512
        || value
            .split('/')
            .any(|p| p == "." || p == ".." || identifier(p, field).is_err())
    {
        return Err(invalid(field, "expected a project-relative logical path"));
    }
    Ok(())
}

fn common(
    spec: &NormalizedSpec,
    variant: &Variant,
    request_id: &str,
    run_id: &str,
    source_digest: &str,
    output_dir: &str,
) -> Result<EvaluationReport, ArchyError> {
    variant.validate()?;
    site_bounds(spec)?;
    let evaluation = evaluate_variant(spec, variant)?;
    identifier(request_id, "request_id")?;
    identifier(run_id, "run_id")?;
    identifier(&spec.project_id, "project_id")?;
    identifier(
        &variant.generation_algorithm,
        "variant.generation_algorithm",
    )?;
    digest(source_digest, "source_digest")?;
    logical_path(output_dir, "output_dir")?;
    if variant.storeys == 0 || variant.storeys > 100 {
        return Err(capacity(
            "variant.storeys",
            "worker supports 1..100 storeys",
        ));
    }
    let height = variant.floor_to_floor_m * f64::from(variant.storeys);
    if !height.is_finite()
        || height > 10000.0
        || variant.floor_to_floor_m <= 1e-6
        || variant.floor_to_floor_m > 10000.0
    {
        return Err(capacity(
            "variant.floor_to_floor_m",
            "worker floor height must exceed 1e-6 m and total height must fit 10000 m",
        ));
    }
    let params = &variant.parameters;
    let dimensions = match params {
        ShapeParameters::Rectangle {
            width_m, depth_m, ..
        } => vec![*width_m, *depth_m],
        ShapeParameters::LShape {
            width_m,
            depth_m,
            notch_width_m,
            notch_depth_m,
            ..
        } => vec![
            *width_m,
            *depth_m,
            *notch_width_m,
            *notch_depth_m,
            width_m - notch_width_m,
            depth_m - notch_depth_m,
        ],
    };
    if dimensions
        .iter()
        .any(|v| !v.is_finite() || *v <= 1e-6 || *v > 10000.0)
    {
        return Err(capacity(
            "variant.parameters",
            "worker dimensions must exceed 1e-6 m and fit 10000 m",
        ));
    }
    if !matches!(variant.footprint_xy_m.len(), 5 | 7)
        || variant
            .footprint_xy_m
            .iter()
            .flatten()
            .chain(spec.site.polygon_xy_m.iter().flatten())
            .any(|v| !v.is_finite() || v.abs() > 10000.0)
        || !spec.site.north_angle_deg.is_finite()
        || spec.site.north_angle_deg.abs() > 10000.0
    {
        return Err(capacity("site", "worker coordinate capacity is ±10000 m"));
    }
    Ok(evaluation)
}

fn bytes<T: Serialize>(wire: &T) -> Result<Vec<u8>, ArchyError> {
    let encoded = serde_json::to_vec(wire)
        .map_err(|e| invalid("$", &format!("request serialization: {e}")))?;
    if encoded.len() > MAX_BYTES {
        return Err(capacity("$", "worker request exceeds 1 MiB"));
    }
    Ok(encoded)
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct BlenderWire {
    pub(crate) schema_version: &'static str,
    pub(crate) operation: &'static str,
    pub(crate) request_id: String,
    pub(crate) project_id: String,
    pub(crate) variant_id: String,
    pub(crate) run_id: String,
    pub(crate) params: Variant,
    pub(crate) site: NormalizedSite,
    pub(crate) input_digest: String,
    pub(crate) preset_id: &'static str,
    pub(crate) output_dir: String,
}

#[derive(Debug, Clone)]
pub struct BlenderRequest {
    wire: BlenderWire,
    evaluation: EvaluationReport,
}

impl BlenderRequest {
    pub fn new(
        spec: &NormalizedSpec,
        variant: &Variant,
        request_id: &str,
        run_id: &str,
        input_digest: &str,
        output_dir: &str,
    ) -> Result<Self, ArchyError> {
        let evaluation = common(spec, variant, request_id, run_id, input_digest, output_dir)?;
        let request = Self {
            wire: BlenderWire {
                schema_version: "blender-worker-v1",
                operation: "build_massing",
                request_id: request_id.into(),
                project_id: spec.project_id.clone(),
                variant_id: variant.id.clone(),
                run_id: run_id.into(),
                params: variant.clone(),
                site: spec.site.clone(),
                input_digest: input_digest.into(),
                preset_id: "smoke-cpu-v1",
                output_dir: output_dir.into(),
            },
            evaluation,
        };
        request.to_bytes()?;
        Ok(request)
    }
    pub fn evaluation(&self) -> &EvaluationReport {
        &self.evaluation
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, ArchyError> {
        bytes(&self.wire)
    }
    pub(crate) fn wire(&self) -> &BlenderWire {
        &self.wire
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SourceArtifact {
    path: String,
    sha256: String,
}

impl SourceArtifact {
    pub fn new(path: &str, sha256: &str) -> Result<Self, ArchyError> {
        logical_path(path, "source_artifacts.path")?;
        digest(sha256, "source_artifacts.sha256")?;
        Ok(Self {
            path: path.into(),
            sha256: sha256.into(),
        })
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct IfcWire {
    pub(crate) schema_version: &'static str,
    pub(crate) operation: &'static str,
    pub(crate) request_id: String,
    pub(crate) project_id: String,
    pub(crate) variant_id: String,
    pub(crate) run_id: String,
    pub(crate) variant: Variant,
    pub(crate) site: NormalizedSite,
    pub(crate) source_digest: String,
    pub(crate) source_artifacts: Vec<SourceArtifact>,
    pub(crate) output_dir: String,
}

#[derive(Debug, Clone)]
pub struct IfcRequest {
    wire: IfcWire,
    evaluation: EvaluationReport,
}

impl IfcRequest {
    pub fn new(
        spec: &NormalizedSpec,
        variant: &Variant,
        request_id: &str,
        run_id: &str,
        source_digest: &str,
        output_dir: &str,
        source_artifacts: Vec<SourceArtifact>,
    ) -> Result<Self, ArchyError> {
        let evaluation = common(spec, variant, request_id, run_id, source_digest, output_dir)?;
        if source_artifacts.len() > 16 {
            return Err(capacity(
                "source_artifacts",
                "IFC worker supports at most 16 source references",
            ));
        }
        let request = Self {
            wire: IfcWire {
                schema_version: "ifc-worker-v1",
                operation: "export_schematic_ifc",
                request_id: request_id.into(),
                project_id: spec.project_id.clone(),
                variant_id: variant.id.clone(),
                run_id: run_id.into(),
                variant: variant.clone(),
                site: spec.site.clone(),
                source_digest: source_digest.into(),
                source_artifacts,
                output_dir: output_dir.into(),
            },
            evaluation,
        };
        request.to_bytes()?;
        Ok(request)
    }
    pub fn evaluation(&self) -> &EvaluationReport {
        &self.evaluation
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, ArchyError> {
        bytes(&self.wire)
    }
    pub(crate) fn wire(&self) -> &IfcWire {
        &self.wire
    }
}
