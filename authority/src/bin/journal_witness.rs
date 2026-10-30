//! Root-owned local witness. It never accepts a caller-supplied digest or project path.
use archy_authority::journal::Journal;
#[path = "journal_witness/project.rs"]
mod project;
use archy_core::ProjectStore;
use project::WitnessProject;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Component, Path, PathBuf};

const DOMAIN: &[u8] = b"archy-root-journal-witness-v2\0";
const BINDING_VERSION: &str = "archy-root-journal-binding-v2";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    schema_version: String,
    broker_uid: u32,
    broker_gid: u32,
    journal_path: PathBuf,
    journal_dev: u64,
    journal_ino: u64,
    project_path: PathBuf,
    project_dev: u64,
    project_ino: u64,
    project_source_sha256: String,
    project_identity_sha256: String,
    project_lock_identity_sha256: String,
}

fn denied(reason: &'static str) -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, reason)
}

fn check_root_path(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        return Err(denied("root path must be normalized absolute"));
    }
    let mut current = PathBuf::from("/");
    for part in path.components().skip(1) {
        current.push(part);
        let metadata = fs::symlink_metadata(&current)?;
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(denied("witness path ancestor is mutable by another UID"));
        }
    }
    Ok(())
}

fn check_state_dir(path: &Path) -> io::Result<File> {
    check_root_path(path)?;
    let dir = File::open(path)?;
    if dir.metadata()?.mode() & 0o7777 != 0o700 {
        return Err(denied("witness directory requires mode 0700"));
    }
    Ok(dir)
}

fn lock(fd: &File) -> io::Result<()> {
    if unsafe { libc::flock(fd.as_raw_fd(), libc::LOCK_EX) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn metadata_id(path: &Path) -> io::Result<(u64, u64)> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() && !meta.is_file() {
        return Err(denied("bound object has wrong type"));
    }
    Ok((meta.dev(), meta.ino()))
}

fn binding_check(binding: &Binding) -> io::Result<()> {
    if metadata_id(&binding.journal_path)? != (binding.journal_dev, binding.journal_ino)
        || metadata_id(&binding.project_path)? != (binding.project_dev, binding.project_ino)
    {
        return Err(denied("bound path identity changed"));
    }
    let project = WitnessProject::open(&binding.project_path, binding.broker_uid)?;
    if project.source_sha256() != binding.project_source_sha256 {
        return Err(denied("bound project source changed"));
    }
    if project.identity_sha256() != binding.project_identity_sha256
        || project.lock_identity_sha256() != binding.project_lock_identity_sha256
    {
        return Err(denied("bound project ancestor or lock identity changed"));
    }
    Ok(())
}

fn write_new(dir: &File, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    dir.sync_all()
}

fn state_name(seq: usize) -> String {
    format!("state-{seq:020}")
}

fn write_head(dir: &File, dir_path: &Path, seq: usize) -> io::Result<()> {
    let pending = dir_path.join(".head-pending");
    write_new(dir, &pending, format!("{seq:020}\n").as_bytes())?;
    fs::rename(pending, dir_path.join("head"))?;
    dir.sync_all()
}

fn extend(old: [u8; 32], record_hash: &str) -> io::Result<[u8; 32]> {
    let record = hex_decode(record_hash)?;
    let mut hash = Sha256::new();
    hash.update(old);
    hash.update(record);
    Ok(hash.finalize().into())
}

fn hex_decode(value: &str) -> io::Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(denied("invalid witness digest"));
    }
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| denied("invalid witness digest"))?;
    }
    Ok(out)
}

fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_binding(dir_path: &Path) -> io::Result<Binding> {
    let path = dir_path.join("binding.json");
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_file()
        || meta.uid() != 0
        || meta.mode() & 0o7777 != 0o600
        || meta.nlink() != 1
        || meta.len() > 4096
    {
        return Err(denied("invalid witness binding"));
    }
    let data = fs::read(path)?;
    let binding: Binding =
        serde_json::from_slice(&data).map_err(|_| denied("invalid witness binding"))?;
    if binding.schema_version != BINDING_VERSION {
        return Err(denied("unsupported witness binding version"));
    }
    for digest in [
        &binding.project_source_sha256,
        &binding.project_identity_sha256,
        &binding.project_lock_identity_sha256,
    ] {
        hex_decode(digest)?;
    }
    Ok(binding)
}

