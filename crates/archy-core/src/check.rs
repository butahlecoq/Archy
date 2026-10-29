use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckResult { Pass, Fail, Warn, Unknown }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub constraint_id: String,
    pub result: CheckResult,
    pub measured: Option<f64>,
    pub limit: Option<f64>,
    pub explanation: String,
    pub required: bool,
}

/// Required WARN is also not a demonstrated pass.
pub fn required_checks_pass(checks: &[Check]) -> bool {
    checks.iter().all(|c| !c.required || c.result == CheckResult::Pass)
}
