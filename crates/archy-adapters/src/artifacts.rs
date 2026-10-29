use crate::response::{ClaimedArtifact, MAX_RESPONSE_BYTES};
use crate::{
    BlenderRequest, BlenderResponse, IfcRequest, IfcResponse, decode_blender_response,
    decode_ifc_response,
};
use archy_core::{ArchyError, ErrorCode, MAX_ARTIFACT_BYTES, sha256_hex};
use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, ResolveFlags, Stat};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;

const MAX_CAPTURE_BYTES: usize = 128 * 1024 * 1024;
const RESOLVE: ResolveFlags = ResolveFlags::BENEATH
    .union(ResolveFlags::NO_SYMLINKS)
    .union(ResolveFlags::NO_MAGICLINKS)
    .union(ResolveFlags::NO_XDEV);
const BLENDER: [(&str, &str); 5] = [
    ("scene.blend", "application/x-blender"),
    ("model.glb", "model/gltf-binary"),
    ("view-top.png", "image/png"),
    ("view-front.png", "image/png"),
    ("view-iso.png", "image/png"),
];
const IFC: [(&str, &str); 2] = [
    ("model.ifc", "application/x-step"),
    ("validation.json", "application/json"),
];

#[derive(Debug)]
pub enum CaptureOutcome {
    Success(CapturedOutputs),
    WorkerError(CapturedWorkerError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorBinding {
    RequestBound,
    UnboundDiagnostic,
}

#[derive(Debug)]
pub struct CapturedOutputs {
    response_bytes: Vec<u8>,
    response_sha256: String,
    artifacts: Vec<CapturedArtifact>,
}

impl CapturedOutputs {
    pub fn response_bytes(&self) -> &[u8] {
        &self.response_bytes
    }
    pub fn response_sha256(&self) -> &str {
        &self.response_sha256
    }
    pub fn artifacts(&self) -> &[CapturedArtifact] {
        &self.artifacts
    }
}

#[derive(Debug)]
pub struct CapturedArtifact {
    path: &'static str,
    media_type: &'static str,
    producer: &'static str,
    sha256: String,
    bytes: Vec<u8>,
}

impl CapturedArtifact {
    pub fn path(&self) -> &str {
        self.path
    }
    pub fn media_type(&self) -> &str {
        self.media_type
    }
    pub fn producer(&self) -> &str {
        self.producer
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug)]
pub struct CapturedWorkerError {
    response_bytes: Vec<u8>,
    response_sha256: String,
    error: ArchyError,
    binding: ErrorBinding,
    scratch_issue: Option<ArchyError>,
}

impl CapturedWorkerError {
    pub fn response_bytes(&self) -> &[u8] {
        &self.response_bytes
    }
    pub fn response_sha256(&self) -> &str {
        &self.response_sha256
    }
    pub fn binding(&self) -> ErrorBinding {
        self.binding
    }
    pub fn bound_error(&self) -> Option<&ArchyError> {
        (self.binding == ErrorBinding::RequestBound).then_some(&self.error)
    }
    pub fn diagnostic_error(&self) -> &ArchyError {
        &self.error
    }
    pub fn scratch_issue(&self) -> Option<&ArchyError> {
        self.scratch_issue.as_ref()
    }
}

fn invalid(message: impl Into<String>) -> ArchyError {
    ArchyError::new(ErrorCode::ArtifactInvalid, "scratch", message)
}

fn checked_stat(stat: &Stat, cap: usize) -> Result<(), ArchyError> {
    if !FileType::from_raw_mode(stat.st_mode).is_file()
        || stat.st_nlink != 1
        || stat.st_size <= 0
        || stat.st_size as u64 > cap as u64
    {
        return Err(invalid(
            "scratch entry is not a bounded single-link regular file",
        ));
    }
    Ok(())
}

fn same_stat(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_nlink == b.st_nlink
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}

fn names(
    scratch: &File,
    allowed: &[(&str, &str)],
    require_all: bool,
) -> Result<BTreeSet<String>, ArchyError> {
    let root = fs::fstat(scratch).map_err(|e| invalid(format!("scratch metadata: {e}")))?;
    if !FileType::from_raw_mode(root.st_mode).is_dir() {
        return Err(invalid("scratch capability is not a directory"));
    }
    let mut cursor =
        fs::Dir::read_from(scratch).map_err(|e| invalid(format!("scratch listing: {e}")))?;
    let mut found = BTreeSet::new();
    while let Some(entry) = cursor.read() {
        let entry = entry.map_err(|e| invalid(format!("scratch entry: {e}")))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| invalid("non-UTF8 scratch entry"))?;
        if name == "." || name == ".." {
            continue;
        }
        if !found.insert(name.to_owned())
            || found.len() > allowed.len() + 1
            || (name != "response.json" && !allowed.iter().any(|(expected, _)| name == *expected))
        {
            return Err(invalid("unexpected scratch entry"));
        }
    }
    if !found.contains("response.json") || (require_all && found.len() != allowed.len() + 1) {
        return Err(invalid("missing required scratch entry"));
    }
    Ok(found)
}

fn current_stat(scratch: &File, name: &str) -> Result<Stat, ArchyError> {
    fs::statat(scratch, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|e| invalid(format!("scratch entry changed: {e}")))
}

#[derive(Debug)]
struct ReadFile {
    bytes: Vec<u8>,
    sha256: String,
    stat: Stat,
}

fn read_file(
    scratch: &File,
    name: &str,
    cap: usize,
    total: &mut usize,
) -> Result<ReadFile, ArchyError> {
    read_file_with_after(scratch, name, cap, total, || {})
}

fn read_file_with_after(
    scratch: &File,
    name: &str,
    cap: usize,
    total: &mut usize,
    after_read: impl FnOnce(),
) -> Result<ReadFile, ArchyError> {
    let probe = fs::openat2(
        scratch,
        name,
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )
    .map_err(|e| invalid(format!("inspect scratch entry: {e}")))?;
    let before = fs::fstat(&probe).map_err(|e| invalid(format!("scratch entry metadata: {e}")))?;
    checked_stat(&before, cap)?;
    *total = total
        .checked_add(before.st_size as usize)
        .ok_or_else(|| invalid("capture size overflow"))?;
    if *total > MAX_CAPTURE_BYTES {
        return Err(invalid("capture aggregate size limit"));
    }
    let fd = fs::openat2(
        scratch,
        name,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )
    .map_err(|e| invalid(format!("open scratch file: {e}")))?;
    let opened = fs::fstat(&fd).map_err(|e| invalid(format!("opened file metadata: {e}")))?;
    checked_stat(&opened, cap)?;
    if !same_stat(&before, &opened) {
        return Err(invalid("scratch file changed before read"));
    }
    let mut file = File::from(fd);
    let mut bytes = Vec::with_capacity(opened.st_size as usize);
    file.by_ref()
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| invalid(format!("read scratch file: {e}")))?;
    if bytes.len() > cap || bytes.len() as i64 != opened.st_size {
        return Err(invalid("scratch file size changed during read"));
    }
    after_read();
    let after = fs::fstat(&file).map_err(|e| invalid(format!("final file metadata: {e}")))?;
    let path = current_stat(scratch, name)?;
    if !same_stat(&opened, &after) || !same_stat(&opened, &path) {
        return Err(invalid("scratch file changed during capture"));
    }
    let sha256 = sha256_hex(&bytes);
    Ok(ReadFile {
        bytes,
        sha256,
        stat: after,
    })
}

