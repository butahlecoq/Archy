//! Linux-only, descriptor-relative storage. A lock serializes cooperating writers.
use crate::{
    ArchyError, Artifact, ErrorCode, MAX_ARTIFACT_BYTES, MAX_ARTIFACTS, MAX_MANIFEST_BYTES,
    MAX_REVISION_BYTES, MAX_RUN_BYTES, REVISION_SCHEMA_VERSION, RUN_REQUEST_SCHEMA_VERSION,
    RUN_SCHEMA_VERSION, RevisionRecord, RunManifest, RunMetadata, RunReceipt, RunRequest,
    VariantOutcome, sha256_hex,
};
use rustix::{
    fs::{self, Mode, OFlags, RenameFlags, ResolveFlags},
    io::Errno,
};
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::{Read, Write},
    os::unix::fs::FileExt,
    os::unix::fs::MetadataExt,
    path::{Component, Path},
};

const RESOLVE: ResolveFlags = ResolveFlags::BENEATH
    .union(ResolveFlags::NO_SYMLINKS)
    .union(ResolveFlags::NO_MAGICLINKS)
    .union(ResolveFlags::NO_XDEV);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FaultPoint {
    FileSync,
    StagingDirectorySync,
    BeforeRunRename,
    AfterRunRename,
    RunStagingParentSync,
    RunParentSync,
    RecoveryStagingParentSync,
    RecoveryDestinationSync,
    BeforeRevisionRename,
    AfterRevisionRename,
    RevisionDirectorySync,
    ReplayRevisionDirectorySync,
}
#[cfg(test)]
thread_local! {
    static TEST_FAULT: std::cell::Cell<Option<FaultPoint>> = const { std::cell::Cell::new(None) };
    static RECOVERY_BOUNDARY: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
fn recovery_boundary() {
    RECOVERY_BOUNDARY.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}
#[cfg(test)]
fn install_fault(point: FaultPoint) {
    TEST_FAULT.with(|fault| fault.set(Some(point)));
}
#[cfg(test)]
fn maybe_fault(point: FaultPoint) -> Result<(), ArchyError> {
    if TEST_FAULT.with(|fault| {
        if fault.get() == Some(point) {
            fault.set(None);
            true
        } else {
            false
        }
    }) {
        Err(ioerr(
            &format!("injected {point:?}"),
            std::io::Error::from_raw_os_error(5),
        ))
    } else {
        Ok(())
    }
}
#[cfg(not(test))]
fn maybe_fault(_point: FaultPoint) -> Result<(), ArchyError> {
    Ok(())
}

fn err(code: ErrorCode, message: impl Into<String>) -> ArchyError {
    ArchyError::new(code, "storage", message)
}
fn invalid(message: impl Into<String>) -> ArchyError {
    err(ErrorCode::ArtifactInvalid, message)
}
fn conflict(message: impl Into<String>) -> ArchyError {
    err(ErrorCode::RevisionConflict, message)
}
fn ioerr(context: &str, e: impl std::fmt::Display) -> ArchyError {
    err(ErrorCode::InternalError, format!("{context}: {e}"))
}
fn patherr(context: &str, e: Errno) -> ArchyError {
    if matches!(e, Errno::LOOP | Errno::XDEV | Errno::NOTDIR) {
        err(
            ErrorCode::PermissionDenied,
            format!("{context}: unsafe or occupied path: {e}"),
        )
    } else {
        ioerr(context, e)
    }
}
fn uncertain(context: &str, e: impl std::fmt::Display) -> ArchyError {
    let mut error = ioerr(context, e);
    error
        .message
        .push_str("; commit may have occurred; retry the same idempotency key");
    error.retriable = true;
    error
}
fn name_path(path: &str) -> Result<Vec<&str>, ArchyError> {
    if path.is_empty() || path.len() > 512 || path.contains('\\') || path.contains('\0') {
        return Err(err(ErrorCode::PermissionDenied, "invalid relative path"));
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() > 16
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == ".." || p.len() > 255)
    {
        return Err(err(
            ErrorCode::PermissionDenied,
            "invalid relative path component",
        ));
    }
    if Path::new(path)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(err(ErrorCode::PermissionDenied, "path must be relative"));
    }
    Ok(parts)
}
fn checked_file(file: &File) -> Result<(), ArchyError> {
    let meta = file.metadata().map_err(|e| ioerr("file metadata", e))?;
    if !meta.is_file() || meta.nlink() != 1 {
        return Err(invalid("managed file is not a unique regular inode"));
    }
    Ok(())
}
fn open_dir(parent: &File, name: &str) -> Result<File, ArchyError> {
    let fd = fs::openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )
    .map_err(|e| patherr("open directory", e))?;
    let directory = File::from(fd);
    let meta = directory
        .metadata()
        .map_err(|e| ioerr("directory metadata", e))?;
    let owner = parent
        .metadata()
        .map_err(|e| ioerr("parent metadata", e))?
        .uid();
    if meta.uid() != owner || meta.mode() & 0o077 != 0 {
        return Err(err(
            ErrorCode::PermissionDenied,
            "managed directory must be private and owned by project owner",
        ));
    }
    Ok(directory)
}
fn ensure_dir(parent: &File, name: &str) -> Result<File, ArchyError> {
    match fs::mkdirat(parent, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
        Ok(()) => parent
            .sync_all()
            .map_err(|e| ioerr("sync created directory entry", e))?,
        Err(Errno::EXIST) => {}
        Err(e) => return Err(patherr("create directory", e)),
    }
    open_dir(parent, name)
}
fn existing_file(parent: &File, name: &str) -> Result<File, ArchyError> {
    let fd = fs::openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )
    .map_err(|e| patherr("open managed file", e))?;
    let file = File::from(fd);
    checked_file(&file)?;
    Ok(file)
}
fn create_file(parent: &File, name: &str) -> Result<File, ArchyError> {
    let fd = fs::openat2(
        parent,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
        RESOLVE,
    )
    .map_err(|e| {
        if e == Errno::EXIST {
            err(ErrorCode::PermissionDenied, "target already exists")
        } else {
            patherr("create file", e)
        }
    })?;
    let file = File::from(fd);
    checked_file(&file)?;
    Ok(file)
}
fn read_bounded(file: &mut File, limit: usize) -> Result<Vec<u8>, ArchyError> {
    let size = file
        .metadata()
        .map_err(|e| ioerr("file metadata", e))?
        .len();
    if size > limit as u64 {
        return Err(invalid("managed file exceeds size limit"));
    }
    let mut data = Vec::with_capacity(size as usize);
    file.take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| ioerr("read file", e))?;
    if data.len() > limit {
        return Err(invalid("managed file exceeds size limit"));
    }
    Ok(data)
}
fn write_synced(parent: &File, name: &str, bytes: &[u8]) -> Result<(), ArchyError> {
    let mut file = create_file(parent, name)?;
    file.write_all(bytes).map_err(|e| ioerr("write file", e))?;
    maybe_fault(FaultPoint::FileSync)?;
    file.sync_all().map_err(|e| ioerr("sync file", e))
}
fn dir_entries(dir: &File, max_entries: usize) -> Result<Vec<String>, ArchyError> {
    let mut cursor = fs::Dir::read_from(dir).map_err(|e| ioerr("read directory", e))?;
    let mut names = Vec::new();
    while let Some(entry) = cursor.read() {
        let entry = entry.map_err(|e| ioerr("read directory entry", e))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| invalid("non-UTF8 managed entry"))?;
        if name != "." && name != ".." {
            names.push(name.to_owned());
        }
        if names.len() > max_entries {
            return Err(invalid("too many managed entries"));
        }
    }
    Ok(names)
}
fn open_path(root: &File, path: &str) -> Result<File, ArchyError> {
    let parts = name_path(path)?;
    let mut dir = root.try_clone().map_err(|e| ioerr("clone root", e))?;
    for part in &parts[..parts.len() - 1] {
        dir = open_dir(&dir, part)?;
    }
    existing_file(&dir, parts[parts.len() - 1])
}
pub fn digest_request(request: &RunRequest) -> Result<String, ArchyError> {
    let mut bytes = b"archy-run-request-fingerprint-v1\0".to_vec();
    serde_json::to_writer(&mut bytes, request).map_err(|e| ioerr("serialize request", e))?;
    Ok(sha256_hex(&bytes))
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
fn valid_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}

