use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RUN_SCHEMA_VERSION: &str = "archy-run-v1";
pub const RUN_REQUEST_SCHEMA_VERSION: &str = "archy-run-request-v1";
pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ARTIFACTS: usize = 4096;
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_RUN_BYTES: usize = 256 * 1024 * 1024;

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
    pub media_type: String,
    pub producer: String,
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    pub schema_version: String,
    pub project_id: String,
    pub run_id: String,
    pub idempotency_key: String,
    pub request_sha256: String,
    pub request_schema_version: String,
    pub expected_project_revision: u64,
    pub operation: String,
    pub parent_run_id: Option<String>,
    pub parent_variant_id: Option<String>,
    pub input_sha256: String,
    pub normalized_sha256: Option<String>,
    pub source_format: Option<String>,
    pub seed: Option<u64>,
    pub count: Option<u32>,
    pub algorithm_version: String,
    pub build_version: String,
    pub artifacts: Vec<Artifact>,
    pub outcomes: Vec<VariantOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VariantOutcome {
    pub variant_id: String,
    pub variant_artifact: Option<String>,
    pub evaluation_artifact: Option<String>,
    pub error_artifact: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MutationContext {
    pub idempotency_key: String,
    pub expected_project_revision: u64,
    pub destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub schema_version: String,
    pub context: MutationContext,
    pub operation: String,
    pub request_bytes: Vec<u8>,
    pub input_sha256: String,
    pub source_run: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunReceipt {
    pub run_id: String,
    pub revision: u64,
    pub manifest_sha256: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunMetadata {
    pub project_id: String,
    pub normalized_sha256: Option<String>,
    pub source_format: Option<String>,
    pub seed: Option<u64>,
    pub count: Option<u32>,
    pub parent_variant_id: Option<String>,
    pub algorithm_version: String,
}
