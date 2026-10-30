//! Pinned broker-owned project source for the root journal witness.
//! This verifies path identity and source bytes; it does not verify a human decision.
use rustix::fs::{self as rfs, AtFlags, Mode, OFlags, ResolveFlags, StatxFlags};
use std::fs::File;
use std::io::{self, ErrorKind, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use archy_authority::hex_digest;

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

fn source_digest(file: &File) -> io::Result<String> {
    let clone = file.try_clone()?;
    let mut bytes = Vec::new();
    clone.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("project source exceeds bound"));
    }
    Ok(hex_digest(&bytes))
}

fn identity_digest(chain: &[File]) -> io::Result<String> {
    // Match ProtectedProject's path identity encoding exactly.
    let mut bytes = Vec::with_capacity(chain.len() * 24);
    for file in chain {
        let id = identity(file)?;
        bytes.extend_from_slice(&id.dev.to_le_bytes());
        bytes.extend_from_slice(&id.ino.to_le_bytes());
        bytes.extend_from_slice(&id.mount.to_le_bytes());
    }
    Ok(hex_digest(&bytes))
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
