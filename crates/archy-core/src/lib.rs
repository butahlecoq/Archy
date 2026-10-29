//! Versioned brief parsing and canonical SI normalization. No backend is needed.
pub mod check;
pub mod decision_contract;
pub mod error;
pub mod metric;
pub mod project;
pub mod run;
pub mod state;
pub mod storage;
pub mod units;
pub mod validation;
pub mod variant;
pub use check::*;
pub use error::{ArchyError, ErrorCode};
pub use metric::*;
pub use project::*;
pub use run::*;
pub use state::*;
pub use storage::*;
pub use validation::{
    CurrentProject, InputFormat, normalize, parse_project, parse_project_current, parse_project_v1,
};
pub use variant::*;

pub fn project_schema() -> schemars::Schema {
    let mut schema = serde_json::to_value(schemars::schema_for!(project::v2::Project)).unwrap();
    schema["properties"]["id"]["pattern"] = serde_json::json!("^[A-Za-z0-9_-]{1,128}$");
    schema["properties"]["id"]["maxLength"] = serde_json::json!(128);
    schema["$defs"]["Constraint"]["properties"]["id"]["pattern"] =
        serde_json::json!("^(?!site_containment$|target_gfa$)[A-Za-z0-9_-]{1,128}$");
    for field in ["source", "authority"] {
        schema["$defs"]["Constraint"]["properties"][field]["pattern"] = serde_json::json!("\\S");
    }
    schema["$defs"]["Constraint"]["properties"]["bound"]["exclusiveMinimum"] = serde_json::json!(0);
    schema["$defs"]["Brief"]["properties"]["storeys"]["minimum"] = serde_json::json!(1);
    schema["$defs"]["Brief"]["properties"]["storeys"]["maximum"] = serde_json::json!(u32::MAX);
    for field in [
        "floor_to_floor",
        "floor_to_floor_m",
        "target_gfa",
        "target_gfa_m2",
    ] {
        schema["$defs"]["Brief"]["properties"][field]["exclusiveMinimum"] = serde_json::json!(0);
    }
    for field in ["width", "width_m", "depth", "depth_m"] {
        schema["$defs"]["Site"]["properties"][field]["exclusiveMinimum"] = serde_json::json!(0);
    }
    schema["$defs"]["Site"]["additionalProperties"] = serde_json::json!(false);
    for field in ["polygon_xy", "polygon_xy_m"] {
        schema["$defs"]["Site"]["properties"][field]["minItems"] = serde_json::json!(5);
        schema["$defs"]["Site"]["properties"][field]["maxItems"] = serde_json::json!(5);
    }
    schema["$defs"]["Generation"]["properties"]["count"]["minimum"] = serde_json::json!(1);
    schema["$defs"]["Generation"]["properties"]["count"]["maximum"] = serde_json::json!(1000);
    schema["$defs"]["Generation"]["properties"]["seed"]["maximum"] = serde_json::json!(u64::MAX);
    schema["$defs"]["Generation"]["properties"]["shapes"]["minItems"] = serde_json::json!(1);
    schema["$defs"]["Generation"]["properties"]["shapes"]["uniqueItems"] = serde_json::json!(true);
    serde_json::from_value(schema).unwrap()
}
pub fn normalized_schema() -> schemars::Schema {
    let mut schema =
        serde_json::to_value(schemars::schema_for!(project::v2::NormalizedSpec)).unwrap();
    schema["properties"]["id"]["pattern"] = serde_json::json!("^[A-Za-z0-9_-]{1,128}$");
    schema["$defs"]["NormalizedConstraint"]["properties"]["id"]["pattern"] =
        serde_json::json!("^(?!site_containment$|target_gfa$)[A-Za-z0-9_-]{1,128}$");
    schema["$defs"]["NormalizedConstraint"]["properties"]["bound"]["exclusiveMinimum"] =
        serde_json::json!(0);
    for field in ["source", "authority"] {
        schema["$defs"]["NormalizedConstraint"]["properties"][field]["pattern"] =
            serde_json::json!("\\S");
    }
    schema["$defs"]["NormalizedBrief"]["properties"]["storeys"]["minimum"] = serde_json::json!(1);
    schema["$defs"]["NormalizedBrief"]["properties"]["storeys"]["maximum"] =
        serde_json::json!(u32::MAX);
    for field in ["floor_to_floor_m", "target_gfa_m2"] {
        schema["$defs"]["NormalizedBrief"]["properties"][field]["exclusiveMinimum"] =
            serde_json::json!(0);
    }
    schema["properties"]["notes"]["items"] =
        serde_json::json!({"type": "string", "maxLength": 512});
    schema["$defs"]["Generation"]["properties"]["count"]["minimum"] = serde_json::json!(1);
    schema["$defs"]["Generation"]["properties"]["count"]["maximum"] = serde_json::json!(1000);
    schema["$defs"]["Generation"]["properties"]["seed"]["maximum"] = serde_json::json!(u64::MAX);
    schema["$defs"]["Generation"]["properties"]["shapes"]["minItems"] = serde_json::json!(1);
    schema["$defs"]["Generation"]["properties"]["shapes"]["uniqueItems"] = serde_json::json!(true);
    serde_json::from_value(schema).unwrap()
}
pub fn project_schema_v1() -> schemars::Schema {
    schemars::schema_for!(Project)
}
pub fn normalized_schema_v1() -> schemars::Schema {
    schemars::schema_for!(NormalizedSpec)
}
pub fn error_schema() -> schemars::Schema {
    schemars::schema_for!(ArchyError)
}
pub fn variant_schema() -> schemars::Schema {
    schemars::schema_for!(Variant)
}
pub fn evaluation_schema() -> schemars::Schema {
    schemars::schema_for!(EvaluationReport)
}
pub fn run_manifest_schema() -> schemars::Schema {
    schemars::schema_for!(RunManifest)
}
pub fn revision_schema() -> schemars::Schema {
    schemars::schema_for!(RevisionRecord)
}
pub fn run_request_schema() -> schemars::Schema {
    schemars::schema_for!(RunRequest)
}
pub fn proposal_schema() -> schemars::Schema {
    schemars::schema_for!(decision_contract::Proposal)
}
pub fn decision_record_schema() -> schemars::Schema {
    schemars::schema_for!(decision_contract::DecisionRecord)
}
