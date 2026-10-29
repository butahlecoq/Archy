//! Measurements of declared schematic geometry.
use archy_core::validation::{TOLERANCE_M, TOLERANCE_M2};
use archy_core::*;
use geo::{Area, BoundingRect};

pub const EVALUATION_VERSION: &str = "planar-massing-v1";
pub const PROFESSIONAL_LIMITS: &str = "Measures declared geometric bounds only; not legal compliance or professional certification. Sources and authority are unverified declarations. GFA approximates repeated footprint area; daylight, structure, access, cost and local regulations are uncomputed.";
pub const GEOMETRY_PARITY_POLICY_VERSION: &str = "polygon-coordinate-parity-v1";
pub const RIGID_TRANSFORM_POLICY_VERSION: &str = "rigid-xy-ordered-v1";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidTransform2D {
    pub rotation_rad: f64,
    pub translation_xy_m: [f64; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryParityComparison {
    pub passed: bool,
    pub max_coordinate_delta_m: f64,
    pub footprint_area_delta_m2: f64,
    pub gross_floor_area_delta_m2: f64,
    pub transform_area_delta_m2: f64,
    pub budget: GeometryParityBudget,
}

pub fn geometry_parity_budget(variant: &Variant) -> Result<GeometryParityBudget, ArchyError> {
    variant.validate()?;
    budget_for_footprint(&variant.footprint_xy_m, variant.storeys)
}

fn budget_for_footprint(
    footprint_xy_m: &[[f64; 2]],
    storeys: u32,
) -> Result<GeometryParityBudget, ArchyError> {
    let perimeter_m: f64 = footprint_xy_m
        .windows(2)
        .map(|edge| (edge[1][0] - edge[0][0]).abs() + (edge[1][1] - edge[0][1]).abs())
        .sum();
    let coordinate_length_m = TOLERANCE_M;
    let vertex_count = (footprint_xy_m.len() - 1) as f64;
    let footprint_area_m2 = perimeter_m * coordinate_length_m
        + vertex_count * coordinate_length_m * coordinate_length_m;
    let gross_floor_area_m2 = footprint_area_m2 * f64::from(storeys);
    if !perimeter_m.is_finite() || !gross_floor_area_m2.is_finite() {
        return Err(ArchyError::new(
            ErrorCode::InvalidGeometry,
            "geometry_parity_budget",
            "Numeric budget overflow",
        ));
    }
    Ok(GeometryParityBudget {
        policy_version: GEOMETRY_PARITY_POLICY_VERSION.into(),
        coordinate_length_m,
        footprint_area_m2,
        gross_floor_area_m2,
        rigid_transform_policy_version: RIGID_TRANSFORM_POLICY_VERSION.into(),
    })
}

pub fn compare_rigid_geometry(
    variant: &Variant,
    observed: &[[f64; 2]],
    transform: RigidTransform2D,
) -> Result<GeometryParityComparison, ArchyError> {
    variant.validate()?;
    if !transform.rotation_rad.is_finite()
        || transform
            .translation_xy_m
            .iter()
            .any(|coordinate| !coordinate.is_finite())
    {
        return Err(ArchyError::new(
            ErrorCode::InvalidGeometry,
            "transform",
            "Rigid transform must be finite",
        ));
    }
    if observed.len() != variant.footprint_xy_m.len() {
        return Err(ArchyError::new(
            ErrorCode::InvalidGeometry,
            "observed",
            "Vertex count differs from canonical footprint",
        ));
    }
    let (sine, cosine) = transform.rotation_rad.sin_cos();
    let expected: Vec<[f64; 2]> = variant
        .footprint_xy_m
        .iter()
        .map(|point| {
            [
                cosine * point[0] - sine * point[1] + transform.translation_xy_m[0],
                sine * point[0] + cosine * point[1] + transform.translation_xy_m[1],
            ]
        })
        .collect();
    let expected_polygon = validated_polygon(&expected)?;
    let observed_polygon = validated_polygon(observed)?;
    let budget = budget_for_footprint(&expected, variant.storeys)?;
    let max_coordinate_delta_m = observed
        .iter()
        .zip(&expected)
        .flat_map(|(actual, target)| [(actual[0] - target[0]).abs(), (actual[1] - target[1]).abs()])
        .fold(0.0_f64, f64::max);
    let footprint_area_delta_m2 =
        (observed_polygon.unsigned_area() - expected_polygon.unsigned_area()).abs();
    let gross_floor_area_delta_m2 = footprint_area_delta_m2 * f64::from(variant.storeys);
    let canonical_area_m2 = validated_polygon(&variant.footprint_xy_m)?.unsigned_area();
    let transform_area_delta_m2 = (expected_polygon.unsigned_area() - canonical_area_m2).abs();
    if [
        max_coordinate_delta_m,
        footprint_area_delta_m2,
        gross_floor_area_delta_m2,
        transform_area_delta_m2,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(ArchyError::new(
            ErrorCode::InvalidGeometry,
            "geometry_parity",
            "Comparison arithmetic overflow",
        ));
    }
    let passed = max_coordinate_delta_m <= budget.coordinate_length_m
        && footprint_area_delta_m2 <= budget.footprint_area_m2
        && gross_floor_area_delta_m2 <= budget.gross_floor_area_m2
        && transform_area_delta_m2 <= budget.footprint_area_m2;
    Ok(GeometryParityComparison {
        passed,
        max_coordinate_delta_m,
        footprint_area_delta_m2,
        gross_floor_area_delta_m2,
        transform_area_delta_m2,
        budget,
    })
}

pub fn evaluate_variant(
    spec: &NormalizedSpec,
    variant: &Variant,
) -> Result<EvaluationReport, ArchyError> {
    variant.validate()?;
    let [sx, sy, sr, st] = site_bounds(spec)?;
    if variant.project_id != spec.project_id {
        return Err(ArchyError::schema(
            "project_id",
            "Variant and project identifiers differ",
        ));
    }
    let polygon = validated_polygon(&variant.footprint_xy_m)?;
    let bounds = polygon.bounding_rect().ok_or_else(|| {
        ArchyError::new(
            ErrorCode::InvalidGeometry,
            "footprint_xy_m",
            "Missing bounds",
        )
    })?;
    let (x, y, r, t) = (
        bounds.min().x,
        bounds.min().y,
        bounds.max().x,
        bounds.max().y,
    );
    let area = polygon.unsigned_area();
    let gfa = area * f64::from(variant.storeys);
    let height = variant.floor_to_floor_m * f64::from(variant.storeys);
    let clearances = [x - sx, sr - r, y - sy, st - t];
    let minimum = clearances.iter().copied().fold(f64::INFINITY, f64::min);
    let mut metrics = Vec::new();
    for (name, value, unit) in [
        ("footprint_area", area, "m2"),
        ("gross_floor_area", gfa, "m2"),
        ("height", height, "m"),
        ("xmin", x, "m"),
        ("ymin", y, "m"),
        ("xmax", r, "m"),
        ("ymax", t, "m"),
        ("clearance_west", clearances[0], "m"),
        ("clearance_east", clearances[1], "m"),
        ("clearance_south", clearances[2], "m"),
        ("clearance_north", clearances[3], "m"),
        ("target_gfa_delta", gfa - spec.brief.target_gfa_m2, "m2"),
    ] {
        if !value.is_finite() {
            return Err(ArchyError::new(
                ErrorCode::InvalidGeometry,
                name,
                "Measurement arithmetic overflow",
            ));
        }
        metrics.push(Metric {
            name: name.into(),
            value,
            unit: unit.into(),
            method_version: EVALUATION_VERSION.into(),
        });
    }
    let make = |id: &str, measured: f64, limit: f64, pass: bool, explanation: String| Check {
        constraint_id: id.into(),
        result: if pass {
            CheckResult::Pass
        } else {
            CheckResult::Fail
        },
        measured: Some(measured),
        limit: Some(limit),
        explanation,
        required: true,
    };
    let mut checks = vec![make(
        "site_containment",
        minimum,
        0.,
        minimum >= -TOLERANCE_M,
        "Minimum signed clearance to declared site; length tolerance applies".into(),
    )];
    for constraint in &spec.constraints {
        let (measured, pass) = match constraint.kind {
            ConstraintKind::MaxHeight => (height, height - constraint.value_m <= TOLERANCE_M),
            ConstraintKind::MinEdgeSetback => {
                (minimum, constraint.value_m - minimum <= TOLERANCE_M)
            }
        };
        checks.push(make(
            &constraint.id,
            measured,
            constraint.value_m,
            pass,
            format!(
                "Declared {:?}; source={}; authority={} (unverified); geometric measurement only",
                constraint.kind, constraint.source, constraint.authority
            ),
        ));
    }
    if spec.brief.target_gfa_hard {
        checks.push(make(
            "target_gfa",
            gfa,
            spec.brief.target_gfa_m2,
            spec.brief.target_gfa_m2 - gfa <= TOLERANCE_M2,
            "Hard target interpreted as minimum approximate GFA; area tolerance applies".into(),
        ));
    }
    let required_checks_passed = required_checks_pass(&checks);
    let geometry_parity_budget = geometry_parity_budget(variant)?;
    Ok(EvaluationReport {
        variant_id: variant.id.clone(),
        evaluation_version: EVALUATION_VERSION.into(),
        metrics,
        checks,
        tolerance: DimensionalTolerance {
            length_m: TOLERANCE_M,
            area_m2: TOLERANCE_M2,
            method_version: "absolute-si-v1".into(),
        },
        geometry_parity_budget,
        required_checks_passed,
        professional_limits: PROFESSIONAL_LIMITS.into(),
    })
}
