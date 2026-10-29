//! Pinned, noncreating project reads shared by desktop and the stdio facade.
use crate::{
    GenerationMode, MutationOptions, TrustedRun, VariantSelection, WorkflowReceipt,
    artifact_invalid,
    evaluate::evaluate_locked,
    generate::generate_locked,
    input_format,
    run_integrity::{verify_history, verify_locked},
};
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
    format: InputFormat,
    source_format: &'static str,
}
impl BoundProjectRead {
    pub fn open(project: &Path) -> Result<Self, ArchyError> {
        let root_path = project
            .parent()
            .ok_or_else(|| ArchyError::schema("project", "Project has no parent"))?
            .to_path_buf();
        let (format, source_format, _) = input_format(project)?;
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
            format,
            source_format,
        })
    }
    fn check_binding(&self) -> Result<(), ArchyError> {
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
        Ok(())
    }
    pub fn snapshot(&self) -> Result<BoundReadSnapshot<'_>, ArchyError> {
        self.check_binding()?;
        Ok(BoundReadSnapshot {
            owner: self,
            history: self.store.read_history()?,
        })
    }
    pub(crate) fn with_verified_locked_run<T>(
        &self,
        run_id: &str,
        use_run: impl FnOnce(&TrustedRun) -> Result<T, ArchyError>,
    ) -> Result<T, ArchyError> {
        let locked = self.store.lock()?;
        self.check_binding()?;
        let run = crate::run_integrity::verify_locked(&locked, run_id)?;
        if run.manifest().project_id != self.spec.project_id {
            return Err(artifact_invalid(
                "Accepted run belongs to another project ID",
            ));
        }
        let result = use_run(&run)?;
        self.check_binding()?;
        Ok(result)
    }
    pub fn project_id(&self) -> &str {
        &self.spec.project_id
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    fn transact(
        &self,
        action: impl FnOnce(
            &archy_core::LockedProject<'_>,
            &dyn Fn() -> Result<(), ArchyError>,
        ) -> Result<WorkflowReceipt, ArchyError>,
    ) -> Result<WorkflowReceipt, ArchyError> {
        self.check_binding()?;
        self.store.read_history()?;
        let locked = self.store.lock()?;
        self.check_binding()?;
        if let Some(head) = locked.receipts()?.last() {
            let run = verify_locked(&locked, &head.run_id)?;
            if run.spec().project_id != self.spec.project_id {
                return Err(artifact_invalid(
                    "Accepted head belongs to another project ID",
                ));
            }
        }
        self.check_binding()?;
        let receipt = action(&locked, &|| self.check_binding())?;
        let run = verify_locked(&locked, &receipt.run_id)?;
        if run.spec().project_id != self.spec.project_id {
            return Err(artifact_invalid(
                "Accepted run belongs to another project ID",
            ));
        }
        self.check_binding().map_err(|_| {
            let mut error = ArchyError::new(
                ErrorCode::RevisionConflict,
                "project",
                "Configured project changed during mutation; commit may have occurred on the pinned root; retry the same idempotency key after restoring the project binding",
            );
            error.retriable = true;
            error
        })?;
        Ok(receipt)
    }
    pub fn generate(
        &self,
        count: u32,
        seed: u64,
        mutation: MutationOptions,
    ) -> Result<WorkflowReceipt, ArchyError> {
        self.transact(|locked, check| {
            generate_locked(
                locked,
                GenerationMode::Sample { count, seed },
                mutation,
                self.format,
                self.source_format,
                Some(&self.source_bytes),
                Some(check),
            )
        })
    }
    pub fn evaluate(
        &self,
        source_run: String,
        variant_id: String,
        mutation: MutationOptions,
    ) -> Result<WorkflowReceipt, ArchyError> {
        self.transact(|locked, check| {
            let source = verify_locked(locked, &source_run)?;
            if source.spec().project_id != self.spec.project_id {
                return Err(artifact_invalid(
                    "Evaluation source belongs to another project ID",
                ));
            }
            evaluate_locked(
                locked,
                Some(source_run),
                VariantSelection::One(variant_id),
                mutation,
                Some(check),
            )
        })
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
