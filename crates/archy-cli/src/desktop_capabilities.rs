use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DESKTOP_CAPABILITIES_VERSION: &str = "archy-desktop-capabilities-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopOperation {
    pub id: DesktopOperationId,
    pub available: bool,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopOperationId {
    ProjectStatus,
    RunStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopCapabilities {
    pub schema_version: String,
    pub operations: Vec<DesktopOperation>,
}
pub fn desktop_capabilities() -> DesktopCapabilities {
    DesktopCapabilities {
        schema_version: DESKTOP_CAPABILITIES_VERSION.into(),
        operations: [
            DesktopOperationId::ProjectStatus,
            DesktopOperationId::RunStatus,
        ]
        .into_iter()
        .map(|id| DesktopOperation {
            id,
            available: true,
            reason: None,
        })
        .collect(),
    }
}
pub fn desktop_capabilities_schema() -> schemars::Schema {
    schemars::schema_for!(DesktopCapabilities)
}