pub struct ProjectStore {
    root: File,
    project_file: String,
}
pub struct PinnedProjectSource {
    file: File,
    device: u64,
    inode: u64,
}
impl PinnedProjectSource {
    pub fn read(&self) -> Result<Vec<u8>, ArchyError> {
        let limit = crate::validation::MAX_INPUT_BYTES;
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 8192];
        while bytes.len() <= limit {
            let count = self
                .file
                .read_at(&mut chunk, bytes.len() as u64)
                .map_err(|e| ioerr("read pinned project source", e))?;
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
        if bytes.len() > limit {
            return Err(err(ErrorCode::InvalidSchema, "project input exceeds 1 MiB"));
        }
        Ok(bytes)
    }
}
pub struct LockedProject<'a> {
    store: &'a ProjectStore,
    _lock: File,
    owner: Option<(u32, u64, u64)>,
}
fn lock_process_identity() -> Option<(u32, u64, u64)> {
    let namespace = std::fs::metadata("/proc/thread-self/ns/pid").ok()?;
    Some((std::process::id(), namespace.dev(), namespace.ino()))
}
impl Drop for LockedProject<'_> {
    fn drop(&mut self) {
        // PID namespaces can assign the same PID to a parent and child.
        // Without a verified identity, closing the descriptor preserves exclusion.
        if self.owner.is_some() && self.owner == lock_process_identity() {
            let _ = self._lock.unlock();
        }
    }
}
/// A validated revision prefix. Each run read rechecks manifest and artifact bytes.
pub struct AcceptedHistory<'store> {
    store: &'store ProjectStore,
    records: Vec<RevisionRecord>,
    head_sha256: Option<String>,
}
impl AcceptedHistory<'_> {
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub fn revision(&self) -> u64 {
        self.records.last().map_or(0, |r| r.revision)
    }
    pub fn head_sha256(&self) -> Option<&str> {
        self.head_sha256.as_deref()
    }
    pub fn receipts(&self) -> impl DoubleEndedIterator<Item = &RunReceipt> {
        self.records.iter().map(|r| &r.receipt)
    }
    pub fn previous_head_for_run(&self, run_id: &str) -> Option<&str> {
        self.records
            .iter()
            .find(|r| r.receipt.run_id == run_id)?
            .previous_sha256
            .as_deref()
    }
    pub fn revision_for_run(&self, run_id: &str) -> Option<u64> {
        Some(
            self.records
                .iter()
                .find(|r| r.receipt.run_id == run_id)?
                .revision,
        )
    }
    pub fn read_run(&self, run_id: &str) -> Result<VerifiedRun, ArchyError> {
        let record = self
            .records
            .iter()
            .find(|r| r.receipt.run_id == run_id)
            .ok_or_else(|| invalid("run is not accepted"))?;
        read_record_run(&self.store.root, record)
    }
}
type OrphanValidator<'a> = dyn Fn(&AcceptedHistory<'_>, &str) -> Result<(), ArchyError> + 'a;

#[derive(Default)]
struct RecoveryChecks<'a> {
    binding: Option<&'a dyn Fn() -> Result<(), ArchyError>>,
    orphan: Option<&'a OrphanValidator<'a>>,
}

pub enum BeginResult<'a> {
    Replay(RunReceipt),
    Pending(Box<PendingRun<'a>>),
}
impl<'a> BeginResult<'a> {
    pub fn receipt(self) -> Option<RunReceipt> {
        if let Self::Replay(r) = self {
            Some(r)
        } else {
            None
        }
    }
    pub fn pending(self) -> Option<Box<PendingRun<'a>>> {
        if let Self::Pending(p) = self {
            Some(p)
        } else {
            None
        }
    }
}
pub struct PendingRun<'a> {
    locked: &'a LockedProject<'a>,
    request: RunRequest,
    request_sha256: String,
    run_id: String,
    staging: File,
    staging_parent: File,
    directories: HashSet<String>,
    artifacts: Vec<Artifact>,
    outcomes: Vec<VariantOutcome>,
    previous_sha256: Option<String>,
    revision: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Interrupt {
    None,
    AfterRunPublication,
    AfterRevisionPublication,
}
fn publish_revision(
    root: &File,
    record: &RevisionRecord,
    interrupt: Interrupt,
) -> Result<(), ArchyError> {
    let bytes = serde_json::to_vec(record).map_err(|e| uncertain("serialize revision", e))?;
    if bytes.len() > MAX_REVISION_BYTES {
        return Err(uncertain("revision too large", bytes.len()));
    }
    let archy = open_dir(root, ".archy")?;
    let revisions = open_dir(&archy, "revisions")?;
    let temp = format!(".tmp-{}", uuid::Uuid::new_v4());
    write_synced(&revisions, &temp, &bytes).map_err(|e| uncertain("write revision", e))?;
    revisions
        .sync_all()
        .map_err(|e| uncertain("sync temporary revision directory", e))?;
    let final_name = format!("{:020}.json", record.revision);
    maybe_fault(FaultPoint::BeforeRevisionRename).map_err(|e| uncertain("publish revision", e))?;
    fs::renameat_with(
        &revisions,
        temp.as_str(),
        &revisions,
        final_name.as_str(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|e| uncertain("publish revision", e))?;
    maybe_fault(FaultPoint::AfterRevisionRename).map_err(|e| uncertain("publish revision", e))?;
    maybe_fault(FaultPoint::RevisionDirectorySync)
        .map_err(|e| uncertain("sync committed revision", e))?;
    revisions
        .sync_all()
        .map_err(|e| uncertain("sync committed revision", e))?;
    if interrupt == Interrupt::AfterRevisionPublication {
        return Err(uncertain(
            "injected interruption after revision publication",
            "test",
        ));
    }
    Ok(())
}
/// Hash-verified bytes and structural metadata only. Evaluation claims require
/// canonical validation and recomputation by the caller.
#[derive(Debug)]
pub struct VerifiedRun {
    manifest: RunManifest,
    artifacts: BTreeMap<String, Vec<u8>>,
}
impl VerifiedRun {
    pub fn manifest(&self) -> &RunManifest {
        &self.manifest
    }
    pub fn artifact(&self, path: &str) -> Option<&[u8]> {
        self.artifacts.get(path).map(Vec::as_slice)
    }
    pub fn artifacts(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.artifacts
    }
}

impl ProjectStore {
    pub fn open(project: &Path) -> Result<Self, ArchyError> {
        let parent = project
            .parent()
            .ok_or_else(|| err(ErrorCode::PermissionDenied, "project has no parent"))?;
        let basename = project
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| err(ErrorCode::PermissionDenied, "invalid project file"))?;
        name_path(basename)?;
        let root = File::open(parent).map_err(|e| ioerr("open project root", e))?;
        if !root
            .metadata()
            .map_err(|e| ioerr("root metadata", e))?
            .is_dir()
        {
            return Err(err(
                ErrorCode::PermissionDenied,
                "project root is not a directory",
            ));
        }
        let source = existing_file(&root, basename)?;
        if source
            .metadata()
            .map_err(|e| ioerr("project metadata", e))?
            .len()
            > crate::validation::MAX_INPUT_BYTES as u64
        {
            return Err(err(ErrorCode::InvalidSchema, "project input exceeds 1 MiB"));
        }
        Ok(Self {
            root,
            project_file: basename.to_owned(),
        })
    }
    pub fn lock(&self) -> Result<LockedProject<'_>, ArchyError> {
        let archy = ensure_dir(&self.root, ".archy")?;
        let _ = ensure_dir(&archy, "staging")?;
        let _ = ensure_dir(&archy, "revisions")?;
        let lock = match create_file(&archy, "lock") {
            Ok(f) => {
                archy
                    .sync_all()
                    .map_err(|e| ioerr("sync lock directory", e))?;
                f
            }
            Err(e) if e.code == ErrorCode::PermissionDenied => existing_file(&archy, "lock")?,
            Err(e) => return Err(e),
        };
        match lock.try_lock() {
            Ok(()) => Ok(LockedProject {
                store: self,
                _lock: lock,
                owner: lock_process_identity(),
            }),
            Err(std::fs::TryLockError::WouldBlock) => {
                let mut c = conflict("project lock held by another process");
                c.retriable = true;
                Err(c)
            }
            Err(e) => Err(ioerr("lock project", e)),
        }
    }
    /// Read a validated accepted prefix without locking, creating, or recovering state.
    pub fn read_history(&self) -> Result<AcceptedHistory<'_>, ArchyError> {
        let (records, head_sha256) = match revisions_from(&self.root, true, None)? {
            Some((records, head, _)) => (records, head),
            None => (Vec::new(), None),
        };
        Ok(AcceptedHistory {
            store: self,
            records,
            head_sha256,
        })
    }
    pub fn read_source(&self) -> Result<Vec<u8>, ArchyError> {
        let mut file = existing_file(&self.root, &self.project_file)?;
        read_bounded(&mut file, crate::validation::MAX_INPUT_BYTES)
    }
    pub fn pin_source(&self) -> Result<PinnedProjectSource, ArchyError> {
        let file = existing_file(&self.root, &self.project_file)?;
        let meta = file.metadata().map_err(|e| ioerr("project metadata", e))?;
        Ok(PinnedProjectSource {
            file,
            device: meta.dev(),
            inode: meta.ino(),
        })
    }
    pub fn source_matches(&self, pinned: &PinnedProjectSource) -> Result<bool, ArchyError> {
        let current = existing_file(&self.root, &self.project_file)?;
        let meta = current
            .metadata()
            .map_err(|e| ioerr("project metadata", e))?;
        Ok(meta.dev() == pinned.device && meta.ino() == pinned.inode)
    }
    pub fn identity_digest(&self) -> Result<String, ArchyError> {
        let meta = self
            .root
            .metadata()
            .map_err(|e| ioerr("root metadata", e))?;
        Ok(sha256_hex(
            format!(
                "archy-project-root-v1\0{}:{}:{}",
                meta.dev(),
                meta.ino(),
                self.project_file
            )
            .as_bytes(),
        ))
    }
    pub fn matches_root_path(&self, path: &Path) -> Result<bool, ArchyError> {
        let pinned = self
            .root
            .metadata()
            .map_err(|e| ioerr("root metadata", e))?;
        let current = std::fs::metadata(path).map_err(|e| ioerr("configured root metadata", e))?;
        Ok(pinned.dev() == current.dev() && pinned.ino() == current.ino())
    }
}

