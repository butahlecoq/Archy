use crate::{ArchyError, ErrorCode, project::*, units::Units};
use std::collections::HashSet;

pub const MAX_INPUT_BYTES: usize = 1_048_576;
pub const TOLERANCE_M: f64 = 1e-6;
pub const TOLERANCE_M2: f64 = 1e-6;

struct TrackedProject(Project);
impl<'de> serde::Deserialize<'de> for TrackedProject {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde_path_to_error::deserialize(deserializer)
            .map(Self)
            .map_err(|e| {
                let mut path = e.path().to_string();
                let message = e.inner().to_string();
                for prefix in ["missing field `", "unknown field `", "duplicate field `"] {
                    if let Some(key) = message
                        .split(prefix)
                        .nth(1)
                        .and_then(|s| s.split('`').next())
                    {
                        if path == "." || path.is_empty() {
                            path = key.into();
                        } else if !path.ends_with(&format!(".{key}")) && path != key {
                            path.push_str(&format!(".{key}"));
                        }
                        break;
                    }
                }
                serde::de::Error::custom(format!("ARCHY_PATH|{path}| {message}"))
            })
    }
}
#[derive(Debug, Clone, Copy)]
pub enum InputFormat {
    Json,
    Yaml,
}

pub fn parse_project(text: &str, format: InputFormat) -> Result<NormalizedSpec, ArchyError> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(ArchyError::schema("$", "Project exceeds 1 MiB input limit"));
    }
    let project: TrackedProject = match format {
        InputFormat::Json => serde_json::from_str(text).map_err(|e| parse_error(e.to_string()))?,
        InputFormat::Yaml => {
            let options = serde_saphyr::options! {
                strict_booleans: true,
                no_schema: true,
                reject_unsupported_tags: true,
                merge_keys: serde_saphyr::MergeKeyPolicy::Error,
                duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
                budget: serde_saphyr::budget! {
                    max_anchors: 0, max_aliases: 0, max_documents: 1,
                    max_depth: 32, flow_nesting_limit: 32, max_nodes: 20_000,
                    max_events: 40_000, max_total_scalar_bytes: MAX_INPUT_BYTES,
                    max_inclusion_depth: 0,
                },
            };
            serde_saphyr::from_str_with_options(text, options)
                .map_err(|e| parse_error(e.to_string()))?
        }
    };
    normalize(project.0)
}
fn parse_error(message: String) -> ArchyError {
    let field = message
        .split("ARCHY_PATH|")
        .nth(1)
        .and_then(|s| s.split('|').next())
        .filter(|s| !s.is_empty() && *s != ".")
        .unwrap_or("$")
        .to_owned();
    ArchyError::schema(field, message)
}
fn positive(v: f64, field: &str) -> Result<f64, ArchyError> {
    if !v.is_finite() || v <= 0.0 {
        Err(ArchyError::schema(
            field,
            "Expected a finite positive dimension",
        ))
    } else {
        Ok(v)
    }
}
fn dimension(
    si: Option<f64>,
    generic: Option<f64>,
    units: Units,
    power: i32,
    field: &str,
) -> Result<f64, ArchyError> {
    let v = match (si, generic) {
        (Some(_), Some(_)) => {
            return Err(ArchyError::schema(
                field,
                "Supply either SI-suffixed or generic value, not both",
            ));
        }
        (Some(v), None) => v,
        (None, Some(v)) => v * units.meters().powi(power),
        (None, None) => return Err(ArchyError::schema(field, "Required dimension is missing")),
    };
    positive(v, field)
}
pub(crate) fn identifier(value: &str, field: &str) -> Result<(), ArchyError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(ArchyError::schema(
            field,
            "Use 1-128 ASCII letters, digits, hyphens or underscores",
        ));
    }
    Ok(())
}
pub(crate) fn constraint_identifier(value: &str, field: &str) -> Result<(), ArchyError> {
    identifier(value, field)?;
    if matches!(value, "site_containment" | "target_gfa") {
        return Err(ArchyError::schema(
            field,
            "Constraint identifier is reserved for an internal check",
        ));
    }
    Ok(())
}
fn rectangle(points: Vec<[f64; 2]>) -> Result<[[f64; 2]; 5], ArchyError> {
    let bad = || {
        ArchyError::new(
            ErrorCode::InvalidGeometry,
            "site.polygon_xy",
            "Expected a closed simple axis-aligned rectangle with four distinct corners",
        )
    };
    if points.len() != 5 || points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(bad());
    }
    let close = |a: f64, b: f64| (a - b).abs() <= TOLERANCE_M;
    if !close(points[0][0], points[4][0]) || !close(points[0][1], points[4][1]) {
        return Err(bad());
    }
    let minx = points[..4]
        .iter()
        .map(|p| p[0])
        .fold(f64::INFINITY, f64::min);
    let miny = points[..4]
        .iter()
        .map(|p| p[1])
        .fold(f64::INFINITY, f64::min);
    let maxx = points[..4]
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let maxy = points[..4]
        .iter()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    if !((maxx - minx).is_finite() && (maxy - miny).is_finite())
        || maxx - minx <= TOLERANCE_M
        || maxy - miny <= TOLERANCE_M
    {
        return Err(bad());
    }
    let mut corners = HashSet::new();
    for i in 0..4 {
        let p = points[i];
        let q = points[(i + 1) % 4];
        if close(p[0], q[0]) == close(p[1], q[1]) {
            return Err(bad());
        }
        let x = if close(p[0], minx) {
            0
        } else if close(p[0], maxx) {
            1
        } else {
            return Err(bad());
        };
        let y = if close(p[1], miny) {
            0
        } else if close(p[1], maxy) {
            1
        } else {
            return Err(bad());
        };
        if !corners.insert((x, y)) {
            return Err(bad());
        }
    }
    Ok([
        [minx, miny],
        [maxx, miny],
        [maxx, maxy],
        [minx, maxy],
        [minx, miny],
    ])
}

