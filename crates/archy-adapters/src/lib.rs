//! Typed, bounded worker protocol. File and process authority belongs to A09.

mod artifacts;
#[cfg(target_os = "linux")]
mod isolation;
#[cfg(all(test, target_os = "linux"))]
mod process;
mod request;
mod response;
#[cfg(target_os = "linux")]
mod service;

pub use artifacts::{
    CaptureOutcome, CapturedArtifact, CapturedOutputs, CapturedWorkerError, ErrorBinding,
    capture_blender, capture_ifc,
};
#[cfg(target_os = "linux")]
pub use isolation::{QuotaCheckedScratch, receive_quota_checked_scratch};

pub use request::{BlenderRequest, BlenderWire, IfcRequest, IfcWire, SourceArtifact};
pub use response::{
    BackendContract, BlenderResponse, IfcResponse, decode_blender_response, decode_ifc_response,
};
#[cfg(target_os = "linux")]
pub use service::{BackendKind, ServiceRequest, ServiceSession, WorkerExit};

pub fn backend_schema() -> schemars::Schema {
    schemars::schema_for!(BackendContract)
}
