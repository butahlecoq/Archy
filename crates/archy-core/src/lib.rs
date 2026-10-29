//! Versioned brief parsing and canonical SI normalization. No backend is needed.
pub mod check;
pub mod error;
pub mod metric;
pub mod project;
pub mod units;
pub mod validation;
pub mod variant;
pub use check::*;
pub use error::{ArchyError, ErrorCode};
pub use metric::*;
pub use project::*;
pub use validation::{InputFormat, normalize, parse_project};
pub use variant::*;

pub fn project_schema() -> schemars::Schema {
    schemars::schema_for!(Project)
}
pub fn normalized_schema() -> schemars::Schema {
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