pub fn normalize(p: Project) -> Result<NormalizedSpec, ArchyError> {
    if p.schema_version != "0.1.0" {
        return Err(ArchyError::schema(
            "schema_version",
            "Supported version is 0.1.0; migrations are not implemented",
        ));
    }
    identifier(&p.project_id, "project_id")?;
    let s = p.site;
    let has_dimensions = [s.width, s.depth, s.width_m, s.depth_m]
        .iter()
        .any(Option::is_some);
    let polygon = match (s.polygon_xy_m, s.polygon_xy) {
        (Some(_), Some(_)) => {
            return Err(ArchyError::schema(
                "site",
                "Supply only one polygon representation",
            ));
        }
        (Some(points), None) => Some(points),
        (None, Some(points)) => Some(
            points
                .into_iter()
                .map(|[x, y]| [x * p.units.meters(), y * p.units.meters()])
                .collect(),
        ),
        (None, None) => None,
    };
    let polygon_xy_m = if let Some(points) = polygon {
        if has_dimensions {
            return Err(ArchyError::schema(
                "site",
                "Supply polygon or width/depth, not both",
            ));
        }
        rectangle(points)?
    } else {
        let w = dimension(s.width_m, s.width, p.units, 1, "site.width")?;
        let d = dimension(s.depth_m, s.depth, p.units, 1, "site.depth")?;
        rectangle(vec![[0., 0.], [w, 0.], [w, d], [0., d], [0., 0.]])?
    };
    if !s.north_deg.is_finite() {
        return Err(ArchyError::schema(
            "site.north_deg",
            "Expected a finite angle",
        ));
    }
    if p.brief.storeys == 0 {
        return Err(ArchyError::schema(
            "brief.storeys",
            "Storeys must be a positive integer",
        ));
    }
    let floor = dimension(
        p.brief.floor_to_floor_m,
        p.brief.floor_to_floor,
        p.units,
        1,
        "brief.floor_to_floor",
    )?;
    positive(floor * f64::from(p.brief.storeys), "brief.floor_to_floor")?;
    let target = dimension(
        p.brief.target_gfa_m2,
        p.brief.target_gfa,
        p.units,
        2,
        "brief.target_gfa",
    )?;
    let width = polygon_xy_m[1][0] - polygon_xy_m[0][0];
    let depth = polygon_xy_m[2][1] - polygon_xy_m[1][1];
    positive(width * depth, "site")?;
    let mut constraints = Vec::new();
    let mut ids = HashSet::new();
    let mut setback = 0.0_f64;
    for (i, c) in p.constraints.into_iter().enumerate() {
        let path = format!("constraints[{i}]");
        constraint_identifier(&c.id, &format!("{path}.id"))?;
        if !ids.insert(c.id.clone()) {
            return Err(ArchyError::schema(
                format!("{path}.id"),
                "Constraint IDs must be unique",
            ));
        }
        if c.source.trim().is_empty() {
            return Err(ArchyError::schema(
                format!("{path}.source"),
                "Declare a source, such as user_brief or assumption",
            ));
        }
        if c.authority.as_ref().is_some_and(|a| a.trim().is_empty()) {
            return Err(ArchyError::schema(
                format!("{path}.authority"),
                "Declare authority or omit it for assumption",
            ));
        }
        let units = c.unit.unwrap_or(p.units);
        let value = dimension(c.value_m, c.value, units, 1, &format!("{path}.value"))?;
        if c.kind == ConstraintKind::MinEdgeSetback {
            setback = setback.max(value);
        }
        constraints.push(NormalizedConstraint {
            id: c.id,
            kind: c.kind,
            value_m: value,
            source: c.source,
            authority: c.authority.unwrap_or_else(|| "assumption".into()),
        });
    }
    if setback * 2.0 >= width || setback * 2.0 >= depth {
        return Err(ArchyError::schema(
            "constraints",
            "Hard setbacks leave no positive rectangular buildable area",
        ));
    }
    let capacity = (width - 2.0 * setback) * (depth - 2.0 * setback) * f64::from(p.brief.storeys);
    positive(capacity, "brief.storeys")?;
    if p.brief.target_gfa_hard && target > capacity + TOLERANCE_M2 {
        return Err(ArchyError::schema(
            "brief.target_gfa",
            "Hard target exceeds rectangular site/storey capacity after setbacks",
        ));
    }
    if p.generation.count == 0 || p.generation.count > 1000 {
        return Err(ArchyError::schema(
            "generation.count",
            "Expected count between 1 and 1000",
        ));
    }
    if p.generation.shapes.is_empty() {
        return Err(ArchyError::schema(
            "generation.shapes",
            "Declare at least one supported shape",
        ));
    }
    let mut shapes = HashSet::new();
    for shape in &p.generation.shapes {
        if !shapes.insert(*shape as u8) {
            return Err(ArchyError::schema("generation.shapes", "Duplicate shape"));
        }
    }
    Ok(NormalizedSpec {
        schema_version: p.schema_version,
        project_id: p.project_id,
        units: Units::M,
        site: NormalizedSite {
            polygon_xy_m,
            north_angle_deg: s.north_deg.rem_euclid(360.0),
        },
        brief: NormalizedBrief {
            storeys: p.brief.storeys,
            floor_to_floor_m: floor,
            target_gfa_m2: target,
            target_gfa_hard: p.brief.target_gfa_hard,
        },
        constraints,
        preferences: p.preferences,
        generation: p.generation,
        normalization_version: "si-rectangle-v1".into(),
        tolerance_m: TOLERANCE_M,
    })
}
