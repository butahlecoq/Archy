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
    OpenReport,
    OpenPreview,
    OpenBlender,
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
            DesktopOperationId::OpenReport,
            DesktopOperationId::OpenPreview,
            DesktopOperationId::OpenBlender,
        ]
        .into_iter()
        .map(|id| {
            let available = match id {
                DesktopOperationId::ProjectStatus | DesktopOperationId::RunStatus => true,
                DesktopOperationId::OpenReport | DesktopOperationId::OpenPreview => std::path::Path::new("/usr/bin/xdg-open").is_file(),
                DesktopOperationId::OpenBlender => std::path::Path::new("/usr/bin/blender").is_file(),
            };
            DesktopOperation { id, available, reason: (!available).then(|| "Fixed viewer unavailable; artifact also must be verified in the selected accepted run".into()) }
        })
        .collect(),
    }
}
pub fn desktop_capabilities_schema() -> schemars::Schema {
    schemars::schema_for!(DesktopCapabilities)
}