fn read_state(dir_path: &Path, binding: &Binding) -> io::Result<(usize, [u8; 32])> {
    let head_path = dir_path.join("head");
    let head_meta = fs::symlink_metadata(&head_path)?;
    if !head_meta.is_file()
        || head_meta.uid() != 0
        || head_meta.mode() & 0o7777 != 0o600
        || head_meta.nlink() != 1
        || head_meta.len() != 21
    {
        return Err(denied("invalid witness head"));
    }
    let head = fs::read(head_path)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(dir_path)? {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| denied("invalid witness entry"))?;
        if name != "binding.json" && name != "head" {
            if !name.starts_with("state-") || name.len() != 26 {
                return Err(denied("unknown witness entry"));
            }
            names.push(name);
        }
    }
    names.sort();
    if names.is_empty() || names.len() > 1025 {
        return Err(denied("missing or oversized witness state"));
    }
    if head != format!("{:020}\n", names.len() - 1).as_bytes() {
        return Err(denied("witness head and state files disagree"));
    }
    let binding_bytes = serde_json::to_vec(binding).map_err(io::Error::other)?;
    let mut seed = Sha256::new();
    seed.update(DOMAIN);
    seed.update(&binding_bytes);
    let genesis: [u8; 32] = seed.finalize().into();
    let mut latest = genesis;
    for (seq, name) in names.iter().enumerate() {
        if *name != state_name(seq) {
            return Err(denied("witness state gap"));
        }
        let path = dir_path.join(name);
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_file()
            || meta.uid() != 0
            || meta.mode() & 0o7777 != 0o600
            || meta.nlink() != 1
            || meta.len() != 65
        {
            return Err(denied("invalid witness state file"));
        }
        let bytes = fs::read(path)?;
        if bytes.len() != 65 || bytes[64] != b'\n' {
            return Err(denied("invalid witness state bytes"));
        }
        latest = hex_decode(
            std::str::from_utf8(&bytes[..64]).map_err(|_| denied("invalid witness state bytes"))?,
        )?;
        if seq == 0 && latest != genesis {
            return Err(denied("witness genesis changed"));
        }
    }
    Ok((names.len() - 1, latest))
}

fn initialize(
    dir_path: &Path,
    journal_path: &Path,
    project_path: &Path,
    broker_uid: u32,
    broker_gid: u32,
) -> io::Result<()> {
    let dir = check_state_dir(dir_path)?;
    lock(&dir)?;
    if broker_uid == 0
        || broker_gid == 0
        || !journal_path.is_absolute()
        || !project_path.is_absolute()
    {
        return Err(denied("invalid provisioning identity"));
    }
    // The journal's parent must be administrator owned so the broker cannot
    // swap the directory path while the witness uses its inode lock.
    check_root_path(
        journal_path
            .parent()
            .ok_or_else(|| denied("missing journal parent"))?,
    )?;
    if fs::read_dir(dir_path)?.next().is_some() {
        return Err(denied("witness directory already provisioned"));
    }
    let journal = Journal::open(journal_path, broker_uid)?;
    if !journal.read_verified()?.is_empty() {
        return Err(denied("provision only an empty journal"));
    }
    let project = WitnessProject::open(project_path, broker_uid)?;
    let (journal_dev, journal_ino) = metadata_id(journal_path)?;
    let (project_dev, project_ino) = metadata_id(project_path)?;
    let binding = Binding {
        schema_version: BINDING_VERSION.to_owned(),
        broker_uid,
        broker_gid,
        journal_path: journal_path.to_owned(),
        journal_dev,
        journal_ino,
        project_path: project_path.to_owned(),
        project_dev,
        project_ino,
        project_source_sha256: project.source_sha256().to_owned(),
        project_identity_sha256: project.identity_sha256().to_owned(),
        project_lock_identity_sha256: project.lock_identity_sha256().to_owned(),
    };
    let bytes = serde_json::to_vec(&binding).map_err(io::Error::other)?;
    let mut seed = Sha256::new();
    seed.update(DOMAIN);
    seed.update(&bytes);
    let initial: [u8; 32] = seed.finalize().into();
    write_new(&dir, &dir_path.join("binding.json"), &bytes)?;
    write_new(
        &dir,
        &dir_path.join(state_name(0)),
        format!("{}\n", hex(&initial)).as_bytes(),
    )?;
    write_new(&dir, &dir_path.join("head"), b"00000000000000000000\n")?;
    Ok(())
}

