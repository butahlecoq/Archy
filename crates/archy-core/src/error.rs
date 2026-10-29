use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidSchema, InvalidGeometry, ConstraintFail, BackendUnavailable, BackendTimeout,
    ArtifactInvalid, RevisionConflict, PermissionDenied, InternalError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchyError {
    pub code: ErrorCode,
    pub message: String,
    /// JSON-style dotted field path; `$` denotes the document itself.
    pub field: String,
    pub retriable: bool,
}
impl ArchyError {
    pub fn new(code: ErrorCode, field: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code, field: field.into(), message: message.into(), retriable: false }
    }
    pub fn schema(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidSchema, field, message)
    }
}
impl std::fmt::Display for ArchyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}: {}", self.field, self.message) }
}
impl std::error::Error for ArchyError {}
