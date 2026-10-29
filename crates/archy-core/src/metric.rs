use crate::{ArchyError, Check, required_checks_pass};
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
pub struct GeometryParityBudget {
    pub policy_version: String,
    pub coordinate_length_m: f64,
    pub footprint_area_m2: f64,
    pub gross_floor_area_m2: f64,
    pub rigid_transform_policy_version: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "EvaluationReportData", deny_unknown_fields)]
pub struct EvaluationReport {
    pub variant_id: String,
    pub evaluation_version: String,
    pub metrics: Vec<Metric>,
    pub checks: Vec<Check>,
    pub tolerance: DimensionalTolerance,
    pub geometry_parity_budget: GeometryParityBudget,
    pub required_checks_passed: bool,
    pub professional_limits: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "EvaluationReport")]
struct EvaluationReportData {
    variant_id: String,
    evaluation_version: String,
    metrics: Vec<Metric>,
    checks: Vec<Check>,
    tolerance: DimensionalTolerance,
    geometry_parity_budget: GeometryParityBudget,
    required_checks_passed: bool,
    professional_limits: String,
}

impl TryFrom<EvaluationReportData> for EvaluationReport {
    type Error = ArchyError;

    fn try_from(data: EvaluationReportData) -> Result<Self, Self::Error> {
        if data.required_checks_passed != required_checks_pass(&data.checks) {
            return Err(ArchyError::schema(
                "required_checks_passed",
                "Aggregate differs from required checks",
            ));
        }
        Ok(Self {
            variant_id: data.variant_id,
            evaluation_version: data.evaluation_version,
            metrics: data.metrics,
            checks: data.checks,
            tolerance: data.tolerance,
            geometry_parity_budget: data.geometry_parity_budget,
            required_checks_passed: data.required_checks_passed,
            professional_limits: data.professional_limits,
        })
    }
}
