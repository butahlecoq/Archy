use crate::RunReceipt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const REVISION_SCHEMA_VERSION: &str = "archy-revision-v1";
pub const MAX_REVISION_BYTES: usize = 65536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevisionRecord {
    pub schema_version: String,
    pub revision: u64,
    pub previous_sha256: Option<String>,
    pub idempotency_key: String,
    pub request_sha256: String,
    pub request_schema_version: String,
    pub receipt: RunReceipt,
}
