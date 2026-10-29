//! Typed, bounded worker protocol. File and process authority belongs to A09.

mod artifacts;
#[cfg(all(test, target_os = "linux"))]
mod process;
mod request;
mod response;

pub use artifacts::{
    CaptureOutcome, CapturedArtifact, CapturedOutputs, CapturedWorkerError, ErrorBinding,
    capture_blender, capture_ifc,
};

pub use request::{BlenderRequest, BlenderWire, IfcRequest, IfcWire, SourceArtifact};
pub use response::{
    BackendContract, BlenderResponse, IfcResponse, decode_blender_response, decode_ifc_response,
};

pub fn backend_schema() -> schemars::Schema {
    schemars::schema_for!(BackendContract)
}