type RevisionSnapshot = (Vec<RevisionRecord>, Option<String>, File);

fn revisions_from(
    root: &File,
    absent_ok: bool,
    limits: Option<(usize, u64)>,
) -> Result<Option<RevisionSnapshot>, ArchyError> {
    let archy = match open_dir(root, ".archy") {
        Ok(dir) => dir,
        Err(_)
            if absent_ok
                && fs::statat(root, ".archy", fs::AtFlags::SYMLINK_NOFOLLOW)
                    .is_err_and(|x| x == Errno::NOENT) =>
        {
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    let dir = open_dir(&archy, "revisions").map_err(|e| {
        if fs::statat(&archy, "revisions", fs::AtFlags::SYMLINK_NOFOLLOW)
            .is_err_and(|x| x == Errno::NOENT)
        {
            invalid("managed history is missing its revisions directory")
        } else {
            e
        }
    })?;
    let mut numbered = Vec::new();
    for name in dir_entries(
        &dir,
        limits.map_or(100_000, |(count, _)| count.saturating_add(32)),
    )? {
        if name.starts_with(".tmp-") {
            continue;
        }
        if name.len() != 25
            || !name.ends_with(".json")
            || !name[..20].bytes().all(|b| b.is_ascii_digit())
        {
            return Err(invalid("unexpected revision entry"));
        }
        numbered.push(name);
    }
    if let Some((max_revisions, _)) = limits
        && numbered.len() > max_revisions
    {
        return Err(invalid("review history revision limit exceeded"));
    }
    numbered.sort();
    let mut records = Vec::new();
    let mut prev = None;
    let mut seen_keys = HashSet::new();
    let mut seen_runs = HashSet::new();
    let mut remaining = limits.map_or(u64::MAX, |(_, bytes)| bytes);
    for (index, name) in numbered.iter().enumerate() {
        let n = u64::try_from(index + 1).map_err(|_| invalid("revision overflow"))?;
        if *name != format!("{n:020}.json") {
            return Err(invalid("revision gap or duplicate"));
        }
        let mut file = existing_file(&dir, name).map_err(|e| invalid(e.message))?;
        let bytes = read_bounded(&mut file, MAX_REVISION_BYTES)?;
        remaining = remaining
            .checked_sub(bytes.len() as u64)
            .ok_or_else(|| invalid("review history byte limit exceeded"))?;
        let record: RevisionRecord = serde_json::from_slice(&bytes)
            .map_err(|e| invalid(format!("invalid revision JSON: {e}")))?;
        if record.schema_version != REVISION_SCHEMA_VERSION {
            return Err(invalid("unsupported revision schema version"));
        }
        if record.request_schema_version != RUN_REQUEST_SCHEMA_VERSION {
            return Err(invalid("unsupported run request schema version"));
        }
        if record.revision != n
            || record.previous_sha256 != prev
            || record.receipt.revision != n
            || !valid_key(&record.idempotency_key)
            || !valid_hash(&record.request_sha256)
            || !seen_keys.insert(record.idempotency_key.clone())
            || !seen_runs.insert(record.receipt.run_id.clone())
        {
            return Err(invalid("invalid revision chain"));
        }
        read_record_run_budget(root, &record, &mut remaining, limits.is_some())?;
        prev = Some(sha256_hex(&bytes));
        records.push(record);
    }
    Ok(Some((records, prev, dir)))
}
fn read_record_run(root: &File, record: &RevisionRecord) -> Result<VerifiedRun, ArchyError> {
    let mut unlimited = u64::MAX;
    read_record_run_budget(root, record, &mut unlimited, false)
}
fn read_record_run_budget(
    root: &File,
    record: &RevisionRecord,
    remaining: &mut u64,
    review_limits: bool,
) -> Result<VerifiedRun, ArchyError> {
    let receipt = &record.receipt;
    let parts = name_path(&receipt.path).map_err(|e| invalid(e.message))?;
    if parts.last() != Some(&receipt.run_id.as_str())
        || uuid::Uuid::parse_str(&receipt.run_id).is_err()
        || !valid_hash(&receipt.manifest_sha256)
    {
        return Err(invalid("invalid run receipt"));
    }
    let mut dir = root.try_clone().map_err(|e| ioerr("clone root", e))?;
    for part in parts {
        dir = open_dir(&dir, part).map_err(|e| invalid(e.message))?;
    }
    let mut manifest_file = existing_file(&dir, "manifest.json").map_err(|e| invalid(e.message))?;
    let manifest_bytes = read_bounded(&mut manifest_file, MAX_MANIFEST_BYTES)?;
    *remaining = remaining
        .checked_sub(manifest_bytes.len() as u64)
        .ok_or_else(|| invalid("review history byte limit exceeded"))?;
    if sha256_hex(&manifest_bytes) != receipt.manifest_sha256 {
        return Err(invalid("manifest hash mismatch"));
    }
    let manifest: RunManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| invalid(format!("invalid manifest JSON: {e}")))?;
    if manifest.schema_version != RUN_SCHEMA_VERSION {
        return Err(invalid("unsupported run schema version"));
    }
    if manifest.run_id != receipt.run_id
        || manifest.idempotency_key != record.idempotency_key
        || manifest.request_sha256 != record.request_sha256
        || manifest.request_schema_version != record.request_schema_version
        || manifest.expected_project_revision.checked_add(1) != Some(record.revision)
        || manifest.artifacts.len() > MAX_ARTIFACTS
        || manifest.artifacts.is_empty()
    {
        return Err(invalid("invalid manifest identity or inventory"));
    }
    if review_limits
        && (manifest.outcomes.len() > 32
            || manifest.count.is_some_and(|n| n > 32)
            || manifest.artifacts.len() > 256)
    {
        return Err(invalid("review run work limit exceeded"));
    }
    let mut inventory = BTreeMap::new();
    let mut aggregate_bytes = 0_u64;
    for artifact in &manifest.artifacts {
        let path = name_path(&artifact.path).map_err(|e| invalid(e.message))?;
        if path[0] == ".archy"
            || artifact.path == "manifest.json"
            || !valid_hash(&artifact.sha256)
            || artifact.byte_len > MAX_ARTIFACT_BYTES as u64
            || artifact.media_type.is_empty()
            || artifact.producer.is_empty()
            || inventory.contains_key(&artifact.path)
        {
            return Err(invalid("invalid artifact inventory"));
        }
        aggregate_bytes = aggregate_bytes
            .checked_add(artifact.byte_len)
            .ok_or_else(|| invalid("run byte count overflow"))?;
        *remaining = remaining
            .checked_sub(artifact.byte_len)
            .ok_or_else(|| invalid("review history byte limit exceeded"))?;
        if aggregate_bytes > MAX_RUN_BYTES as u64 {
            return Err(invalid("run exceeds aggregate size limit"));
        }
        let mut file = open_path(&dir, &artifact.path).map_err(|e| invalid(e.message))?;
        if file
            .metadata()
            .map_err(|e| ioerr("artifact metadata", e))?
            .len()
            != artifact.byte_len
        {
            return Err(invalid("artifact declared length differs from file"));
        }
        let bytes = read_bounded(&mut file, MAX_ARTIFACT_BYTES)?;
        if bytes.len() as u64 != artifact.byte_len || sha256_hex(&bytes) != artifact.sha256 {
            return Err(invalid("artifact hash or length mismatch"));
        }
        inventory.insert(artifact.path.clone(), bytes);
    }
    for name in ["input.json", "input.yaml", "normalized.json"] {
        if let Some(bytes) = inventory.get(name) {
            let expected = if name == "normalized.json" {
                manifest.normalized_sha256.as_ref()
            } else {
                Some(&manifest.input_sha256)
            };
            if expected != Some(&sha256_hex(bytes)) {
                return Err(invalid("input or normalized digest mismatch"));
            }
        }
    }
    let mut outcome_ids = HashSet::new();
    if manifest.outcomes.len() > 1000 {
        return Err(invalid("too many variant outcomes"));
    }
    for outcome in &manifest.outcomes {
        if outcome.variant_id.is_empty()
            || outcome.variant_id.len() > 128
            || !outcome_ids.insert(&outcome.variant_id)
            || outcome.evaluation_artifact.is_some() == outcome.error_artifact.is_some()
        {
            return Err(invalid("invalid variant outcome"));
        }
        for path in [
            &outcome.variant_artifact,
            &outcome.evaluation_artifact,
            &outcome.error_artifact,
        ]
        .into_iter()
        .flatten()
        {
            if !inventory.contains_key(path) {
                return Err(invalid("outcome references absent artifact"));
            }
        }
    }
    Ok(VerifiedRun {
        manifest,
        artifacts: inventory,
    })
}

impl LockedProject<'_> {
    fn require_owner(&self) -> Result<(), ArchyError> {
        if self.owner.is_some() && self.owner != lock_process_identity() {
            return Err(err(
                ErrorCode::PermissionDenied,
                "project lock belongs to another process or its identity cannot be verified",
            ));
        }
        Ok(())
    }

    fn recover_orphan(
        &self,
        request: &RunRequest,
        digest: &str,
        records: &[RevisionRecord],
        previous_sha256: Option<String>,
        next: u64,
        checks: RecoveryChecks<'_>,
    ) -> Result<Option<RunReceipt>, ArchyError> {
        let mut dest = self
            .store
            .root
            .try_clone()
            .map_err(|e| ioerr("clone root", e))?;
        for part in name_path(&request.context.destination)? {
            let fd = match fs::openat2(
                &dest,
                part,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
                RESOLVE,
            ) {
                Ok(fd) => fd,
                Err(Errno::NOENT) => return Ok(None),
                Err(e) => return Err(patherr("open destination for recovery", e)),
            };
            dest = File::from(fd);
        }
        for name in dir_entries(&dest, 100_000)? {
            if uuid::Uuid::parse_str(&name).is_err()
                || records.iter().any(|r| r.receipt.run_id == name)
            {
                continue;
            }
            let run = open_dir(&dest, &name).map_err(|e| invalid(e.message))?;
            let manifest_fd = match fs::openat2(
                &run,
                "manifest.json",
                OFlags::RDONLY | OFlags::CLOEXEC,
                Mode::empty(),
                RESOLVE,
            ) {
                Ok(fd) => fd,
                Err(Errno::NOENT) => continue,
                Err(e) => return Err(invalid(format!("unsafe orphan manifest: {e}"))),
            };
            let mut manifest_file = File::from(manifest_fd);
            checked_file(&manifest_file)?;
            let bytes = read_bounded(&mut manifest_file, MAX_MANIFEST_BYTES)?;
            let manifest: RunManifest = serde_json::from_slice(&bytes)
                .map_err(|e| invalid(format!("invalid orphan manifest: {e}")))?;
            if manifest.idempotency_key != request.context.idempotency_key {
                continue;
            }
            if manifest.request_sha256 != digest
                || manifest.expected_project_revision != request.context.expected_project_revision
            {
                return Err(conflict(
                    "orphan idempotency key reused with different request",
                ));
            }
            let receipt = RunReceipt {
                run_id: name,
                revision: next,
                manifest_sha256: sha256_hex(&bytes),
                path: format!("{}/{}", request.context.destination, manifest.run_id),
            };
            let record = RevisionRecord {
                schema_version: REVISION_SCHEMA_VERSION.into(),
                revision: next,
                previous_sha256,
                idempotency_key: request.context.idempotency_key.clone(),
                request_sha256: digest.into(),
                request_schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
                receipt: receipt.clone(),
            };
            self.read_record_run(&record)?;
            if let Some(validate) = checks.orphan {
                let mut candidate_records = records.to_vec();
                candidate_records.push(record.clone());
                let candidate = AcceptedHistory {
                    store: self.store,
                    records: candidate_records,
                    head_sha256: None,
                };
                validate(&candidate, &receipt.run_id)?;
            }
            let archy = open_dir(&self.store.root, ".archy")?;
            let staging_parent = open_dir(&archy, "staging")?;
            maybe_fault(FaultPoint::RecoveryStagingParentSync)
                .map_err(|e| uncertain("sync orphan staging parent", e))?;
            staging_parent
                .sync_all()
                .map_err(|e| uncertain("sync orphan staging parent", e))?;
            maybe_fault(FaultPoint::RecoveryDestinationSync)
                .map_err(|e| uncertain("sync orphan destination", e))?;
            dest.sync_all()
                .map_err(|e| uncertain("sync orphan destination", e))?;
            #[cfg(test)]
            recovery_boundary();
            if let Some(check) = checks.binding {
                check().map_err(|error| {
                    uncertain("binding changed before orphan revision publication", error)
                })?;
            }
            if checks.orphan.is_some() {
                self.read_record_run(&record)?;
            }
            publish_revision(&self.store.root, &record, Interrupt::None)?;
            return Ok(Some(receipt));
        }
        Ok(None)
    }
    fn revisions(&self) -> Result<(Vec<RevisionRecord>, Option<String>, File), ArchyError> {
        self.require_owner()?;
        revisions_from(&self.store.root, false, None)?
            .ok_or_else(|| invalid("missing managed history"))
    }
    fn read_record_run(&self, record: &RevisionRecord) -> Result<VerifiedRun, ArchyError> {
        read_record_run(&self.store.root, record)
    }
    pub fn revision(&self) -> Result<u64, ArchyError> {
        Ok(self.revisions()?.0.last().map_or(0, |r| r.revision))
    }
    /// Accepted receipts in revision order; the full chain is checked first.
    pub fn receipts(&self) -> Result<Vec<RunReceipt>, ArchyError> {
        Ok(self.revisions()?.0.into_iter().map(|r| r.receipt).collect())
    }
    /// Validate the full accepted chain once, then read selected runs against it.
    pub fn accepted_history(&self) -> Result<AcceptedHistory<'_>, ArchyError> {
        let (records, head_sha256, _) = self.revisions()?;
        Ok(AcceptedHistory {
            store: self.store,
            records,
            head_sha256,
        })
    }
    /// Fail before reading run artifacts when the accepted prefix exceeds a caller's review budget.
    pub fn accepted_history_bounded(
        &self,
        max_revisions: usize,
        max_bytes: u64,
    ) -> Result<AcceptedHistory<'_>, ArchyError> {
        self.require_owner()?;
        let (records, head_sha256, _) =
            revisions_from(&self.store.root, false, Some((max_revisions, max_bytes)))?
                .ok_or_else(|| invalid("missing managed history"))?;
        Ok(AcceptedHistory {
            store: self.store,
            records,
            head_sha256,
        })
    }
    pub fn read_run(&self, run_id: &str) -> Result<VerifiedRun, ArchyError> {
        let (records, _, _) = self.revisions()?;
        let record = records
            .iter()
            .find(|r| r.receipt.run_id == run_id)
            .ok_or_else(|| invalid("run is not accepted"))?;
        self.read_record_run(record)
    }
    pub fn read_source(&self) -> Result<Vec<u8>, ArchyError> {
        self.require_owner()?;
        let mut file = existing_file(&self.store.root, &self.store.project_file)?;
        read_bounded(&mut file, crate::validation::MAX_INPUT_BYTES)
    }
    /// Read bounded project-relative bytes through the pinned root descriptor.
    pub fn read_project_file(&self, path: &str, limit: usize) -> Result<Vec<u8>, ArchyError> {
        self.require_owner()?;
        let components = name_path(path)?;
        if components[0] == ".archy" || limit > crate::validation::MAX_INPUT_BYTES {
            return Err(err(
                ErrorCode::PermissionDenied,
                "reserved or excessive project file read",
            ));
        }
        let mut file = open_path(&self.store.root, path)?;
        read_bounded(&mut file, limit)
    }
    pub fn begin<'a>(&'a self, request: RunRequest) -> Result<BeginResult<'a>, ArchyError> {
        self.begin_inner(request, RecoveryChecks::default())
    }
    /// Checks an external binding before publishing a recovered orphan revision.
    /// A concurrent rename between this check and publication remains possible.
    pub fn begin_checked<'a>(
        &'a self,
        request: RunRequest,
        check: &dyn Fn() -> Result<(), ArchyError>,
    ) -> Result<BeginResult<'a>, ArchyError> {
        self.begin_inner(
            request,
            RecoveryChecks {
                binding: Some(check),
                orphan: None,
            },
        )
    }
    /// Validate a hash-checked orphan as the next run before publishing its revision.
    /// The candidate history is available only during this call under the project lock.
    pub fn begin_checked_recovery<'a>(
        &'a self,
        request: RunRequest,
        check: &dyn Fn() -> Result<(), ArchyError>,
        validate: &dyn Fn(&AcceptedHistory<'_>, &str) -> Result<(), ArchyError>,
    ) -> Result<BeginResult<'a>, ArchyError> {
        self.begin_inner(
            request,
            RecoveryChecks {
                binding: Some(check),
                orphan: Some(validate),
            },
        )
    }
    fn begin_inner<'a>(
        &'a self,
        request: RunRequest,
        checks: RecoveryChecks<'_>,
    ) -> Result<BeginResult<'a>, ArchyError> {
        self.require_owner()?;
        if request.schema_version != RUN_REQUEST_SCHEMA_VERSION {
            return Err(err(
                ErrorCode::InvalidSchema,
                "unsupported run request schema version",
            ));
        }
        if !valid_key(&request.context.idempotency_key)
            || request.request_bytes.len() > crate::validation::MAX_INPUT_BYTES
            || !valid_hash(&request.input_sha256)
            || request.operation.is_empty()
            || request.operation.len() > 64
            || request
                .source_run
                .as_ref()
                .is_some_and(|s| s.len() != 36 || uuid::Uuid::parse_str(s).is_err())
        {
            return Err(err(ErrorCode::InvalidSchema, "invalid bounded run request"));
        }
        let destination = name_path(&request.context.destination)?;
        if destination[0] == ".archy"
            || destination[0] == self.store.project_file
            || destination[0] == "."
        {
            return Err(err(ErrorCode::PermissionDenied, "reserved destination"));
        }
        let digest = digest_request(&request)?;
        let (records, previous_sha256, revisions_dir) = self.revisions()?;
        if let Some(record) = records
            .iter()
            .find(|r| r.idempotency_key == request.context.idempotency_key)
        {
            if record.request_sha256 == digest {
                maybe_fault(FaultPoint::ReplayRevisionDirectorySync)
                    .map_err(|e| uncertain("sync replayed revision directory", e))?;
                revisions_dir
                    .sync_all()
                    .map_err(|e| uncertain("sync replayed revision directory", e))?;
                return Ok(BeginResult::Replay(record.receipt.clone()));
            }
            return Err(conflict("idempotency key reused with different request"));
        }
        let current = records.last().map_or(0, |r| r.revision);
        if request.context.expected_project_revision != current {
            return Err(conflict("stale project revision"));
        }
        let next = current
            .checked_add(1)
            .ok_or_else(|| conflict("revision overflow"))?;
        if let Some(receipt) = self.recover_orphan(
            &request,
            &digest,
            &records,
            previous_sha256.clone(),
            next,
            checks,
        )? {
            return Ok(BeginResult::Replay(receipt));
        }
        let archy = open_dir(&self.store.root, ".archy")?;
        let staging_parent = open_dir(&archy, "staging")?;
        let run_id = uuid::Uuid::new_v4().to_string();
        fs::mkdirat(
            &staging_parent,
            run_id.as_str(),
            Mode::RUSR | Mode::WUSR | Mode::XUSR,
        )
        .map_err(|e| ioerr("create staging run", e))?;
        let staging = open_dir(&staging_parent, &run_id)?;
        Ok(BeginResult::Pending(Box::new(PendingRun {
            locked: self,
            request,
            request_sha256: digest,
            run_id,
            staging,
            staging_parent,
            directories: HashSet::new(),
            artifacts: Vec::new(),
            outcomes: Vec::new(),
            previous_sha256,
            revision: next,
        })))
    }
}