fn final_check(
    scratch: &File,
    allowed: &[(&str, &str)],
    captured: &[(&str, Stat)],
) -> Result<(), ArchyError> {
    names(scratch, allowed, true)?;
    for (name, original) in captured {
        if !same_stat(original, &current_stat(scratch, name)?) {
            return Err(invalid("scratch file changed after read"));
        }
    }
    Ok(())
}

fn ifc_directory(scratch: &File) -> Result<File, ArchyError> {
    let root = fs::fstat(scratch).map_err(|e| invalid(format!("scratch metadata: {e}")))?;
    if !FileType::from_raw_mode(root.st_mode).is_dir() {
        return Err(invalid("scratch capability is not a directory"));
    }
    let mut cursor =
        fs::Dir::read_from(scratch).map_err(|e| invalid(format!("scratch listing: {e}")))?;
    let mut found = false;
    while let Some(entry) = cursor.read() {
        let entry = entry.map_err(|e| invalid(format!("scratch entry: {e}")))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| invalid("non-UTF8 scratch entry"))?;
        if name == "." || name == ".." {
            continue;
        }
        if name != "ifc" || found {
            return Err(invalid("unexpected scratch entry"));
        }
        found = true;
    }
    if !found {
        return Err(invalid("missing IFC output directory"));
    }
    let fd = fs::openat2(
        scratch,
        "ifc",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )
    .map_err(|e| invalid(format!("open IFC output: {e}")))?;
    let directory = File::from(fd);
    let stat = fs::fstat(&directory).map_err(|e| invalid(format!("IFC output metadata: {e}")))?;
    if !FileType::from_raw_mode(stat.st_mode).is_dir() || stat.st_nlink != 2 {
        return Err(invalid("unsafe IFC output directory"));
    }
    Ok(directory)
}

