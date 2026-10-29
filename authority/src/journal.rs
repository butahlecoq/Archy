//! Broker-private durable record primitive. No approval event or writer is exposed.
use crate::hex_digest;
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::Mutex;

const MAX_EVENTS: usize = 1024;
const MAX_PAYLOAD: usize = 16 * 1024;
const MAX_EVENT_BYTES: usize = 64 * 1024;
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    sequence: u64,
    previous_sha256: String,
    idempotency_key: String,
    payload: String,
}

impl Record {
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

pub struct Journal {
    directory: OwnedFd,
    owner: u32,
    mutex: Mutex<()>,
    sync_directory: fn(RawFd) -> io::Result<()>,
}

struct FileLock(RawFd);
impl FileLock {
    fn take(fd: RawFd) -> io::Result<Self> {
        if unsafe { libc::flock(fd, libc::LOCK_EX) } < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(fd))
        }
    }
}
impl Drop for FileLock {
    fn drop(&mut self) {
        unsafe { libc::flock(self.0, libc::LOCK_UN) };
    }
}

impl Journal {
    /// Open a dedicated directory. Production requires broker UID and mode 0700.
    pub fn open(path: &Path, owner: u32) -> io::Result<Self> {
        let path = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| io::Error::from(ErrorKind::InvalidInput))?;
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let directory = unsafe { OwnedFd::from_raw_fd(fd) };
        let metadata = File::from(directory.try_clone()?).metadata()?;
        if metadata.uid() != owner || metadata.permissions().mode() & 0o777 != 0o700 {
            return Err(denied("journal must be broker-owned mode 0700"));
        }
        let journal = Self {
            directory,
            owner,
            mutex: Mutex::new(()),
            sync_directory: sync_dir,
        };
        let _lock = FileLock::take(journal.directory.as_raw_fd())?;
        journal.recover_pending()?;
        sync_dir(journal.directory.as_raw_fd())?;
        journal.scan()?;
        Ok(journal)
    }

    /// Opaque records only. The authority service does not call this for decisions.
    /// A future authenticated writer must supply a broker-verified snapshot.
    pub fn append(&self, key: &str, payload: &str) -> io::Result<(u64, String)> {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || payload.is_empty()
            || payload.len() > MAX_PAYLOAD
        {
            return Err(io::Error::from(ErrorKind::InvalidInput));
        }
        let _mutex = self
            .mutex
            .lock()
            .map_err(|_| denied("journal lock poisoned"))?;
        let _lock = FileLock::take(self.directory.as_raw_fd())?;
        // Settles an earlier rename whose directory fsync failed before any new commit.
        (self.sync_directory)(self.directory.as_raw_fd())?;
        let events = self.scan()?;
        for (record, hash) in &events {
            if record.idempotency_key == key {
                return if record.payload == payload {
                    Ok((record.sequence, hash.clone()))
                } else {
                    Err(io::Error::from(ErrorKind::AlreadyExists))
                };
            }
        }
        if events.len() == MAX_EVENTS {
            return Err(io::Error::from(ErrorKind::StorageFull));
        }
        let record = Record {
            sequence: events.len() as u64 + 1,
            previous_sha256: events
                .last()
                .map_or(ZERO_HASH.to_owned(), |(_, hash)| hash.clone()),
            idempotency_key: key.to_owned(),
            payload: payload.to_owned(),
        };
        let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
        if bytes.len() > MAX_EVENT_BYTES {
            return Err(io::Error::from(ErrorKind::InvalidInput));
        }
        let hash = hex_digest(&bytes);
        let final_name = CString::new(format!("{:020}-{hash}.event", record.sequence)).unwrap();
        let mut nonce = [0u8; 16];
        File::open("/dev/urandom")?.read_exact(&mut nonce)?;
        let pending = CString::new(format!(".pending-{}", hex_digest(&nonce))).unwrap();
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                pending.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            if unsafe {
                libc::renameat2(
                    self.directory.as_raw_fd(),
                    pending.as_ptr(),
                    self.directory.as_raw_fd(),
                    final_name.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            } < 0
            {
                return Err(io::Error::last_os_error());
            }
            (self.sync_directory)(self.directory.as_raw_fd())?;
            Ok((record.sequence, hash))
        })();
        // A pre-rename failure leaves no event. Post-rename failure leaves a
        // valid event that exact replay can settle. Never remove the final name.
        unsafe { libc::unlinkat(self.directory.as_raw_fd(), pending.as_ptr(), 0) };
        result
    }

    pub fn read_verified(&self) -> io::Result<Vec<Record>> {
        let _mutex = self
            .mutex
            .lock()
            .map_err(|_| denied("journal lock poisoned"))?;
        let _lock = FileLock::take(self.directory.as_raw_fd())?;
        (self.sync_directory)(self.directory.as_raw_fd())?;
        Ok(self.scan()?.into_iter().map(|(record, _)| record).collect())
    }

    fn scan(&self) -> io::Result<Vec<(Record, String)>> {
        let path = format!("/proc/self/fd/{}", self.directory.as_raw_fd());
        let mut names = Vec::new();
        for entry in fs::read_dir(path)? {
            let name = entry?
                .file_name()
                .into_string()
                .map_err(|_| denied("invalid journal entry"))?;
            if !valid_name(&name) {
                return Err(denied("unknown or pending journal entry"));
            }
            names.push(name);
            if names.len() > MAX_EVENTS {
                return Err(denied("journal over capacity"));
            }
        }
        names.sort();
        let mut records = Vec::with_capacity(names.len());
        let mut previous = ZERO_HASH.to_owned();
        for name in names {
            let cname = CString::new(name.as_bytes()).unwrap();
            let fd = unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    cname.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let mut file = unsafe { File::from_raw_fd(fd) };
            let metadata = file.metadata()?;
            if !metadata.file_type().is_file()
                || metadata.permissions().mode() & 0o777 != 0o600
                || metadata.uid() != self.owner
                || metadata.len() == 0
                || metadata.len() > MAX_EVENT_BYTES as u64
            {
                return Err(denied("invalid journal file"));
            }
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            file.read_to_end(&mut bytes)?;
            if bytes.len() != metadata.len() as usize {
                return Err(denied("journal changed during read"));
            }
            let hash = hex_digest(&bytes);
            let record: Record =
                serde_json::from_slice(&bytes).map_err(|_| denied("invalid journal record"))?;
            if name != format!("{:020}-{hash}.event", record.sequence)
                || record.sequence != records.len() as u64 + 1
                || record.previous_sha256 != previous
                || record.idempotency_key.is_empty()
                || record.idempotency_key.len() > 128
                || !record
                    .idempotency_key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || record.payload.is_empty()
                || record.payload.len() > MAX_PAYLOAD
                || records.iter().any(|(old, _): &(Record, String)| {
                    old.idempotency_key == record.idempotency_key
                })
            {
                return Err(denied("journal chain or record invalid"));
            }
            previous = hash.clone();
            records.push((record, hash));
        }
        Ok(records)
    }

    fn recover_pending(&self) -> io::Result<()> {
        let path = format!("/proc/self/fd/{}", self.directory.as_raw_fd());
        let mut removed = false;
        for entry in fs::read_dir(path)? {
            let name = entry?
                .file_name()
                .into_string()
                .map_err(|_| denied("invalid journal entry"))?;
            if !name.starts_with(".pending-") {
                continue;
            }
            if name.len() != 73
                || !name.as_bytes()[9..]
                    .iter()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(denied("malformed pending journal entry"));
            }
            let cname = CString::new(name).unwrap();
            let fd = unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    cname.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let file = unsafe { File::from_raw_fd(fd) };
            let metadata = file.metadata()?;
            if !metadata.file_type().is_file()
                || metadata.permissions().mode() & 0o777 != 0o600
                || metadata.uid() != self.owner
                || metadata.len() > MAX_EVENT_BYTES as u64
            {
                return Err(denied("unsafe pending journal entry"));
            }
            if unsafe { libc::unlinkat(self.directory.as_raw_fd(), cname.as_ptr(), 0) } < 0 {
                return Err(io::Error::last_os_error());
            }
            removed = true;
        }
        if removed {
            sync_dir(self.directory.as_raw_fd())?;
        }
        Ok(())
    }
}

