use crate::units::Units;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Structural input contract. Generic dimensions obey declared units; explicit
/// _m/_m2 fields always mean SI. Core additionally enforces exclusive dimension
/// representations, positive finite values, version and rectangular geometry.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: String,
    pub project_id: String,
    pub units: Units,
    pub site: Site,
    pub brief: Brief,
    pub constraints: Vec<Constraint>,
    /// Text preferences survive normalization without invented metrics.
    #[serde(default)]
    pub preferences: Vec<String>,
    #[serde(default)]
    pub generation: Generation,
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Site {
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub depth: Option<f64>,
    #[serde(default)]
    pub width_m: Option<f64>,
    #[serde(default)]
    pub depth_m: Option<f64>,
    #[serde(default)]
    pub polygon_xy: Option<Vec<[f64; 2]>>,
    #[serde(default)]
    pub polygon_xy_m: Option<Vec<[f64; 2]>>,
    #[serde(default)]
    pub north_deg: f64,
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Brief {
    pub storeys: u32,
    #[serde(default)]
    pub floor_to_floor: Option<f64>,
    #[serde(default)]
    pub floor_to_floor_m: Option<f64>,
    #[serde(default)]
    pub target_gfa: Option<f64>,
    #[serde(default)]
    pub target_gfa_m2: Option<f64>,
    #[serde(default)]
    pub target_gfa_hard: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
    MaxHeight,
    MinEdgeSetback,
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Constraint {
    pub id: String,
    pub kind: ConstraintKind,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub value_m: Option<f64>,
    #[serde(default)]
    pub unit: Option<Units>,
    pub source: String,
    #[serde(default)]
    pub authority: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    pub count: u32,
    pub seed: u64,
    pub shapes: Vec<Shape>,
}
impl Default for Generation {
    fn default() -> Self {
        Self {
            count: 3,
            seed: 42,
            shapes: vec![Shape::Rectangle, Shape::LShape],
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Rectangle,
    LShape,
}

/// Canonical SI project produced by core normalization, preserving declarations
/// and textual preferences without certifying regulatory compliance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedSpec {
    pub schema_version: String,
    pub project_id: String,
    pub units: Units,
    pub site: NormalizedSite,
    pub brief: NormalizedBrief,
    pub constraints: Vec<NormalizedConstraint>,
    pub preferences: Vec<String>,
    pub generation: Generation,
    pub normalization_version: String,
    pub tolerance_m: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedSite {
    pub polygon_xy_m: [[f64; 2]; 5],
    pub north_angle_deg: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedBrief {
    pub storeys: u32,
    pub floor_to_floor_m: f64,
    pub target_gfa_m2: f64,
    pub target_gfa_hard: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedConstraint {
    pub id: String,
    pub kind: ConstraintKind,
    pub value_m: f64,
    pub source: String,
    /// User declaration only. Absent authority becomes `assumption`.
    pub authority: String,
}
