//! Build a worker request only from a locked, semantically verified accepted run.
use crate::{BoundProjectRead, artifact_invalid};
use archy_adapters::{
    BlenderRequest, CaptureOutcome, IfcRequest, QuotaCheckedScratch, ServiceRequest,
    ServiceSession, WorkerExit, capture_blender, capture_ifc,
};
use archy_core::{ArchyError, sha256_hex};
use std::path::Path;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptedWorkerBackend {
    Blender,
    Ifc,
}

/// The caller selects a run and a variant; their identity and digest are read from
/// accepted storage. The owned request remains a snapshot after the lock is released.
pub fn accepted_service_request(
    project: &Path,
    run_id: &str,
    variant_id: &str,
    request_id: &str,
    backend: AcceptedWorkerBackend,
) -> Result<ServiceRequest, ArchyError> {
    let reader = BoundProjectRead::open(project)?;
    reader.with_verified_locked_run(run_id, |run| {
        let variant = run
            .variants()
            .iter()
            .find(|variant| variant.id == variant_id)
            .ok_or_else(|| artifact_invalid("variant is absent from accepted run"))?;
        let digest = sha256_hex(run.input());
        let stored_run_id = &run.manifest().run_id;
        let request = match backend {
            AcceptedWorkerBackend::Blender => ServiceRequest::Blender(BlenderRequest::new(
                run.spec(),
                variant,
                request_id,
                stored_run_id,
                &digest,
                "output",
            )?),
            AcceptedWorkerBackend::Ifc => ServiceRequest::Ifc(IfcRequest::new(
                run.spec(),
                variant,
                request_id,
                stored_run_id,
                &digest,
                "output/ifc",
                Vec::new(),
            )?),
        };
        Ok(request)
    })
}

/// Resolve accepted evidence before entering the fixed A09 service boundary.
pub fn start_accepted_service(
    project: &Path,
    run_id: &str,
    variant_id: &str,
    request_id: &str,
    backend: AcceptedWorkerBackend,
) -> Result<ServiceSession, ArchyError> {
    ServiceSession::start(accepted_service_request(
        project, run_id, variant_id, request_id, backend,
    )?)
}

fn complete_service(
    request: ServiceRequest,
    cancel: &AtomicBool,
) -> Result<(QuotaCheckedScratch, WorkerExit), ArchyError> {
    let mut session = ServiceSession::start(request)?;
    session.release(cancel)?;
    session.finish(cancel)
}

fn checked_outcome(
    outcome: CaptureOutcome,
    exit: WorkerExit,
) -> Result<CaptureOutcome, ArchyError> {
    match (&outcome, exit) {
        (CaptureOutcome::Success(_), WorkerExit::Success)
        | (CaptureOutcome::WorkerError(_), WorkerExit::HandledError) => Ok(outcome),
        _ => Err(artifact_invalid(
            "worker exit and captured response disagree",
        )),
    }
}

pub(crate) fn run_blender_request(
    request: BlenderRequest,
    cancel: &AtomicBool,
) -> Result<CaptureOutcome, ArchyError> {
    let (scratch, exit) = complete_service(ServiceRequest::Blender(request.clone()), cancel)?;
    checked_outcome(capture_blender(scratch.as_file(), &request)?, exit)
}

/// Run an accepted request through the fixed service, verify teardown, then
/// capture bounded output bytes. Publication remains a separate decision.
pub fn run_accepted_service(
    project: &Path,
    run_id: &str,
    variant_id: &str,
    request_id: &str,
    backend: AcceptedWorkerBackend,
    cancel: &AtomicBool,
) -> Result<CaptureOutcome, ArchyError> {
    match accepted_service_request(project, run_id, variant_id, request_id, backend)? {
        ServiceRequest::Blender(request) => {
            let (scratch, exit) =
                complete_service(ServiceRequest::Blender(request.clone()), cancel)?;
            checked_outcome(capture_blender(scratch.as_file(), &request)?, exit)
        }
        ServiceRequest::Ifc(request) => {
            let (scratch, exit) = complete_service(ServiceRequest::Ifc(request.clone()), cancel)?;
            checked_outcome(capture_ifc(scratch.as_file(), &request)?, exit)
        }
    }
}
