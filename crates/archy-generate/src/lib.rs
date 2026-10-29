//! Deterministic bounded massing alternatives.
use archy_core::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const ALGORITHM_VERSION:&str="chacha8-u53-massing-v1";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GenerationResult {
    pub variants: Vec<Variant>,
    pub requested_count:u32,
    pub attempts:u32,
    pub algorithm_version:String,
    pub exhaustion_explanations:Vec<String>,
}
pub fn generate_variants(_: &NormalizedSpec, _:u32, _:u64)->Result<GenerationResult,ArchyError> {
    Err(ArchyError::schema("$","Generation not implemented"))
}