impl PendingRun<'_> {
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
    pub fn write(
        &mut self,
        path: &str,
        bytes: &[u8],
        media_type: &str,
        producer: &str,
    ) -> Result<(), ArchyError> {
        self.locked.require_owner()?;
        let parts = name_path(path)?;
        if path == "manifest.json"
            || parts[0] == ".archy"
            || bytes.len() > MAX_ARTIFACT_BYTES
            || self.artifacts.len() >= MAX_ARTIFACTS
            || self.artifacts.iter().map(|a| a.byte_len).sum::<u64>() + bytes.len() as u64
                > MAX_RUN_BYTES as u64
            || media_type.is_empty()
            || media_type.len() > 128
            || producer.is_empty()
            || producer.len() > 128
        {
            return Err(err(ErrorCode::InvalidSchema, "invalid artifact"));
        }
        let mut dir = self
            .staging
            .try_clone()
            .map_err(|e| ioerr("clone staging", e))?;
        let mut prefix = String::new();
        for part in &parts[..parts.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            dir = ensure_dir(&dir, part)?;
            self.directories.insert(prefix.clone());
        }
        write_synced(&dir, parts[parts.len() - 1], bytes)?;
        self.artifacts.push(Artifact {
            path: path.into(),
            sha256: sha256_hex(bytes),
            media_type: media_type.into(),
            producer: producer.into(),
            byte_len: bytes.len() as u64,
        });
        Ok(())
    }
    pub fn record_outcome(&mut self, outcome: VariantOutcome) -> Result<(), ArchyError> {
        self.locked.require_owner()?;
        if outcome.variant_id.is_empty()
            || outcome.variant_id.len() > 128
            || self.outcomes.len() >= 1000
            || self
                .outcomes
                .iter()
                .any(|o| o.variant_id == outcome.variant_id)
            || outcome.evaluation_artifact.is_some() == outcome.error_artifact.is_some()
        {
            return Err(err(ErrorCode::InvalidSchema, "invalid variant outcome"));
        }
        for path in [
            &outcome.variant_artifact,
            &outcome.evaluation_artifact,
            &outcome.error_artifact,
        ]
        .into_iter()
        .flatten()
        {
            if !self.artifacts.iter().any(|a| a.path == *path) {
                return Err(err(
                    ErrorCode::InvalidSchema,
                    "outcome references absent artifact",
                ));
            }
        }
        self.outcomes.push(outcome);
        Ok(())
    }
    /// Discard a staged run before any artifacts are written, for a failed worker attempt.
    pub fn discard_empty(self) -> Result<(), ArchyError> {
        self.locked.require_owner()?;
        if !self.artifacts.is_empty() || !self.outcomes.is_empty() {
            return Err(err(ErrorCode::InternalError, "staged run is not empty"));
        }
        fs::unlinkat(
            &self.staging_parent,
            self.run_id.as_str(),
            fs::AtFlags::REMOVEDIR,
        )
        .map_err(|e| ioerr("discard empty staged run", e))?;
        self.staging_parent
            .sync_all()
            .map_err(|e| ioerr("sync discarded staged run", e))?;
        Ok(())
    }
    pub fn commit(self, metadata: RunMetadata) -> Result<RunReceipt, ArchyError> {
        self.commit_inner(metadata, Interrupt::None, None)
    }
    /// The callback checks an external binding at publication boundaries. It cannot
    /// exclude a concurrent rename between a check and a filesystem operation.
    pub fn commit_checked(
        self,
        metadata: RunMetadata,
        check: &dyn Fn() -> Result<(), ArchyError>,
    ) -> Result<RunReceipt, ArchyError> {
        self.commit_inner(metadata, Interrupt::None, Some(check))
    }
    fn commit_inner(
        self,
        metadata: RunMetadata,
        interrupt: Interrupt,
        check: Option<&dyn Fn() -> Result<(), ArchyError>>,
    ) -> Result<RunReceipt, ArchyError> {
        self.locked.require_owner()?;
        if self.artifacts.is_empty()
            || metadata.project_id.is_empty()
            || metadata.project_id.len() > 128
            || metadata.algorithm_version.is_empty()
            || metadata.algorithm_version.len() > 128
            || metadata
                .normalized_sha256
                .as_ref()
                .is_some_and(|s| !valid_hash(s))
            || metadata
                .source_format
                .as_deref()
                .is_some_and(|s| !matches!(s, "json" | "yaml"))
            || metadata.parent_variant_id.as_ref().is_some_and(|s| {
                s.len() != 72
                    || !s.starts_with("variant-")
                    || !s[8..]
                        .bytes()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            })
        {
            return Err(err(ErrorCode::InvalidSchema, "incomplete run"));
        }
        let manifest = RunManifest {
            schema_version: RUN_SCHEMA_VERSION.into(),
            project_id: metadata.project_id,
            run_id: self.run_id.clone(),
            idempotency_key: self.request.context.idempotency_key.clone(),
            request_sha256: self.request_sha256.clone(),
            request_schema_version: self.request.schema_version.clone(),
            expected_project_revision: self.request.context.expected_project_revision,
            operation: self.request.operation.clone(),
            parent_run_id: self.request.source_run.clone(),
            parent_variant_id: metadata.parent_variant_id,
            input_sha256: self.request.input_sha256.clone(),
            normalized_sha256: metadata.normalized_sha256,
            source_format: metadata.source_format,
            seed: metadata.seed,
            count: metadata.count,
            algorithm_version: metadata.algorithm_version,
            build_version: env!("CARGO_PKG_VERSION").into(),
            artifacts: self.artifacts,
            outcomes: self.outcomes,
        };
        let manifest_bytes =
            serde_json::to_vec(&manifest).map_err(|e| ioerr("serialize manifest", e))?;
        if manifest_bytes.len() > MAX_MANIFEST_BYTES {
            return Err(err(ErrorCode::InvalidSchema, "manifest exceeds limit"));
        }
        write_synced(&self.staging, "manifest.json", &manifest_bytes)?;
        let mut nested: Vec<_> = self.directories.into_iter().collect();
        nested.sort_by_key(|p| std::cmp::Reverse(p.matches('/').count()));
        for path in nested {
            let mut dir = self
                .staging
                .try_clone()
                .map_err(|e| ioerr("clone staging", e))?;
            for part in path.split('/') {
                dir = open_dir(&dir, part)?;
            }
            dir.sync_all()
                .map_err(|e| ioerr("sync artifact directory", e))?;
        }
        maybe_fault(FaultPoint::StagingDirectorySync)?;
        self.staging
            .sync_all()
            .map_err(|e| ioerr("sync staging run", e))?;
        self.staging_parent
            .sync_all()
            .map_err(|e| ioerr("sync staging parent", e))?;
        let parts = name_path(&self.request.context.destination)?;
        let mut dest = self
            .locked
            .store
            .root
            .try_clone()
            .map_err(|e| ioerr("clone root", e))?;
        for part in parts {
            dest = ensure_dir(&dest, part)?;
            dest.sync_all()
                .map_err(|e| ioerr("sync destination directory", e))?;
        }
        maybe_fault(FaultPoint::BeforeRunRename)?;
        if let Some(check) = check {
            check()?;
        }
        fs::renameat_with(
            &self.staging_parent,
            self.run_id.as_str(),
            &dest,
            self.run_id.as_str(),
            RenameFlags::NOREPLACE,
        )
        .map_err(|e| ioerr("publish run without replacement", e))?;
        maybe_fault(FaultPoint::AfterRunRename).map_err(|e| uncertain("publish run", e))?;
        maybe_fault(FaultPoint::RunStagingParentSync)
            .map_err(|e| uncertain("sync staging parent after publication", e))?;
        self.staging_parent
            .sync_all()
            .map_err(|e| uncertain("sync staging parent after publication", e))?;
        maybe_fault(FaultPoint::RunParentSync).map_err(|e| uncertain("sync run destination", e))?;
        dest.sync_all()
            .map_err(|e| uncertain("sync run destination", e))?;
        if interrupt == Interrupt::AfterRunPublication {
            return Err(uncertain(
                "injected interruption after run publication",
                "test",
            ));
        }
        let receipt = RunReceipt {
            run_id: self.run_id.clone(),
            revision: self.revision,
            manifest_sha256: sha256_hex(&manifest_bytes),
            path: format!("{}/{}", self.request.context.destination, self.run_id),
        };
        let record = RevisionRecord {
            schema_version: REVISION_SCHEMA_VERSION.into(),
            revision: self.revision,
            previous_sha256: self.previous_sha256,
            idempotency_key: self.request.context.idempotency_key,
            request_sha256: self.request_sha256,
            request_schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
            receipt: receipt.clone(),
        };
        if let Some(check) = check {
            check().map_err(|error| uncertain("binding changed after run publication", error))?;
        }
        publish_revision(&self.locked.store.root, &record, interrupt)?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod interruption_tests {
    use super::*;
    use crate::MutationContext;
    use std::cell::Cell;
    use std::process::Command;
    use std::sync::mpsc;

    fn fixture() -> (std::path::PathBuf, ProjectStore) {
        let root = std::env::temp_dir().join(format!("archy-interrupt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("project.json"), b"source").unwrap();
        let store = ProjectStore::open(&root.join("project.json")).unwrap();
        (root, store)
    }
    fn request(key: &str, revision: u64) -> RunRequest {
        RunRequest {
            schema_version: RUN_REQUEST_SCHEMA_VERSION.into(),
            context: MutationContext {
                idempotency_key: key.into(),
                expected_project_revision: revision,
                destination: "runs".into(),
            },
            operation: "generate".into(),
            request_bytes: b"seed=42".to_vec(),
            input_sha256: sha256_hex(b"source"),
            source_run: None,
        }
    }
    fn metadata() -> RunMetadata {
        RunMetadata {
            project_id: "p".into(),
            normalized_sha256: None,
            source_format: None,
            seed: None,
            count: None,
            parent_variant_id: None,
            algorithm_version: "v1".into(),
        }
    }
    #[test]
    fn validated_orphan_is_rechecked_after_recovery_boundary() {
        let (root, store) = fixture();
        let locked = store.lock().unwrap();
        let mut pending = locked
            .begin(request("checked", 0))
            .unwrap()
            .pending()
            .unwrap();
        let run_id = pending.run_id().to_owned();
        pending
            .write("input.json", b"source", "application/json", "test")
            .unwrap();
        assert!(
            pending
                .commit_inner(metadata(), Interrupt::AfterRunPublication, None)
                .is_err()
        );
        let artifact = root.join("runs").join(&run_id).join("input.json");
        RECOVERY_BOUNDARY.with(|hook| {
            *hook.borrow_mut() = Some(Box::new({
                let artifact = artifact.clone();
                move || std::fs::write(artifact, b"changed").unwrap()
            }));
        });
        let validate = |history: &AcceptedHistory<'_>, candidate: &str| {
            assert_eq!(
                history.read_run(candidate)?.artifact("input.json"),
                Some(b"source".as_slice())
            );
            Ok(())
        };
        let error = locked
            .begin_checked_recovery(request("checked", 0), &|| Ok(()), &validate)
            .err()
            .unwrap();
        assert_eq!(error.code, ErrorCode::ArtifactInvalid);
        assert_eq!(locked.revision().unwrap(), 0);
        std::fs::write(&artifact, b"source").unwrap();
        let receipt = locked
            .begin_checked_recovery(request("checked", 0), &|| Ok(()), &validate)
            .unwrap()
            .receipt()
            .unwrap();
        assert_eq!(receipt.run_id, run_id);
        assert_eq!(locked.revision().unwrap(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn orphan_recovery_rechecks_real_binding_before_revision_publication() {
        for replace_root in [false, true] {
            let (root, store) = fixture();
            let source = store.pin_source().unwrap();
            let locked = store.lock().unwrap();
            let mut pending = locked
                .begin(request("orphan", 0))
                .unwrap()
                .pending()
                .unwrap();
            let run_id = pending.run_id().to_owned();
            pending
                .write("input.json", b"source", "application/json", "test")
                .unwrap();
            let error = pending
                .commit_inner(metadata(), Interrupt::AfterRunPublication, None)
                .unwrap_err();
            assert!(error.retriable);
            assert_eq!(locked.revision().unwrap(), 0);
            let moved = root.with_extension("moved");
            let replacement = root.with_extension("replacement");
            let moved_source = root.join("project.moved");
            let boundary_root = root.clone();
            let boundary_moved = moved.clone();
            let boundary_replacement = replacement.clone();
            let boundary_moved_source = moved_source.clone();
            RECOVERY_BOUNDARY.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || {
                    if replace_root {
                        std::fs::rename(&boundary_root, &boundary_moved).unwrap();
                        std::fs::create_dir(&boundary_root).unwrap();
                        std::fs::write(boundary_root.join("project.json"), b"source").unwrap();
                    } else {
                        std::fs::rename(boundary_root.join("project.json"), &boundary_moved_source)
                            .unwrap();
                        std::fs::write(&boundary_replacement, b"changed").unwrap();
                        std::fs::rename(&boundary_replacement, boundary_root.join("project.json"))
                            .unwrap();
                    }
                }))
            });
            let check = || {
                if !store.matches_root_path(&root).unwrap_or(false)
                    || !store.source_matches(&source).unwrap_or(false)
                {
                    return Err(conflict("binding changed"));
                }
                Ok(())
            };
            check().unwrap();
            let result = locked.begin_checked(request("orphan", 0), &check);
            assert!(
                result.is_err(),
                "recovery published after binding replacement"
            );
            let rejection = result.err().unwrap();
            assert!(rejection.retriable);
            assert!(rejection.message.contains("binding changed"));
            assert_eq!(locked.revision().unwrap(), 0);
            assert!(locked.receipts().unwrap().is_empty());
            if replace_root {
                std::fs::remove_dir_all(&root).unwrap();
                std::fs::rename(&moved, &root).unwrap();
            } else {
                std::fs::remove_file(root.join("project.json")).unwrap();
                std::fs::rename(&moved_source, root.join("project.json")).unwrap();
            }
            check().unwrap();
            let replay = locked
                .begin_checked(request("orphan", 0), &check)
                .unwrap()
                .receipt()
                .unwrap();
            assert_eq!(replay.run_id, run_id);
            assert_eq!(locked.revision().unwrap(), 1);
            assert_eq!(
                locked
                    .begin_checked(request("orphan", 0), &check)
                    .unwrap()
                    .receipt(),
                Some(replay)
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn binding_swap_at_publication_boundaries_never_returns_success() {
        for replace_root in [false, true] {
            for swap_at in [1, 2] {
                let (root, store) = fixture();
                let source = store.pin_source().unwrap();
                let locked = store.lock().unwrap();
                let mut pending = locked.begin(request("swap", 0)).unwrap().pending().unwrap();
                pending
                    .write("input.json", b"source", "application/json", "test")
                    .unwrap();
                let moved = root.with_extension("moved");
                let replaced = root.with_extension("replacement");
                let calls = Cell::new(0);
                let (start_swap, swap_requested) = mpsc::channel();
                let (swap_done, done) = mpsc::channel();
                let worker_root = root.clone();
                let worker_moved = moved.clone();
                let worker_replaced = replaced.clone();
                let worker = std::thread::spawn(move || {
                    swap_requested.recv().unwrap();
                    if replace_root {
                        std::fs::rename(&worker_root, &worker_moved).unwrap();
                        std::fs::create_dir(&worker_root).unwrap();
                        std::fs::write(worker_root.join("project.json"), b"source").unwrap();
                    } else {
                        std::fs::write(&worker_replaced, b"changed").unwrap();
                        std::fs::rename(&worker_replaced, worker_root.join("project.json"))
                            .unwrap();
                    }
                    swap_done.send(()).unwrap();
                });
                let check = || {
                    calls.set(calls.get() + 1);
                    if calls.get() == swap_at {
                        start_swap.send(()).unwrap();
                        done.recv().unwrap();
                    }
                    if !store.matches_root_path(&root).unwrap_or(false)
                        || !store.source_matches(&source).unwrap_or(false)
                    {
                        return Err(conflict("binding changed"));
                    }
                    Ok(())
                };
                let failure = pending.commit_checked(metadata(), &check).unwrap_err();
                worker.join().unwrap();
                assert_eq!(calls.get(), swap_at);
                assert_eq!(failure.retriable, swap_at == 2);
                assert_eq!(locked.revision().unwrap(), 0);
                if replace_root {
                    assert!(!root.join(".archy").exists());
                    std::fs::remove_dir_all(&root).unwrap();
                    std::fs::rename(&moved, &root).unwrap();
                } else {
                    std::fs::remove_file(root.join("project.json")).unwrap();
                    std::fs::write(root.join("project.json"), b"source").unwrap();
                }
                let retry = locked.begin(request("swap", 0)).unwrap();
                if swap_at == 1 {
                    let mut retry = retry.pending().unwrap();
                    retry
                        .write("input.json", b"source", "application/json", "test")
                        .unwrap();
                    retry.commit(metadata()).unwrap();
                } else {
                    assert!(retry.receipt().is_some());
                }
                assert_eq!(locked.revision().unwrap(), 1);
                assert_eq!(locked.receipts().unwrap().len(), 1);
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }
    #[test]
    fn injected_sync_and_publication_failures_preserve_history_and_replay_once() {
        for (point, published, committed) in [
            (FaultPoint::FileSync, false, false),
            (FaultPoint::StagingDirectorySync, false, false),
            (FaultPoint::BeforeRunRename, false, false),
            (FaultPoint::AfterRunRename, true, false),
            (FaultPoint::RunStagingParentSync, true, false),
            (FaultPoint::RunParentSync, true, false),
            (FaultPoint::BeforeRevisionRename, true, false),
            (FaultPoint::AfterRevisionRename, true, true),
            (FaultPoint::RevisionDirectorySync, true, true),
        ] {
            let (root, store) = fixture();
            let locked = store.lock().unwrap();
            let mut first = locked
                .begin(request("first", 0))
                .unwrap()
                .pending()
                .unwrap();
            first
                .write("input.json", b"source", "application/json", "test")
                .unwrap();
            let prior = first.commit(metadata()).unwrap();
            let mut second = locked
                .begin(request("second", 1))
                .unwrap()
                .pending()
                .unwrap();
            let second_id = second.run_id().to_owned();
            second
                .write("input.json", b"source", "application/json", "test")
                .unwrap();
            install_fault(point);
            let failure = second.commit(metadata()).unwrap_err();
            assert_eq!(failure.code, ErrorCode::InternalError, "{point:?}");
            assert_eq!(failure.retriable, published, "{point:?}");
            assert_eq!(
                locked
                    .read_run(&prior.run_id)
                    .unwrap()
                    .artifact("input.json"),
                Some(b"source".as_slice()),
                "{point:?}"
            );
            assert_eq!(
                locked.revision().unwrap(),
                if committed { 2 } else { 1 },
                "{point:?}"
            );
            assert_eq!(
                root.join("runs").join(&second_id).exists(),
                published,
                "{point:?}"
            );
            let retry = locked.begin(request("second", 1)).unwrap();
            let receipt = match retry {
                BeginResult::Replay(receipt) => {
                    assert!(published);
                    receipt
                }
                BeginResult::Pending(mut pending) => {
                    assert!(!published);
                    pending
                        .write("input.json", b"source", "application/json", "test")
                        .unwrap();
                    pending.commit(metadata()).unwrap()
                }
            };
            assert_eq!(receipt.revision, 2, "{point:?}");
            if published {
                assert_eq!(receipt.run_id, second_id, "{point:?}");
            }
            assert_eq!(
                locked
                    .begin(request("second", 1))
                    .unwrap()
                    .receipt()
                    .unwrap(),
                receipt,
                "{point:?}"
            );
            assert_eq!(locked.revision().unwrap(), 2, "{point:?}");
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn revision_rename_failure_replay_requires_successful_directory_sync() {
        for point in [
            FaultPoint::AfterRevisionRename,
            FaultPoint::RevisionDirectorySync,
        ] {
            let (root, store) = fixture();
            let locked = store.lock().unwrap();
            let mut first = locked
                .begin(request("first", 0))
                .unwrap()
                .pending()
                .unwrap();
            first
                .write("input.json", b"source", "application/json", "test")
                .unwrap();
            let prior = first.commit(metadata()).unwrap();
            let mut second = locked
                .begin(request("second", 1))
                .unwrap()
                .pending()
                .unwrap();
            let second_id = second.run_id().to_owned();
            second
                .write("input.json", b"source", "application/json", "test")
                .unwrap();
            install_fault(point);
            let failure = second.commit(metadata()).unwrap_err();
            assert!(failure.retriable, "{point:?}");
            let revision_path = root.join(".archy/revisions/00000000000000000002.json");
            let record_bytes = std::fs::read(&revision_path).unwrap();
            let record: RevisionRecord = serde_json::from_slice(&record_bytes).unwrap();
            assert_eq!(record.receipt.run_id, second_id);
            install_fault(FaultPoint::ReplayRevisionDirectorySync);
            let replay_error = locked.begin(request("second", 1)).err().unwrap();
            assert_eq!(replay_error.code, ErrorCode::InternalError, "{point:?}");
            assert!(replay_error.retriable, "{point:?}");
            assert!(
                replay_error
                    .message
                    .contains("retry the same idempotency key")
            );
            assert_eq!(locked.revision().unwrap(), 2);
            assert_eq!(
                locked
                    .read_run(&prior.run_id)
                    .unwrap()
                    .artifact("input.json"),
                Some(b"source".as_slice())
            );
            assert_eq!(std::fs::read(&revision_path).unwrap(), record_bytes);
            let replay = locked
                .begin(request("second", 1))
                .unwrap()
                .receipt()
                .unwrap();
            assert_eq!(replay, record.receipt);
            assert_eq!(
                locked
                    .begin(request("second", 1))
                    .unwrap()
                    .receipt()
                    .unwrap(),
                replay
            );
            assert_eq!(locked.revision().unwrap(), 2);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn orphan_retries_both_rename_parent_syncs_before_revision_commit() {
        for (initial, recovery) in [
            (
                FaultPoint::RunStagingParentSync,
                FaultPoint::RecoveryStagingParentSync,
            ),
            (
                FaultPoint::RunParentSync,
                FaultPoint::RecoveryDestinationSync,
            ),
        ] {
            let (root, store) = fixture();
            let locked = store.lock().unwrap();
            let mut first = locked
                .begin(request("first", 0))
                .unwrap()
                .pending()
                .unwrap();
            first
                .write("input.json", b"source", "application/json", "test")
                .unwrap();
            let prior = first.commit(metadata()).unwrap();
            let mut second = locked
                .begin(request("second", 1))
                .unwrap()
                .pending()
                .unwrap();
            let second_id = second.run_id().to_owned();
            second
                .write("input.json", b"source", "application/json", "test")
                .unwrap();
            install_fault(initial);
            let error = second.commit(metadata()).unwrap_err();
            assert!(error.retriable, "{initial:?}");
            assert_eq!(locked.revision().unwrap(), 1);
            install_fault(recovery);
            let error = locked.begin(request("second", 1)).err().unwrap();
            assert!(error.retriable, "{recovery:?}");
            assert_eq!(locked.revision().unwrap(), 1, "{recovery:?}");
            assert_eq!(
                locked
                    .read_run(&prior.run_id)
                    .unwrap()
                    .artifact("input.json"),
                Some(b"source".as_slice())
            );
            assert!(root.join("runs").join(&second_id).exists());
            let receipt = locked
                .begin(request("second", 1))
                .unwrap()
                .receipt()
                .unwrap();
            assert_eq!(receipt.run_id, second_id);
            assert_eq!(receipt.revision, 2);
            assert_eq!(
                locked
                    .begin(request("second", 1))
                    .unwrap()
                    .receipt()
                    .unwrap(),
                receipt
            );
            assert_eq!(locked.revision().unwrap(), 2);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn interrupted_after_run_publication_recovers_verified_orphan() {
        let (root, store) = fixture();
        let locked = store.lock().unwrap();
        let mut pending = locked
            .begin(request("first", 0))
            .unwrap()
            .pending()
            .unwrap();
        let run_id = pending.run_id().to_owned();
        pending
            .write("input.json", b"source", "application/json", "test")
            .unwrap();
        assert!(
            pending
                .commit_inner(metadata(), Interrupt::AfterRunPublication, None)
                .is_err()
        );
        assert!(root.join("runs").join(&run_id).exists());
        assert_eq!(locked.revision().unwrap(), 0);
        assert!(locked.read_run(&run_id).is_err());
        let recovered = locked
            .begin(request("first", 0))
            .unwrap()
            .receipt()
            .unwrap();
        assert_eq!(recovered.run_id, run_id);
        assert_eq!(recovered.revision, 1);
        assert_eq!(locked.revision().unwrap(), 1);
        let mut next = locked
            .begin(request("second", 1))
            .unwrap()
            .pending()
            .unwrap();
        next.write("input.json", b"source", "application/json", "test")
            .unwrap();
        assert_eq!(next.commit(metadata()).unwrap().revision, 2);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_after_revision_publication_replays_one_receipt() {
        let (root, store) = fixture();
        let locked = store.lock().unwrap();
        let mut pending = locked
            .begin(request("first", 0))
            .unwrap()
            .pending()
            .unwrap();
        let run_id = pending.run_id().to_owned();
        pending
            .write("input.json", b"source", "application/json", "test")
            .unwrap();
        assert!(
            pending
                .commit_inner(metadata(), Interrupt::AfterRevisionPublication, None)
                .is_err()
        );
        let receipt = locked
            .begin(request("first", 0))
            .unwrap()
            .receipt()
            .unwrap();
        assert_eq!(receipt.run_id, run_id);
        assert_eq!(receipt.revision, 1);
        assert_eq!(locked.revision().unwrap(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn child_aborts_after_run_publication() {
        let Ok(root) = std::env::var("ARCHY_ABORT_AFTER_RUN") else {
            return;
        };
        let store = ProjectStore::open(&std::path::Path::new(&root).join("project.json")).unwrap();
        let locked = store.lock().unwrap();
        let mut pending = locked
            .begin(request("crash", 0))
            .unwrap()
            .pending()
            .unwrap();
        pending
            .write("input.json", b"source", "application/json", "test")
            .unwrap();
        let result = pending.commit_inner(metadata(), Interrupt::AfterRunPublication, None);
        assert!(result.is_err());
        std::process::abort();
    }

    #[test]
    fn aborted_child_run_is_recovered_once() {
        let (root, store) = fixture();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::interruption_tests::child_aborts_after_run_publication",
                "--nocapture",
            ])
            .env("ARCHY_ABORT_AFTER_RUN", &root)
            .status()
            .unwrap();
        assert!(!status.success());
        let locked = store.lock().unwrap();
        let receipt = locked
            .begin(request("crash", 0))
            .unwrap()
            .receipt()
            .unwrap();
        assert_eq!(receipt.revision, 1);
        assert_eq!(
            locked
                .begin(request("crash", 0))
                .unwrap()
                .receipt()
                .unwrap(),
            receipt
        );
        assert_eq!(locked.revision().unwrap(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