fn check_ifc_directory(scratch: &File, directory: &File) -> Result<(), ArchyError> {
    let path = current_stat(scratch, "ifc")?;
    let pinned = fs::fstat(directory).map_err(|e| invalid(format!("IFC output metadata: {e}")))?;
    if !same_stat(&path, &pinned) {
        return Err(invalid("IFC output directory changed"));
    }
    let mut cursor =
        fs::Dir::read_from(scratch).map_err(|e| invalid(format!("scratch listing: {e}")))?;
    let mut entries = 0;
    while let Some(entry) = cursor.read() {
        let entry = entry.map_err(|e| invalid(format!("scratch entry: {e}")))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| invalid("non-UTF8 scratch entry"))?;
        if name != "." && name != ".." {
            entries += 1;
            if name != "ifc" || entries != 1 {
                return Err(invalid("unexpected scratch entry"));
            }
        }
    }
    Ok(())
}

fn success(
    scratch: &File,
    response: ReadFile,
    claims: &[ClaimedArtifact],
    allowed: &'static [(&'static str, &'static str)],
    producer: &'static str,
    total: &mut usize,
) -> Result<CaptureOutcome, ArchyError> {
    names(scratch, allowed, true)?;
    let mut artifacts = Vec::with_capacity(allowed.len());
    let mut captured = vec![("response.json", response.stat)];
    for ((name, media), claim) in allowed.iter().zip(claims) {
        if claim.path != *name || claim.media_type != *media || claim.producer != producer {
            return Err(invalid("worker artifact identity mismatch"));
        }
        let file = read_file(scratch, name, MAX_ARTIFACT_BYTES, total)?;
        if file.bytes.len() as u64 != claim.size_bytes || file.sha256 != claim.sha256 {
            return Err(invalid("worker artifact byte claim mismatch"));
        }
        captured.push((name, file.stat));
        artifacts.push(CapturedArtifact {
            path: name,
            media_type: media,
            producer,
            sha256: file.sha256,
            bytes: file.bytes,
        });
    }
    final_check(scratch, allowed, &captured)?;
    Ok(CaptureOutcome::Success(CapturedOutputs {
        response_bytes: response.bytes,
        response_sha256: response.sha256,
        artifacts,
    }))
}

fn worker_error(
    scratch: &File,
    response: ReadFile,
    error: ArchyError,
    binding: ErrorBinding,
    allowed: &[(&str, &str)],
) -> CaptureOutcome {
    let issue = (|| {
        let mut total = response.bytes.len();
        let found = names(scratch, allowed, false)?;
        for name in found {
            if name == "response.json" {
                continue;
            }
            let fd = fs::openat2(
                scratch,
                name.as_str(),
                OFlags::PATH | OFlags::CLOEXEC,
                Mode::empty(),
                RESOLVE,
            )
            .map_err(|e| invalid(format!("inspect partial file: {e}")))?;
            let stat = fs::fstat(fd).map_err(|e| invalid(format!("partial file metadata: {e}")))?;
            checked_stat(&stat, MAX_ARTIFACT_BYTES)?;
            total = total
                .checked_add(stat.st_size as usize)
                .ok_or_else(|| invalid("partial inventory size overflow"))?;
            if total > MAX_CAPTURE_BYTES {
                return Err(invalid("partial inventory aggregate size limit"));
            }
        }
        if !same_stat(&response.stat, &current_stat(scratch, "response.json")?) {
            return Err(invalid("worker error response changed during capture"));
        }
        Ok::<(), ArchyError>(())
    })()
    .err();
    CaptureOutcome::WorkerError(CapturedWorkerError {
        response_bytes: response.bytes,
        response_sha256: response.sha256,
        error,
        binding,
        scratch_issue: issue,
    })
}