fn valid_name(name: &str) -> bool {
    name.len() == 91
        && name.as_bytes()[20] == b'-'
        && name.ends_with(".event")
        && name.as_bytes()[..20].iter().all(u8::is_ascii_digit)
        && name.as_bytes()[21..85]
            .iter()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn sync_dir(fd: RawFd) -> io::Result<()> {
    if unsafe { libc::fsync(fd) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn denied(message: &str) -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
mod durability_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SYNC_CALLS: AtomicUsize = AtomicUsize::new(0);
    fn fail_after_rename(fd: RawFd) -> io::Result<()> {
        if SYNC_CALLS.fetch_add(1, Ordering::SeqCst) == 1 {
            Err(io::Error::other(
                "injected post-rename directory sync failure",
            ))
        } else {
            sync_dir(fd)
        }
    }

    #[test]
    fn uncertain_commit_replay_settles_directory_before_success() {
        let path = std::env::temp_dir().join(format!(
            "archy-journal-sync-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let mut journal = Journal::open(&path, unsafe { libc::geteuid() }).unwrap();
        SYNC_CALLS.store(0, Ordering::SeqCst);
        journal.sync_directory = fail_after_rename;
        assert!(
            journal
                .append("probe-sync", "non-decision durability probe")
                .is_err()
        );
        assert_eq!(SYNC_CALLS.load(Ordering::SeqCst), 2);
        let replay = journal
            .append("probe-sync", "non-decision durability probe")
            .unwrap();
        assert_eq!(replay.0, 1);
        assert_eq!(
            Journal::open(&path, unsafe { libc::geteuid() })
                .unwrap()
                .read_verified()
                .unwrap()
                .len(),
            1
        );
        fs::remove_dir_all(path).unwrap();
    }
}
