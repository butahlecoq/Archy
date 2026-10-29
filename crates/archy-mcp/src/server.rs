use crate::tools::*;
use archy_cli::{BoundProjectRead, MutationOptions, render_verified_comparison};
use archy_core::{ArchyError, ErrorCode, sha256_hex};
use archy_report::ComparisonDocument;
use rmcp::{
    RoleServer, ServerHandler,
    model::{
        CallToolRequestMethod, CallToolRequestParams, CallToolResponse, CallToolResult, ErrorData,
        InitializeRequestParams, InitializeResult, ListToolsResult, PaginatedRequestParams,
        ProtocolVersion, ServerCapabilities, ServerConfig, Tool,
    },
    service::RequestContext,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    borrow::Cow,
    collections::HashSet,
    sync::{Arc, Mutex},
};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct ArchyServer {
    project: Arc<BoundProjectRead>,
    slots: Arc<Semaphore>,
    mutations: Arc<Mutex<()>>,
}
impl ArchyServer {
    pub fn new(project: BoundProjectRead) -> Self {
        Self {
            project: Arc::new(project),
            slots: Arc::new(Semaphore::new(4)),
            mutations: Arc::new(Mutex::new(())),
        }
    }
    fn call(&self, request: CallToolRequestParams) -> Result<CallToolResponse, ErrorData> {
        let name = request.name.as_ref();
        if !NAMES.contains(&name) {
            return Err(ErrorData::method_not_found::<CallToolRequestMethod>());
        }
        if request.input_responses.is_some() || request.request_state.is_some() {
            return Err(ErrorData::invalid_params(
                "Input requests are unsupported",
                None,
            ));
        }
        let args = Value::Object(request.arguments.unwrap_or_default());
        validate_input_bounds(&args)?;
        let _permit = match self.slots.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                let mut error = ArchyError::new(
                    ErrorCode::BackendUnavailable,
                    "tool",
                    "Concurrent tool limit reached",
                );
                error.retriable = true;
                return Ok(error_tool::<PrerequisiteData>(error).into());
            }
        };
        let result = match name {
            "get_project" => self.get_project(parse(args)?),
            "validate_project" => self.validate_project(parse(args)?),
            "list_variants" => self.list_variants(parse(args)?),
            "generate_variants" => self.generate_variants(parse(args)?),
            "evaluate_variant" => self.evaluate_variant(parse(args)?),
            "render_variant" => self.unavailable::<RenderVariantArgs>(
                args,
                name,
                "Verified render service is unavailable",
            ),
            "compare_variants" => self.compare_variants(parse(args)?),
            "export_ifc" => self.unavailable::<ExportIfcArgs>(
                args,
                name,
                "Verified IFC export service is unavailable",
            ),
            "propose_approval" => self.unavailable::<ProposeApprovalArgs>(
                args,
                name,
                "Proposal service is unavailable",
            ),
            _ => unreachable!(),
        };
        Ok(result.into())
    }
    fn ensure_id(&self, id: &str) -> Result<(), ArchyError> {
        if id == self.project.project_id() {
            Ok(())
        } else {
            Err(ArchyError::new(
                ErrorCode::PermissionDenied,
                "project_id",
                "Project ID does not match the configured project",
            ))
        }
    }
    fn generate_variants(&self, args: GenerateVariantsArgs) -> CallToolResult {
        result_tool((|| {
            self.ensure_id(&args.project_id)?;
            let seed = decimal(&args.seed, "seed")?;
            let options = mutation_options(
                args.idempotency_key,
                args.expected_project_revision,
                args.destination,
            )?;
            let _guard = self.mutations.lock().map_err(|_| {
                ArchyError::new(
                    ErrorCode::InternalError,
                    "mutation",
                    "Transaction gate is unavailable",
                )
            })?;
            let receipt = self.project.generate(args.count, seed, options)?;
            Ok(MutationData::from_receipt(args.project_id, receipt))
        })())
    }
    fn evaluate_variant(&self, args: EvaluateVariantArgs) -> CallToolResult {
        result_tool((|| {
            self.ensure_id(&args.project_id)?;
            if !valid_run_uuid(&args.source_run) {
                return Err(ArchyError::schema("source_run", "Expected a run UUID"));
            }
            let options = mutation_options(
                args.idempotency_key,
                args.expected_project_revision,
                args.destination,
            )?;
            let _guard = self.mutations.lock().map_err(|_| {
                ArchyError::new(
                    ErrorCode::InternalError,
                    "mutation",
                    "Transaction gate is unavailable",
                )
            })?;
            let receipt = self
                .project
                .evaluate(args.source_run, args.variant_id, options)?;
            Ok(MutationData::from_receipt(args.project_id, receipt))
        })())
    }
    fn get_project(&self, args: GetProjectArgs) -> CallToolResult {
        let result = (|| {
            self.ensure_id(&args.project_id)?;
            let snapshot = self.project.snapshot()?;
            snapshot.verify_head()?;
            let (revision, source_run, normalized, digest) = if let Some(text) = args.revision {
                let revision = decimal(&text, "revision")?;
                if revision > snapshot.revision() {
                    return Err(ArchyError::new(
                        ErrorCode::RevisionConflict,
                        "revision",
                        "Revision is not accepted",
                    ));
                }
                if revision == 0 {
                    if snapshot.revision() != 0 {
                        return Err(ArchyError::new(
                            ErrorCode::RevisionConflict,
                            "revision",
                            "Revision 0 has no immutable source snapshot",
                        ));
                    }
                    (
                        0,
                        None,
                        snapshot.spec().clone(),
                        snapshot.source_sha256().to_owned(),
                    )
                } else {
                    let receipt = snapshot
                        .receipts()
                        .find(|r| r.revision == revision)
                        .ok_or_else(|| {
                            ArchyError::new(
                                ErrorCode::RevisionConflict,
                                "revision",
                                "Revision is not accepted",
                            )
                        })?;
                    let run = snapshot.verify_run(&receipt.run_id)?;
                    (
                        revision,
                        Some(receipt.run_id.clone()),
                        run.spec().clone(),
                        run.manifest().input_sha256.clone(),
                    )
                }
            } else {
                (
                    snapshot.revision(),
                    None,
                    snapshot.spec().clone(),
                    snapshot.source_sha256().to_owned(),
                )
            };
            Ok(GetProjectData {
                project_id: args.project_id,
                revision: revision.to_string(),
                source_sha256: digest,
                source_run,
                normalized,
                read_operations: vec![
                    "get_project".into(),
                    "validate_project".into(),
                    "list_variants".into(),
                    "compare_variants".into(),
                ],
            })
        })();
        result_tool(result)
    }
    fn validate_project(&self, args: ValidateProjectArgs) -> CallToolResult {
        result_tool((|| {
            self.ensure_id(&args.project_id)?;
            let snapshot = self.project.snapshot()?;
            snapshot.verify_head()?;
            Ok(ValidateProjectData {
                project_id: args.project_id,
                source_sha256: snapshot.source_sha256().into(),
                normalized: snapshot.spec().clone(),
            })
        })())
    }
    fn list_variants(&self, args: ListVariantsArgs) -> CallToolResult {
        result_tool((|| {
            self.ensure_id(&args.project_id)?;
            let limit = args.limit.unwrap_or(100);
            if !(1..=100).contains(&limit) {
                return Err(ArchyError::schema("limit", "Limit must be 1..=100"));
            }
            if args.source_run.as_ref().is_some_and(|s| s.len() > 128) {
                return Err(ArchyError::schema("source_run", "Run ID too long"));
            }
            let snapshot = self.project.snapshot()?;
            let binding = sha256_hex(
                format!(
                    "archy-mcp-page-v2\0{}:{}:{}:{}:{}:{}",
                    snapshot.identity(),
                    args.project_id,
                    snapshot.source_sha256(),
                    snapshot.revision(),
                    snapshot.head_sha256().unwrap_or(""),
                    args.source_run.as_deref().unwrap_or("")
                )
                .as_bytes(),
            );
            let cursor = args.cursor.as_deref().unwrap_or("0");
            if cursor.len() > 256 {
                return Err(ArchyError::schema("cursor", "Cursor too long"));
            }
            let (bound, offset, last_id) = if cursor == "0" {
                (binding.as_str(), 0, None)
            } else {
                let mut parts = cursor.splitn(3, ':');
                let bound = parts.next().unwrap_or("");
                let text = parts
                    .next()
                    .ok_or_else(|| ArchyError::schema("cursor", "Invalid cursor"))?;
                let last = parts
                    .next()
                    .ok_or_else(|| ArchyError::schema("cursor", "Invalid cursor"))?;
                if last.is_empty() || last.len() > 128 {
                    return Err(ArchyError::schema("cursor", "Invalid cursor"));
                }
                let offset = decimal(text, "cursor")?;
                if offset == 0 {
                    return Err(ArchyError::schema("cursor", "Invalid cursor"));
                }
                (
                    bound,
                    usize::try_from(offset)
                        .map_err(|_| ArchyError::schema("cursor", "Invalid cursor"))?,
                    Some(last),
                )
            };
            if bound != binding {
                return Err(ArchyError::new(
                    ErrorCode::RevisionConflict,
                    "cursor",
                    "Cursor is stale or belongs to another query",
                ));
            }
            let mut variants = Vec::new();
            let mut seen = 0usize;
            let mut more = false;
            let mut matched_run = args.source_run.is_none();
            for receipt in snapshot.receipts().rev() {
                if args
                    .source_run
                    .as_ref()
                    .is_some_and(|id| receipt.run_id != *id)
                {
                    continue;
                }
                matched_run = true;
                let trusted = snapshot.verify_run(&receipt.run_id)?;
                for (variant, evaluation) in trusted.variants().iter().zip(trusted.evaluations()) {
                    if seen < offset {
                        seen += 1;
                        if seen == offset && Some(variant.id.as_str()) != last_id {
                            return Err(ArchyError::new(
                                ErrorCode::RevisionConflict,
                                "cursor",
                                "Cursor's last variant changed",
                            ));
                        }
                        continue;
                    }
                    if variants.len() >= limit as usize {
                        more = true;
                        break;
                    }
                    variants.push(VariantSummary {
                        run_id: receipt.run_id.clone(),
                        revision: receipt.revision.to_string(),
                        variant_id: variant.id.clone(),
                        required_checks_passed: evaluation.required_checks_passed,
                        artifact_count: trusted.manifest().artifacts.len(),
                    });
                    seen += 1;
                }
                if more {
                    break;
                }
            }
            if !matched_run {
                return Err(ArchyError::schema("source_run", "Run is not accepted"));
            }
            if offset > seen {
                return Err(ArchyError::schema(
                    "cursor",
                    "Cursor exceeds available variants",
                ));
            }
            let next_cursor = more.then(|| {
                let last = &variants.last().expect("nonempty bounded page").variant_id;
                format!("{binding}:{seen}:{last}")
            });
            Ok(ListVariantsData {
                project_id: args.project_id,
                revision: snapshot.revision().to_string(),
                variants,
                next_cursor,
            })
        })())
    }
    fn compare_variants(&self, args: CompareVariantsArgs) -> CallToolResult {
        result_tool((|| {
            self.ensure_id(&args.project_id)?;
            if args.objective != "absolute_target_gfa_delta" {
                return Err(ArchyError::schema(
                    "objective",
                    "Only absolute_target_gfa_delta is supported",
                ));
            }
            if args.run_ids.is_empty() || args.run_ids.len() > 100 {
                return Err(ArchyError::schema("run_ids", "Select 1..=100 runs"));
            }
            let mut unique = HashSet::new();
            if args.run_ids.iter().any(|id| !unique.insert(id)) {
                return Err(ArchyError::schema("run_ids", "Duplicate run ID"));
            }
            let limit = args.limit.unwrap_or(1);
            if !(1..=100).contains(&limit) {
                return Err(ArchyError::schema("limit", "Limit must be 1..=100"));
            }
            let snapshot = self.project.snapshot()?;
            snapshot.verify_head()?;
            let receipts = args
                .run_ids
                .iter()
                .map(|id| {
                    snapshot
                        .receipts()
                        .find(|receipt| &receipt.run_id == id)
                        .ok_or_else(|| ArchyError::schema("run_ids", "Run is not accepted"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let binding_bytes = serde_json::to_vec(&(
                "archy-mcp-comparison-page-v1",
                snapshot.identity(),
                &args.project_id,
                snapshot.source_sha256(),
                snapshot.revision(),
                snapshot.head_sha256(),
                &args.objective,
                &args.run_ids,
            ))
            .expect("bounded comparison query");
            let binding = sha256_hex(&binding_bytes);
            let offset = match args.cursor.as_deref() {
                None | Some("0") => 0,
                Some(cursor) => {
                    let mut parts = cursor.splitn(3, ':');
                    let bound = parts.next().unwrap_or("");
                    let number = parts
                        .next()
                        .ok_or_else(|| ArchyError::schema("cursor", "Invalid comparison cursor"))?;
                    let last = parts
                        .next()
                        .ok_or_else(|| ArchyError::schema("cursor", "Invalid comparison cursor"))?;
                    if bound != binding {
                        return Err(ArchyError::new(
                            ErrorCode::RevisionConflict,
                            "cursor",
                            "Cursor is stale or belongs to another comparison",
                        ));
                    }
                    let offset = usize::try_from(decimal(number, "cursor")?)
                        .map_err(|_| ArchyError::schema("cursor", "Invalid comparison cursor"))?;
                    if offset == 0
                        || offset >= args.run_ids.len()
                        || args.run_ids[offset - 1] != last
                    {
                        return Err(ArchyError::schema("cursor", "Invalid comparison cursor"));
                    }
                    offset
                }
            };
            let end = (offset + limit as usize).min(receipts.len());
            let mut comparisons = Vec::with_capacity(end - offset);
            for receipt in &receipts[offset..end] {
                let trusted = snapshot.verify_run(&receipt.run_id)?;
                let artifacts = render_verified_comparison(
                    &trusted,
                    &receipt.manifest_sha256,
                    snapshot.revision(),
                    snapshot.head_sha256().map(str::to_owned),
                )?;
                let document: ComparisonDocument = serde_json::from_slice(&artifacts.json)
                    .map_err(|_| {
                        ArchyError::new(
                            ErrorCode::InternalError,
                            "comparison",
                            "Typed comparison output could not be decoded",
                        )
                    })?;
                comparisons.push(document);
            }
            let next_cursor = (end < receipts.len())
                .then(|| format!("{binding}:{end}:{}", args.run_ids[end - 1]));
            Ok(CompareVariantsData {
                project_id: args.project_id,
                revision: snapshot.revision().to_string(),
                objective: args.objective,
                comparisons,
                next_cursor,
            })
        })())
    }
    fn unavailable<T: DeserializeOwned + ProjectId>(
        &self,
        args: Value,
        field: &str,
        reason: &str,
    ) -> CallToolResult {
        let parsed: T = match parse(args) {
            Ok(v) => v,
            Err(_) => {
                return error_tool::<PrerequisiteData>(ArchyError::schema(
                    field,
                    "Invalid tool arguments",
                ));
            }
        };
        let error = self
            .ensure_id(parsed.project_id())
            .err()
            .unwrap_or_else(|| ArchyError::new(ErrorCode::BackendUnavailable, field, reason));
        error_tool::<PrerequisiteData>(error)
    }
}
fn mutation_options(
    key: String,
    revision: String,
    destination: String,
) -> Result<MutationOptions, ArchyError> {
    Ok(MutationOptions {
        idempotency_key: Some(key),
        expected_project_revision: Some(decimal(&revision, "expected_project_revision")?),
        destination,
    })
}
fn valid_run_uuid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
fn validate_input_bounds(args: &Value) -> Result<(), ErrorData> {
    let Some(fields) = args.as_object() else {
        return Err(ErrorData::invalid_params(
            "Arguments must be an object",
            None,
        ));
    };
    for (field, value) in fields {
        if value.is_null()
            && matches!(
                field.as_str(),
                "revision" | "source_run" | "limit" | "cursor"
            )
        {
            continue;
        }
        if field != "run_ids"
            && let Some(bound) = string_bound(field)
        {
            if value.as_str().is_none_or(|text| !bound.accepts(text)) {
                return Err(ErrorData::invalid_params(
                    "Arguments exceed the tool schema bounds",
                    None,
                ));
            }
            continue;
        }
        let invalid = match field.as_str() {
            "revision" | "expected_project_revision" | "seed" => value
                .as_str()
                .is_none_or(|text| decimal(text, field).is_err()),
            "limit" => value
                .as_u64()
                .is_none_or(|number| !(1..=100).contains(&number)),
            "count" => value
                .as_u64()
                .is_none_or(|number| !(1..=1000).contains(&number)),
            "run_ids" => value.as_array().is_none_or(|ids| {
                ids.is_empty()
                    || ids.len() > 100
                    || ids.iter().any(|id| {
                        id.as_str().is_none_or(|text| {
                            !string_bound("run_ids").expect("run ID bound").accepts(text)
                        })
                    })
            }),
            _ => false,
        };
        if invalid {
            return Err(ErrorData::invalid_params(
                "Arguments exceed the tool schema bounds",
                None,
            ));
        }
    }
    Ok(())
}
fn decimal(text: &str, field: &str) -> Result<u64, ArchyError> {
    if text.is_empty()
        || text.len() > 20
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ArchyError::schema(
            field,
            "Expected canonical unsigned decimal string",
        ));
    }
    text.parse()
        .map_err(|_| ArchyError::schema(field, "Unsigned decimal value is out of range"))
}
fn parse<T: DeserializeOwned>(value: Value) -> Result<T, ErrorData> {
    serde_json::from_value(value)
        .map_err(|_| ErrorData::invalid_params("Arguments do not match the tool schema", None))
}
fn result_tool<T: Serialize>(value: Result<T, ArchyError>) -> CallToolResult {
    match value {
        Ok(data) => packed(
            Envelope {
                ok: true,
                data: Some(data),
                error: None,
            },
            false,
        ),
        Err(error) => error_tool::<T>(error),
    }
}
fn error_tool<T: Serialize>(error: ArchyError) -> CallToolResult {
    packed(
        Envelope::<T> {
            ok: false,
            data: None,
            error: Some(error),
        },
        true,
    )
}
fn packed<T: Serialize>(envelope: T, error: bool) -> CallToolResult {
    let value = serde_json::to_value(envelope).expect("typed envelope");
    // The SDK emits both structured and text copies inside a JSON-RPC frame.
    if value.to_string().len() > 400_000 {
        let small = serde_json::json!({"ok":false,"data":null,"error":ArchyError::new(ErrorCode::ArtifactInvalid,"response","Response exceeds MCP bound; select a smaller page")});
        return CallToolResult::structured_error(small);
    }
    if error {
        CallToolResult::structured_error(value)
    } else {
        CallToolResult::structured(value)
    }
}
trait ProjectId {
    fn project_id(&self) -> &str;
}
macro_rules! project_id { ($($t:ty),*) => { $(impl ProjectId for $t { fn project_id(&self) -> &str { &self.project_id } })* }; }
project_id!(
    GenerateVariantsArgs,
    EvaluateVariantArgs,
    RenderVariantArgs,
    CompareVariantsArgs,
    ExportIfcArgs,
    ProposeApprovalArgs
);
impl ServerHandler for ArchyServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(vec![
            ProtocolVersion::V_2025_11_25,
            ProtocolVersion::V_2026_07_28,
        ])
    }
    fn negotiate_initialize(
        &self,
        request: &InitializeRequestParams,
    ) -> Result<InitializeResult, ErrorData> {
        let versions = self.supported_protocol_versions();
        if !versions.contains(&request.protocol_version) {
            return Err(ErrorData::unsupported_protocol_version(
                request.protocol_version.clone(),
                &versions,
            ));
        }
        Ok(self
            .get_info()
            .with_protocol_version(request.protocol_version.clone()))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let server = self.clone();
        tokio::task::spawn_blocking(move || server.call(request))
            .await
            .map_err(|_| ErrorData::internal_error("Read task failed", None))?
    }
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(definitions()))
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        definitions().into_iter().find(|t| t.name == name)
    }
}
