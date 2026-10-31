//! Historical normalization stays available for accepted-run verification.
pub mod v1;
pub mod v2;
pub use v1::parse_project;
pub use v1::parse_project as parse_project_v1;
pub use v1::{InputFormat, MAX_INPUT_BYTES, TOLERANCE_M, TOLERANCE_M2, normalize};
pub(crate) use v1::{constraint_identifier, identifier};
pub use v2::{CurrentProject, parse_project_current};
