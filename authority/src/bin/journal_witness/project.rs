//! Pinned broker-owned project source for the root journal witness.
//! This verifies path identity and source bytes; it does not verify a human decision.

use std::fs::File;
use std::io::{self, ErrorKind};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use archy_authority::project_path::{child, identity, identity_digest, pin_path, source_digest};

fn denied(why: &'static str) -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, why)
}

fn lock_chain(project_dir: &File, owner: u32) -> io::Result<Vec<File>> {
    let archy = child(project_dir, std::ffi::OsStr::new(".archy"), true)?;
    let staging = child(&archy, std::ffi::OsStr::new("staging"), true)?;
    let revisions = child(&archy, std::ffi::OsStr::new("revisions"), true)?;
    let lock = child(&archy, std::ffi::OsStr::new("lock"), false)?;
    let chain = vec![archy, staging, revisions, lock];
    for (index, file) in chain.iter().enumerate() {
        let meta = file.metadata()?;
        let valid = if index == 3 {
            meta.is_file() && meta.nlink() == 1 && meta.mode() & 0o7777 == 0o600
        } else {
            meta.is_dir() && meta.mode() & 0o7777 == 0o700
        };
        if !valid || meta.uid() != owner {
            return Err(denied("project lock structure is not broker provisioned"));
        }
    }
    Ok(chain)
}

/// Fixed broker-owned project path and source used by the witness.
pub struct WitnessProject {
    path: PathBuf,
    owner: u32,
    chain: Vec<File>,
    source_sha256: String,
    identity_sha256: String,
    lock_identity_sha256: String,
}

impl WitnessProject {
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    pub fn identity_sha256(&self) -> &str {
        &self.identity_sha256
    }
    pub fn lock_identity_sha256(&self) -> &str {
        &self.lock_identity_sha256
    }
    pub fn open(path: &Path, owner: u32) -> io::Result<Self> {
        if owner == 0 {
            return Err(denied("broker must be non-root"));
        }
        let chain = pin_path(path, owner)?;
        let source_sha256 = source_digest(chain.last().unwrap())?;
        let identity_sha256 = identity_digest(&chain)?;
        let lock_identity_sha256 = identity_digest(&lock_chain(&chain[chain.len() - 2], owner)?)?;
        let project = Self {
            path: path.to_owned(),
            owner,
            chain,
            source_sha256,
            identity_sha256,
            lock_identity_sha256,
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
        if identity_digest(&lock_chain(&current[current.len() - 2], self.owner)?)?
            != self.lock_identity_sha256
        {
            return Err(denied("project lock identity changed since open"));
        }
        Ok(())
    }
}