pub fn capture_blender(
    scratch: &File,
    request: &BlenderRequest,
) -> Result<CaptureOutcome, ArchyError> {
    let mut total = 0;
    let response = read_file(scratch, "response.json", MAX_RESPONSE_BYTES, &mut total)?;
    match decode_blender_response(&response.bytes, request)? {
        BlenderResponse::Ok(success_response) => success(
            scratch,
            response,
            &success_response.artifacts,
            &BLENDER,
            "blender-worker-v1",
            &mut total,
        ),
        BlenderResponse::Error(failure) => Ok(worker_error(
            scratch,
            response,
            failure.error,
            ErrorBinding::UnboundDiagnostic,
            &BLENDER,
        )),
    }
}

pub fn capture_ifc(scratch: &File, request: &IfcRequest) -> Result<CaptureOutcome, ArchyError> {
    let directory = ifc_directory(scratch)?;
    let mut total = 0;
    let response = read_file(&directory, "response.json", MAX_RESPONSE_BYTES, &mut total)?;
    let outcome = match decode_ifc_response(&response.bytes, request)? {
        IfcResponse::Ok(success_response) => success(
            &directory,
            response,
            &success_response.artifacts,
            &IFC,
            "ifc-worker-v1",
            &mut total,
        ),
        IfcResponse::Error(failure) => Ok(worker_error(
            &directory,
            response,
            failure.error,
            ErrorBinding::RequestBound,
            &IFC,
        )),
    }?;
    check_ifc_directory(scratch, &directory)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;
    use std::io::Write;
    use std::os::fd::AsFd;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn late_same_size_write_and_device_metadata_are_rejected() {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "archy-late-capture-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("model.ifc"), b"before").unwrap();
        let directory = File::open(&root).unwrap();
        let mut total = 0;
        let result = read_file_with_after(
            &directory,
            "model.ifc",
            MAX_ARTIFACT_BYTES,
            &mut total,
            || {
                let mut file = File::options()
                    .write(true)
                    .open(root.join("model.ifc"))
                    .unwrap();
                file.write_all(b"after!").unwrap();
                file.set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1)))
                    .unwrap();
            },
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::ArtifactInvalid);
        std::fs::remove_dir_all(root).unwrap();
        let device = fs::open("/dev/null", OFlags::PATH, Mode::empty()).unwrap();
        let stat = fs::fstat(device.as_fd()).unwrap();
        assert_eq!(
            checked_stat(&stat, MAX_ARTIFACT_BYTES).unwrap_err().code,
            ErrorCode::ArtifactInvalid
        );
    }

    #[test]
    fn aggregate_limit_rejects_at_over_and_arithmetic_overflow() {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "archy-capture-total-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("model.ifc"), b"123456").unwrap();
        let directory = File::open(&root).unwrap();
        let mut total = MAX_CAPTURE_BYTES - 6;
        assert!(read_file(&directory, "model.ifc", MAX_ARTIFACT_BYTES, &mut total).is_ok());
        let mut total = MAX_CAPTURE_BYTES - 5;
        assert_eq!(
            read_file(&directory, "model.ifc", MAX_ARTIFACT_BYTES, &mut total)
                .unwrap_err()
                .code,
            ErrorCode::ArtifactInvalid
        );
        let mut total = usize::MAX;
        assert_eq!(
            read_file(&directory, "model.ifc", MAX_ARTIFACT_BYTES, &mut total)
                .unwrap_err()
                .code,
            ErrorCode::ArtifactInvalid
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
