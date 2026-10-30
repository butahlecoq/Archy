//! Broker-owned evidence verification. This module does not authenticate a human.
use archy_cli::{BoundProjectRead, MutationOptions, WorkflowReceipt};
use archy_core::decision_contract::Proposal;
use archy_core::{CheckResult, InputFormat, ProjectStore, parse_project_current};
use rustix::fs::{self as rfs, AtFlags, Mode, OFlags, ResolveFlags, StatxFlags};
use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use crate::{MAX_FRAME, hex_digest};
const MAX_REVIEW_BYTES: usize = 1024 * 1024;
const MAX_REVIEW_REVISIONS: usize = 16;
const MAX_REVIEW_HISTORY_BYTES: u64 = 32 * 1024 * 1024;

fn denied(why: &'static str) -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, why)
}

fn invalid(why: &'static str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, why)
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    mount: u64,
}

fn identity(file: &File) -> io::Result<Identity> {
    let meta = file.metadata()?;
    let stat =
        rfs::statx(file, "", AtFlags::EMPTY_PATH, StatxFlags::MNT_ID).map_err(io::Error::from)?;
    if stat.stx_mask & StatxFlags::MNT_ID.bits() == 0 {
        return Err(denied("mount identity unavailable"));
    }
    Ok(Identity {
        dev: meta.dev(),
        ino: meta.ino(),
        mount: stat.stx_mnt_id,
    })
}

fn child(parent: &File, name: &std::ffi::OsStr, directory: bool) -> io::Result<File> {
    let mut flags = OFlags::RDONLY | OFlags::CLOEXEC;
    if directory {
        flags |= OFlags::DIRECTORY;
    }
    let fd = rfs::openat2(
        parent,
        name,
        flags,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(io::Error::from)?;
    Ok(File::from(fd))
}

fn pin_path(path: &Path, owner: u32) -> io::Result<Vec<File>> {
    if !path.is_absolute()
        || path.components().collect::<PathBuf>() != path
        || path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        return Err(denied("protected path must be absolute and normalized"));
    }
    let mut chain = vec![File::open("/")?];
    let filesystem_root = chain[0].metadata()?;
    if !filesystem_root.is_dir()
        || filesystem_root.uid() != 0
        || filesystem_root.mode() & 0o022 != 0
    {
        return Err(denied("untrusted filesystem root"));
    }
    let components: Vec<_> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    if components.len() < 2 {
        return Err(denied("project has no private root"));
    }
    for (index, name) in components.iter().enumerate() {
        let is_file = index + 1 == components.len();
        let next = child(chain.last().unwrap(), name, !is_file)?;
        let meta = next.metadata()?;
        let valid = if is_file {
            meta.is_file()
                && meta.uid() == owner
                && meta.mode() & 0o7777 == 0o600
                && meta.nlink() == 1
        } else if index + 2 == components.len() {
            meta.is_dir() && meta.uid() == owner && meta.mode() & 0o7777 == 0o700
        } else {
            meta.is_dir() && (meta.uid() == 0 || meta.uid() == owner) && meta.mode() & 0o022 == 0
        };
        if !valid {
            return Err(denied("untrusted protected path owner, mode or link"));
        }
        chain.push(next);
    }
    Ok(chain)
}

fn pin_state_directory(path: &Path, owner: u32) -> io::Result<Vec<File>> {
    if !path.is_absolute()
        || path.components().collect::<PathBuf>() != path
        || path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        return Err(denied("broker state path must be absolute and normalized"));
    }
    let mut chain = vec![File::open("/")?];
    let root = chain[0].metadata()?;
    if root.uid() != 0 || root.mode() & 0o022 != 0 {
        return Err(denied("untrusted filesystem root"));
    }
    let names: Vec<_> = path
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    if names.is_empty() {
        return Err(denied("broker state directory missing"));
    }
    for (index, name) in names.iter().enumerate() {
        let next = child(chain.last().unwrap(), name, true)?;
        let meta = next.metadata()?;
        let valid = if index + 1 == names.len() {
            meta.is_dir() && meta.uid() == owner && meta.mode() & 0o7777 == 0o700
        } else {
            meta.is_dir() && (meta.uid() == 0 || meta.uid() == owner) && meta.mode() & 0o022 == 0
        };
        if !valid {
            return Err(denied("untrusted broker state ancestor"));
        }
        chain.push(next);
    }
    Ok(chain)
}

