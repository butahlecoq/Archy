use crate::Check;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub method_version: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DimensionalTolerance {
    pub length_m: f64,
    pub area_m2: f64,
    pub method_version: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationReport {
    pub variant_id: String,
    pub evaluation_version: String,
    pub metrics: Vec<Metric>,
    pub checks: Vec<Check>,
    pub tolerance: DimensionalTolerance,
    pub required_checks_passed: bool,
    pub professional_limits: String,
}
