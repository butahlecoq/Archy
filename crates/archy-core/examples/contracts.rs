//! Export source-derived schemas and actual serialized values for independent checking.
use archy_core::*;

fn main() {
    let input = include_str!("../../../tests/fixtures/briefs/courtyard.json");
    let project: Project = serde_json::from_str(input).unwrap();
    let normalized = parse_project(input, InputFormat::Json).unwrap();
    let codes = [
        ErrorCode::InvalidSchema,
        ErrorCode::InvalidGeometry,
        ErrorCode::ConstraintFail,
        ErrorCode::BackendUnavailable,
        ErrorCode::BackendTimeout,
        ErrorCode::ArtifactInvalid,
        ErrorCode::RevisionConflict,
        ErrorCode::PermissionDenied,
        ErrorCode::InternalError,
    ];
    let errors: Vec<_> = codes
        .into_iter()
        .map(|code| ArchyError::new(code, "site.width", "Example"))
        .collect();
    println!(
        "{}",
        serde_json::json!({"schemas": {"project": project_schema(), "normalized-project": normalized_schema(), "error": error_schema(), "proposal": proposal_schema(), "decision-record": decision_record_schema()}, "project": project, "normalized": normalized, "errors": errors})
    );
}