fn source_digest(file: &File) -> io::Result<String> {
    let clone = file.try_clone()?;
    let mut bytes = Vec::new();
    clone.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("project source exceeds bound"));
    }
    Ok(hex_digest(&bytes))
}

/// The caller must supply a fixed administrator-selected path beneath the broker
/// state directory. The agent must have no write access to any ancestor.
pub struct ProtectedProject {
    path: PathBuf,
    owner: u32,
    chain: Vec<File>,
    source_sha256: String,
    identity_sha256: String,
}

/// Exact bounded display bytes and their broker-calculated digest. No approval token.
pub struct ReviewSnapshot {
    bytes: Vec<u8>,
    digest: String,
    revision: u64,
    head: String,
}

impl ReviewSnapshot {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn head(&self) -> &str {
        &self.head
    }
}

impl ProtectedProject {
    pub fn state_dir(&self) -> &Path {
        self.path.parent().unwrap().parent().unwrap()
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub fn identity_sha256(&self) -> &str {
        &self.identity_sha256
    }

    /// Create one fixed `project/project.json` beneath a preexisting private
    /// broker state directory. An interrupted initialization leaves an exclusive
    /// partial directory for administrator recovery; it never adopts old bytes.
    pub fn initialize(state_dir: &Path, owner: u32, source: &[u8]) -> io::Result<Self> {
        if owner == 0 || unsafe { libc::geteuid() } != owner {
            return Err(denied("initializer must run as broker"));
        }
        if source.is_empty() || source.len() > 1024 * 1024 {
            return Err(invalid("protected source exceeds bound"));
        }
        let text = std::str::from_utf8(source).map_err(|_| invalid("source is not UTF-8"))?;
        parse_project_current(text, InputFormat::Json).map_err(io::Error::other)?;
        let state_chain = pin_state_directory(state_dir, owner)?;
        let state = state_chain.last().unwrap();
        rfs::mkdirat(state, "project", Mode::RUSR | Mode::WUSR | Mode::XUSR)
            .map_err(io::Error::from)?;
        state.sync_all()?;
        let project_dir = child(state, std::ffi::OsStr::new("project"), true)?;
        let fd = rfs::openat2(
            &project_dir,
            "project.json",
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )
        .map_err(io::Error::from)?;
        let mut file = File::from(fd);
        file.write_all(source)?;
        file.sync_all()?;
        project_dir.sync_all()?;
        let project_path = state_dir.join("project/project.json");
        let project = Self::open(&project_path, owner)?;
        let current_state = pin_state_directory(state_dir, owner)?;
        if current_state.len() != state_chain.len() {
            return Err(denied("broker state path changed during initialization"));
        }
        for (a, b) in current_state.iter().zip(&state_chain) {
            if identity(a)? != identity(b)? {
                return Err(denied("broker state path changed during initialization"));
            }
        }
        Ok(project)
    }
    pub fn open(path: &Path, owner: u32) -> io::Result<Self> {
        if owner == 0 {
            return Err(denied("broker must be non-root"));
        }
        let chain = pin_path(path, owner)?;
        let source_sha256 = source_digest(chain.last().unwrap())?;
        let mut identity_bytes = Vec::with_capacity(chain.len() * 24);
        for file in &chain {
            let id = identity(file)?;
            identity_bytes.extend_from_slice(&id.dev.to_le_bytes());
            identity_bytes.extend_from_slice(&id.ino.to_le_bytes());
            identity_bytes.extend_from_slice(&id.mount.to_le_bytes());
        }
        let project = Self {
            path: path.to_owned(),
            owner,
            chain,
            source_sha256,
            identity_sha256: hex_digest(&identity_bytes),
        };
        project.check_owner()?;
        Ok(project)
    }

    fn check_owner(&self) -> io::Result<()> {
        let current = pin_path(&self.path, self.owner)?;
        if current.len() != self.chain.len() {
            return Err(denied("protected path identity changed"));
        }
        for (a, b) in current.iter().zip(&self.chain) {
            if identity(a)? != identity(b)? {
                return Err(denied("protected path identity changed"));
            }
        }
        if source_digest(current.last().unwrap())? != self.source_sha256 {
            return Err(denied("project source changed since open"));
        }
        Ok(())
    }

    /// Publish a broker-generated run into this private history. The caller may
    /// choose generation parameters, but cannot supply accepted artifacts or
    /// checks. The pinned reader verifies the source and accepted run under the
    /// project lock. A changed path/source invalidates this writer instance.
    pub fn generate_accepted(
        &self,
        count: u32,
        seed: u64,
        idempotency_key: String,
        expected_revision: u64,
    ) -> io::Result<WorkflowReceipt> {
        if count == 0 || count > 32 {
            return Err(invalid("protected run exceeds review outcome bound"));
        }
        if idempotency_key.is_empty() || idempotency_key.len() > 128 {
            return Err(invalid("invalid protected run key"));
        }
        self.check_owner()?;
        let store = ProjectStore::open(&self.path).map_err(io::Error::other)?;
        let history = store.read_history().map_err(io::Error::other)?;
        if history.revision() > MAX_REVIEW_REVISIONS as u64 {
            return Err(invalid("protected history exceeds review bound"));
        }
        if history.revision() == MAX_REVIEW_REVISIONS as u64
            && expected_revision == history.revision()
        {
            return Err(invalid("protected history is full"));
        }
        let reader = BoundProjectRead::open(&self.path).map_err(io::Error::other)?;
        self.check_owner()?;
        let receipt = reader
            .generate(
                count,
                seed,
                MutationOptions {
                    idempotency_key: Some(idempotency_key),
                    expected_project_revision: Some(expected_revision),
                    destination: "runs".into(),
                },
            )
            .map_err(io::Error::other)?;
        self.check_owner()?;
        let store = ProjectStore::open(&self.path).map_err(io::Error::other)?;
        let history = store.read_history().map_err(io::Error::other)?;
        if history.revision() > MAX_REVIEW_REVISIONS as u64 {
            return Err(invalid("protected history exceeds review bound"));
        }
        let _ = archy_cli::verify_history(&history, &receipt.run_id).map_err(io::Error::other)?;
        Ok(receipt)
    }

    /// Recomputes the accepted run and comparison while holding the project lock.
    /// A matching proposal still has no authority until a separate authenticated
    /// human action and final locked recheck have completed.
    pub fn snapshot(&self, proposal_bytes: &[u8]) -> io::Result<ReviewSnapshot> {
        if proposal_bytes.is_empty() || proposal_bytes.len() >= MAX_FRAME {
            return Err(invalid("proposal exceeds bound"));
        }
        self.check_owner()?;
        let proposal: Proposal =
            serde_json::from_slice(proposal_bytes).map_err(|_| invalid("invalid proposal"))?;
        proposal
            .validate()
            .map_err(|_| invalid("invalid proposal evidence"))?;
        let store = ProjectStore::open(&self.path).map_err(io::Error::other)?;
        let locked = store.lock().map_err(io::Error::other)?;
        self.check_owner()?;
        if !store
            .matches_root_path(self.path.parent().unwrap())
            .map_err(io::Error::other)?
        {
            return Err(denied("protected root changed"));
        }
        let history = locked
            .accepted_history_bounded(MAX_REVIEW_REVISIONS, MAX_REVIEW_HISTORY_BYTES)
            .map_err(io::Error::other)?;
        let evidence = &proposal.evidence;
        if history.revision() != evidence.expected_project_revision
            || history.head_sha256() != Some(evidence.expected_head_sha256.as_str())
        {
            return Err(invalid("stale project head"));
        }
        let run = archy_cli::verify_history(&history, &evidence.source_run_id)
            .map_err(io::Error::other)?;
        let receipt = history
            .receipts()
            .find(|r| r.run_id == evidence.source_run_id)
            .ok_or_else(|| invalid("run not accepted"))?;
        let manifest = run.manifest();
        if manifest.project_id != evidence.project_id
            || manifest.input_sha256 != evidence.input_sha256
            || manifest.normalized_sha256.as_deref() != Some(evidence.normalized_sha256.as_str())
        {
            return Err(invalid("source evidence differs"));
        }
        let index = run
            .variants()
            .iter()
            .position(|v| v.id == evidence.variant_id)
            .ok_or_else(|| invalid("variant absent from run"))?;
        let variant = &run.variants()[index];
        let evaluation = &run.evaluations()[index];
        let variant_path = format!("variants/{}.json", variant.id);
        let evaluation_path = format!("evaluations/{}.json", variant.id);
        let stored = history
            .read_run(&evidence.source_run_id)
            .map_err(io::Error::other)?;
        let variant_bytes = stored
            .artifact(&variant_path)
            .ok_or_else(|| invalid("variant artifact absent"))?;
        let evaluation_bytes = stored
            .artifact(&evaluation_path)
            .ok_or_else(|| invalid("evaluation artifact absent"))?;
        let normalized = stored
            .artifact("normalized.json")
            .ok_or_else(|| invalid("normalized artifact absent"))?;
        if hex_digest(variant_bytes) != evidence.variant_sha256
            || hex_digest(evaluation_bytes) != evidence.evaluation_sha256
            || hex_digest(normalized) != evidence.normalized_sha256
            || evaluation.checks != evidence.checks
            || !proposal
                .evidence
                .claimed_checks_passed_against(normalized, variant_bytes, evaluation_bytes)
                .map_err(|_| invalid("claimed checks differ"))?
            || evaluation
                .checks
                .iter()
                .any(|c| c.result != CheckResult::Pass)
        {
            return Err(invalid("evidence does not pass recomputation"));
        }
        let comparison = archy_cli::render_verified_comparison(
            &run,
            &receipt.manifest_sha256,
            history.revision(),
            history.head_sha256().map(str::to_owned),
        )
        .map_err(io::Error::other)?;
        if hex_digest(&comparison.json) != evidence.comparison_sha256 {
            return Err(invalid("comparison differs"));
        }
        let input_text =
            std::str::from_utf8(run.input()).map_err(|_| invalid("input is not UTF-8"))?;
        let normalized_text =
            std::str::from_utf8(normalized).map_err(|_| invalid("normalized is not UTF-8"))?;
        let variant_text =
            std::str::from_utf8(variant_bytes).map_err(|_| invalid("variant is not UTF-8"))?;
        let evaluation_text = std::str::from_utf8(evaluation_bytes)
            .map_err(|_| invalid("evaluation is not UTF-8"))?;
        let comparison_text = std::str::from_utf8(&comparison.json)
            .map_err(|_| invalid("comparison is not UTF-8"))?;
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": "archy-broker-review-v1",
            "proposal_sha256": hex_digest(proposal_bytes),
            "project_id": evidence.project_id,
            "source_run_id": evidence.source_run_id,
            "variant_id": evidence.variant_id,
            "revision": history.revision(),
            "head_sha256": evidence.expected_head_sha256,
            "comparison_sha256": evidence.comparison_sha256,
            "suggested_reason_unverified": proposal.suggested_reason,
            "checks": evaluation.checks,
            "input_provenance": "accepted_run_artifact",
            "current_source_sha256": self.source_sha256,
            "current_source_differs_from_run": self.source_sha256 != evidence.input_sha256,
            "input_utf8": input_text,
            "normalized_json": normalized_text,
            "variant_json": variant_text,
            "evaluation_json": evaluation_text,
            "comparison_json": comparison_text,
        }))
        .map_err(io::Error::other)?;
        if bytes.len() > MAX_REVIEW_BYTES {
            return Err(invalid("review snapshot exceeds bound"));
        }
        self.check_owner()?;
        Ok(ReviewSnapshot {
            digest: hex_digest(&bytes),
            bytes,
            revision: history.revision(),
            head: evidence.expected_head_sha256.clone(),
        })
    }
}
