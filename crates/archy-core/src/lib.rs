//! Versioned brief parsing and canonical SI normalization. No backend is needed.
pub mod error;
pub mod project;
pub mod units;
pub mod validation;
pub use error::{ArchyError, ErrorCode};
pub use project::*;
pub use validation::{InputFormat, normalize, parse_project};

pub fn project_schema() -> schemars::Schema { schemars::schema_for!(Project) }
pub fn normalized_schema() -> schemars::Schema { schemars::schema_for!(NormalizedSpec) }
pub fn error_schema() -> schemars::Schema { schemars::schema_for!(ArchyError) }
