//! Shared descriptor-based project source path and identity checks.
use crate::hex_digest;
use rustix::fs::{self as rfs, AtFlags, Mode, OFlags, ResolveFlags, StatxFlags};
use std::fs::File;
use std::io::{self, ErrorKind, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

fn denied(why: &'static str) -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, why)
}

fn invalid(why: &'static str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, why)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub dev: u64,
    pub ino: u64,
    pub mount: u64,
}

pub fn identity(file: &File) -> io::Result<Identity> {
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

pub fn child(parent: &File, name: &std::ffi::OsStr, directory: bool) -> io::Result<File> {
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

pub fn pin_path(path: &Path, owner: u32) -> io::Result<Vec<File>> {
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

pub fn source_digest(file: &File) -> io::Result<String> {
    let clone = file.try_clone()?;
    let mut bytes = Vec::new();
    clone.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("project source exceeds bound"));
    }
    Ok(hex_digest(&bytes))
}

pub fn identity_digest(chain: &[File]) -> io::Result<String> {
    let mut bytes = Vec::with_capacity(chain.len() * 24);
    for file in chain {
        let id = identity(file)?;
        bytes.extend_from_slice(&id.dev.to_le_bytes());
        bytes.extend_from_slice(&id.ino.to_le_bytes());
        bytes.extend_from_slice(&id.mount.to_le_bytes());
    }
    Ok(hex_digest(&bytes))
}