fn transition(dir_path: &Path, dir: &File, extend_requested: bool) -> io::Result<String> {
    let binding = read_binding(dir_path)?;
    check_root_path(
        binding
            .journal_path
            .parent()
            .ok_or_else(|| denied("missing journal parent"))?,
    )?;
    binding_check(&binding)?;
    let (sequence, observed) = read_state(dir_path, &binding)?;
    // The pinned project check also verifies every broker-provisioned lock entry.
    let store = ProjectStore::open(&binding.project_path).map_err(io::Error::other)?;
    let _project_lock = store.lock().map_err(io::Error::other)?;
    binding_check(&binding)?;
    let journal = Journal::open(&binding.journal_path, binding.broker_uid)?;
    journal.with_locked_hashes(|records| {
        binding_check(&binding)?;
        let mut expected = {
            let bytes = serde_json::to_vec(&binding).map_err(io::Error::other)?;
            let mut h = Sha256::new();
            h.update(DOMAIN);
            h.update(bytes);
            h.finalize().into()
        };
        if fs::read(dir_path.join(state_name(0)))? != format!("{}\n", hex(&expected)).as_bytes() {
            return Err(denied("witness genesis diverged"));
        }
        let mut matching = if expected == observed { Some(0) } else { None };
        for (index, (_, hash)) in records.iter().enumerate() {
            expected = extend(expected, hash)?;
            if index < sequence
                && fs::read(dir_path.join(state_name(index + 1)))?
                    != format!("{}\n", hex(&expected)).as_bytes()
            {
                return Err(denied("witness state history diverged"));
            }
            if expected == observed {
                matching = Some(index + 1);
            }
        }
        if matching != Some(sequence) {
            return Err(denied("witness state diverged from journal"));
        }
        if sequence == records.len() {
            return Ok(format!("EXACT {sequence}\n"));
        }
        if !extend_requested {
            return Ok(format!("STALE {sequence} {}\n", records.len()));
        }
        let next = extend(observed, &records[sequence].1)?;
        binding_check(&binding)?;
        write_new(
            dir,
            &dir_path.join(state_name(sequence + 1)),
            format!("{}\n", hex(&next)).as_bytes(),
        )?;
        write_head(dir, dir_path, sequence + 1)?;
        // A lost response is settled by rereading state; no duplicate extend.
        let (committed_seq, committed) = read_state(dir_path, &binding)?;
        if committed_seq != sequence + 1 || committed != next {
            return Err(denied("uncertain witness commit"));
        }
        binding_check(&binding)?;
        Ok(format!("EXTENDED {committed_seq}\n"))
    })
}

fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    if len as usize != std::mem::size_of::<libc::ucred>() {
        return Err(denied("peer credentials absent"));
    }
    Ok(cred.uid)
}

fn serve(dir_path: &Path, socket_path: &Path) -> io::Result<()> {
    let dir = check_state_dir(dir_path)?;
    lock(&dir)?;
    let binding = read_binding(dir_path)?;
    read_state(dir_path, &binding)?;
    check_root_path(
        socket_path
            .parent()
            .ok_or_else(|| denied("missing socket parent"))?,
    )?;
    if let Ok(meta) = fs::symlink_metadata(socket_path) {
        if !meta.file_type().is_socket() || meta.uid() != 0 {
            return Err(denied("foreign socket path exists"));
        }
        // The state-directory lock proves that no other witness instance is
        // serving this binding; a leftover root-owned socket is from a crash.
        fs::remove_file(socket_path)?;
    }
    let listener = UnixListener::bind(socket_path)?;
    let path = std::ffi::CString::new(socket_path.as_os_str().as_encoded_bytes())
        .map_err(|_| denied("invalid socket path"))?;
    if unsafe { libc::chown(path.as_ptr(), 0, binding.broker_gid) } < 0 {
        return Err(io::Error::last_os_error());
    }
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660))?;
    for accepted in listener.incoming() {
        let mut stream = accepted?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
        let result = (|| {
            if peer_uid(&stream)? != binding.broker_uid {
                return Err(denied("unauthorized peer"));
            }
            let mut request = [0u8; 1];
            stream.read_exact(&mut request)?;
            match request[0] {
                b'S' => transition(dir_path, &dir, false),
                b'E' => transition(dir_path, &dir, true),
                _ => Err(denied("unknown witness operation")),
            }
        })();
        let response = result.unwrap_or_else(|e| format!("DENIED {e}\n"));
        let _ = stream.write_all(response.as_bytes());
    }
    Ok(())
}

fn run() -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(denied("witness requires root identity"));
    }
    unsafe {
        libc::umask(0o077);
    }
    let args: Vec<_> = std::env::args_os().collect();
    match args.get(1).and_then(|s| s.to_str()) {
        Some("init") if args.len() == 7 => initialize(
            Path::new(&args[2]),
            Path::new(&args[3]),
            Path::new(&args[4]),
            args[5]
                .to_str()
                .ok_or_else(|| denied("invalid UID"))?
                .parse()
                .map_err(|_| denied("invalid UID"))?,
            args[6]
                .to_str()
                .ok_or_else(|| denied("invalid GID"))?
                .parse()
                .map_err(|_| denied("invalid GID"))?,
        ),
        Some("serve") if args.len() == 4 => serve(Path::new(&args[2]), Path::new(&args[3])),
        _ => Err(io::Error::new(
            ErrorKind::InvalidInput,
            "usage: archy-journal-witness init STATE JOURNAL PROJECT BROKER_UID BROKER_GID | serve STATE SOCKET",
        )),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("witness unavailable: {e}");
        std::process::exit(1);
    }
}
