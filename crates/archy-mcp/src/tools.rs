use archy_cli::{RenderReceipt, RenderResult, RenderVariant, WorkflowReceipt};
use archy_core::project::v2;
use archy_core::{ArchyError, NormalizedSpec, RunReceipt};
use archy_report::ComparisonDocument;
use rmcp::model::{Tool, ToolAnnotations};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetProjectArgs {
    pub project_id: String,
    pub revision: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateProjectArgs {
    pub project_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListVariantsArgs {
    pub project_id: String,
    pub source_run: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GenerateVariantsArgs {
    pub project_id: String,
    pub count: u32,
    pub seed: String,
    pub idempotency_key: String,
    pub expected_project_revision: String,
    pub destination: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluateVariantArgs {
    pub project_id: String,
    pub source_run: String,
    pub variant_id: String,
    pub idempotency_key: String,
    pub expected_project_revision: String,
    pub destination: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderVariantArgs {
    pub project_id: String,
    pub source_run: String,
    pub variant_id: String,
    pub preset: String,
    pub idempotency_key: String,
    pub expected_project_revision: String,
    pub destination: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompareVariantsArgs {
    pub project_id: String,
    pub run_ids: Vec<String>,
    pub objective: String,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportIfcArgs {
    pub project_id: String,
    pub source_run: String,
    pub variant_id: String,
    pub idempotency_key: String,
    pub expected_project_revision: String,
    pub destination: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposeApprovalArgs {
    pub project_id: String,
    pub source_run: String,
    pub variant_id: String,
    pub reason: String,
    pub idempotency_key: String,
    pub expected_project_revision: String,
    pub destination: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<ArchyError>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum NormalizedProject {
    Historical(NormalizedSpec),
    Current(v2::NormalizedSpec),
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetProjectData {
    pub project_id: String,
    pub revision: String,
    pub source_sha256: String,
    pub source_run: Option<String>,
    pub normalized: NormalizedProject,
    pub read_operations: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateProjectData {
    pub project_id: String,
    pub source_sha256: String,
    pub normalized: NormalizedProject,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VariantSummary {
    pub run_id: String,
    pub revision: String,
    pub variant_id: String,
    pub required_checks_passed: bool,
    pub artifact_count: usize,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListVariantsData {
    pub project_id: String,
    pub revision: String,
    pub variants: Vec<VariantSummary>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompareVariantsData {
    pub project_id: String,
    pub revision: String,
    pub objective: String,
    pub comparisons: Vec<ComparisonDocument>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrerequisiteData {
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MutationData {
    pub project_id: String,
    pub run_id: String,
    pub revision: String,
    pub path: String,
    pub variant_ids: Vec<String>,
    pub all_required_checks_passed: bool,
    pub exhaustion_explanations: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderData {
    pub project_id: String,
    pub run_id: String,
    pub revision: String,
    pub path: String,
    pub manifest_sha256: String,
    pub source_run_id: String,
    pub source_manifest_sha256: String,
    pub all_verified: bool,
    pub variants: Vec<RenderVariant>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportIfcData {
    pub project_id: String,
    pub run_id: String,
    pub revision: String,
    pub path: String,
    pub manifest_sha256: String,
    pub source_run_id: String,
    pub variant_id: String,
    pub scope: String,
    pub approval_state: String,
}
impl ExportIfcData {
    pub fn from_receipt(
        project_id: String,
        source_run_id: String,
        variant_id: String,
        receipt: RunReceipt,
    ) -> Self {
        Self {
            project_id,
            run_id: receipt.run_id,
            revision: receipt.revision.to_string(),
            path: receipt.path,
            manifest_sha256: receipt.manifest_sha256,
            source_run_id,
            variant_id,
            scope: "SCHEMATIC_MASSING_ONLY".into(),
            approval_state: "DRAFT".into(),
        }
    }
}
impl RenderData {
    pub fn from_receipt(project_id: String, receipt: RenderReceipt) -> Self {
        let all_verified = receipt
            .record
            .variants
            .iter()
            .all(|variant| matches!(&variant.result, RenderResult::Verified { .. }));
        Self {
            project_id,
            run_id: receipt.run_id,
            revision: receipt.revision.to_string(),
            path: receipt.path,
            manifest_sha256: receipt.manifest_sha256,
            source_run_id: receipt.record.source_run_id,
            source_manifest_sha256: receipt.record.source_manifest_sha256,
            all_verified,
            variants: receipt.record.variants,
        }
    }
}
impl MutationData {
    pub fn from_receipt(project_id: String, receipt: WorkflowReceipt) -> Self {
        Self {
            project_id,
            run_id: receipt.run_id,
            revision: receipt.revision.to_string(),
            path: receipt.path,
            variant_ids: receipt.variant_ids,
            all_required_checks_passed: receipt.all_required_checks_passed,
            exhaustion_explanations: receipt.exhaustion_explanations,
        }
    }
}

pub const NAMES: [&str; 9] = [
    "get_project",
    "validate_project",
    "list_variants",
    "generate_variants",
    "evaluate_variant",
    "render_variant",
    "compare_variants",
    "export_ifc",
    "propose_approval",
];

#[derive(Clone, Copy)]
pub(crate) struct StringBound {
    bytes: usize,
    ascii: bool,
    empty: bool,
}

pub(crate) fn string_bound(field: &str) -> Option<StringBound> {
    let (bytes, ascii, empty) = match field {
        "project_id" | "source_run" | "variant_id" | "idempotency_key" | "run_ids" => {
            (128, true, false)
        }
        "cursor" => (256, true, true),
        "preset" | "objective" => (128, false, false),
        "destination" => (512, false, false),
        "reason" => (4096, false, false),
        _ => return None,
    };
    Some(StringBound {
        bytes,
        ascii,
        empty,
    })
}

impl StringBound {
    pub(crate) fn accepts(self, text: &str) -> bool {
        if (!self.empty && text.is_empty()) || text.len() > self.bytes {
            return false;
        }
        if self.ascii {
            return text.is_ascii();
        }
        let width = text.chars().map(char::len_utf8).max().unwrap_or(1);
        text.chars().count() <= self.bytes / width
    }

    fn apply_schema(self, schema: &mut serde_json::Map<String, serde_json::Value>) {
        if !self.empty {
            schema.insert("minLength".into(), 1.into());
        }
        schema.insert("maxLength".into(), self.bytes.into());
        if self.ascii {
            schema.insert("pattern".into(), "^[\\u0000-\\u007f]*(?![\\s\\S])".into());
        } else {
            schema.insert(
                "anyOf".into(),
                serde_json::json!([
                    {"maxLength": self.bytes, "pattern": "^[\\u0000-\\u007f]*(?![\\s\\S])"},
                    {"maxLength": self.bytes / 2, "pattern": "^[\\u0000-\\u07ff]*(?![\\s\\S])"},
                    {"maxLength": self.bytes / 3, "pattern": "^[\\u0000-\\ud7ff\\ue000-\\uffff]*(?![\\s\\S])"},
                    {"maxLength": self.bytes / 4, "pattern": "^(?:[^\\ud800-\\udfff]|[\\ud800-\\udbff][\\udc00-\\udfff])*(?![\\s\\S])"}
                ]),
            );
        }
    }
}

fn typed<I: JsonSchema + 'static, O: JsonSchema + 'static>(
    name: &'static str,
    description: &'static str,
    read: bool,
) -> Tool {
    let mut tool = Tool::new(name, description, Arc::new(serde_json::Map::new()))
        .with_input_schema::<I>()
        .with_output_schema::<Envelope<O>>()
        .with_annotations(
            ToolAnnotations::new()
                .read_only(read)
                .open_world(false)
                .destructive(false),
        );
    let mut schema = tool.input_schema.as_ref().clone();
    if let Some(properties) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (field, value) in properties {
            let value = if value.get("type").is_none() {
                value
                    .get_mut("anyOf")
                    .and_then(serde_json::Value::as_array_mut)
                    .and_then(|items| {
                        items.iter_mut().find(|item| {
                            item.get("type").and_then(serde_json::Value::as_str) == Some("string")
                        })
                    })
            } else {
                Some(value)
            };
            if let Some(value) = value.and_then(serde_json::Value::as_object_mut) {
                if field != "run_ids"
                    && let Some(bound) = string_bound(field)
                {
                    bound.apply_schema(value);
                    continue;
                }
                match field.as_str() {
                    "revision" | "expected_project_revision" | "seed" => {
                        value.insert("pattern".into(), "^(0|[1-9][0-9]{0,19})$".into());
                    }
                    "limit" => {
                        value.insert("minimum".into(), 1.into());
                        value.insert("maximum".into(), 100.into());
                    }
                    "count" => {
                        value.insert("minimum".into(), 1.into());
                        value.insert("maximum".into(), 1000.into());
                    }
                    "run_ids" => {
                        value.insert("minItems".into(), 1.into());
                        value.insert("maxItems".into(), 100.into());
                        let items = value
                            .get_mut("items")
                            .and_then(serde_json::Value::as_object_mut)
                            .expect("run ID item schema");
                        string_bound(field)
                            .expect("run ID bound")
                            .apply_schema(items);
                    }
                    _ => {}
                }
            }
        }
    }
    tool.input_schema = Arc::new(schema);
    let mut output = tool
        .output_schema
        .as_ref()
        .expect("typed output schema")
        .as_ref()
        .clone();
    let properties = output
        .remove("properties")
        .and_then(|value| value.as_object().cloned())
        .expect("envelope properties");
    let data = present(properties.get("data").expect("envelope data"));
    let error = present(properties.get("error").expect("envelope error"));
    output.remove("required");
    output.remove("type");
    output.remove("additionalProperties");
    output.insert(
        "oneOf".into(),
        serde_json::json!([
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["ok", "data", "error"],
                "properties": {"ok": {"const": true}, "data": data, "error": {"type": "null"}}
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["ok", "data", "error"],
                "properties": {"ok": {"const": false}, "data": {"type": "null"}, "error": error}
            }
        ]),
    );
    tool.output_schema = Some(Arc::new(output));
    tool
}

fn present(value: &serde_json::Value) -> serde_json::Value {
    let variants = value
        .get("anyOf")
        .and_then(serde_json::Value::as_array)
        .expect("optional envelope member");
    let mut present = variants
        .iter()
        .filter(|variant| variant.get("type").and_then(serde_json::Value::as_str) != Some("null"));
    let one = present.next().expect("nonnull envelope member").clone();
    assert!(present.next().is_none(), "single nonnull envelope member");
    one
}
pub fn definitions() -> Vec<Tool> {
    vec![
        typed::<GetProjectArgs, GetProjectData>(
            NAMES[0],
            "Read the pinned project and accepted revision",
            true,
        ),
        typed::<ValidateProjectArgs, ValidateProjectData>(
            NAMES[1],
            "Validate the pinned project source",
            true,
        ),
        typed::<ListVariantsArgs, ListVariantsData>(
            NAMES[2],
            "List verified variants from accepted runs",
            true,
        ),
        typed::<GenerateVariantsArgs, MutationData>(
            NAMES[3],
            "Generate accepted variants in a scoped transaction",
            false,
        ),
        typed::<EvaluateVariantArgs, MutationData>(
            NAMES[4],
            "Evaluate one accepted variant in a scoped transaction",
            false,
        ),
        typed::<RenderVariantArgs, RenderData>(
            NAMES[5],
            "Render one evaluated variant using the fixed supervised Blender service and return a verified or failed immutable receipt",
            false,
        ),
        typed::<CompareVariantsArgs, CompareVariantsData>(
            NAMES[6],
            "Compare verified generation/edit runs by absolute target GFA delta; ranks are within each run",
            true,
        ),
        typed::<ExportIfcArgs, ExportIfcData>(
            NAMES[7],
            "Export one selected variant as draft schematic IFC through the fixed supervised service and verified CLI transaction",
            false,
        ),
        typed::<ProposeApprovalArgs, PrerequisiteData>(
            NAMES[8],
            "Unavailable until proposal integration; never final approval",
            false,
        ),
    ]
}
