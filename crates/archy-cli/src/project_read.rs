//! Pinned, noncreating project reads shared by desktop and the stdio facade.
use crate::{TrustedRun, artifact_invalid, input_format, run_integrity::verify_history};
use archy_core::{
    AcceptedHistory, ArchyError, ErrorCode, InputFormat, NormalizedSpec, PinnedProjectSource,
    ProjectStore, RunReceipt, parse_project, sha256_hex,
};
use std::path::{Path, PathBuf};

pub struct BoundProjectRead {
    store: ProjectStore,
    source: PinnedProjectSource,
    source_bytes: Vec<u8>,
    source_sha256: String,
    spec: NormalizedSpec,
    root_path: PathBuf,
    identity: String,
    _format: InputFormat,
}
impl BoundProjectRead {
    pub fn open(project: &Path) -> Result<Self, ArchyError> {
        let root_path = project
            .parent()
            .ok_or_else(|| ArchyError::schema("project", "Project has no parent"))?
            .to_path_buf();
        let (format, _, _) = input_format(project)?;
        let store = ProjectStore::open(project)?;
        let source = store.pin_source()?;
        let source_bytes = source.read()?;
        let text = std::str::from_utf8(&source_bytes)
            .map_err(|_| ArchyError::schema("project", "Project is not UTF-8"))?;
        let spec = parse_project(text, format)?;
        let source_sha256 = sha256_hex(&source_bytes);
        let identity = store.identity_digest()?;
        Ok(Self {
            store,
            source,
            source_bytes,
            source_sha256,
            spec,
            root_path,
            identity,
            _format: format,
        })
    }
    pub fn snapshot(&self) -> Result<BoundReadSnapshot<'_>, ArchyError> {
        let root_matches = self
            .store
            .matches_root_path(&self.root_path)
            .unwrap_or(false);
        let source_matches = self.store.source_matches(&self.source).unwrap_or(false);
        if !root_matches || !source_matches || self.source.read()? != self.source_bytes {
            return Err(ArchyError::new(
                ErrorCode::RevisionConflict,
                "project",
                "Configured project path or source changed; restart the reader",
            ));
        }
        Ok(BoundReadSnapshot {
            owner: self,
            history: self.store.read_history()?,
        })
    }
    pub fn project_id(&self) -> &str {
        &self.spec.project_id
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
}
pub struct BoundReadSnapshot<'a> {
    owner: &'a BoundProjectRead,
    history: AcceptedHistory<'a>,
}
impl BoundReadSnapshot<'_> {
    pub fn revision(&self) -> u64 {
        self.history.revision()
    }
    pub fn head_sha256(&self) -> Option<&str> {
        self.history.head_sha256()
    }
    pub fn receipts(&self) -> impl DoubleEndedIterator<Item = &RunReceipt> {
        self.history.receipts()
    }
    pub fn spec(&self) -> &NormalizedSpec {
        &self.owner.spec
    }
    pub fn source_sha256(&self) -> &str {
        &self.owner.source_sha256
    }
    pub fn identity(&self) -> &str {
        &self.owner.identity
    }
    pub fn verify_head(&self) -> Result<(), ArchyError> {
        if let Some(receipt) = self.history.receipts().last() {
            self.verify_run(&receipt.run_id)?;
        }
        Ok(())
    }
    pub fn verify_run(&self, run_id: &str) -> Result<TrustedRun, ArchyError> {
        let run = verify_history(&self.history, run_id)?;
        if run.manifest().project_id != self.owner.spec.project_id {
            return Err(artifact_invalid(
                "Accepted run belongs to another project ID",
            ));
        }
        Ok(run)
    }
}
