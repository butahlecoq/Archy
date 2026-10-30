//! Broker-owned proposal ingress. There is deliberately no approval API.
pub mod bus_identity;
pub mod human_bus;
pub mod journal;
pub mod protected_project;
pub mod snapshot_worker;
use archy_core::decision_contract::Proposal;
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fs;
use std::io::{self, ErrorKind, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::Mutex;

pub const MAX_FRAME: usize = 64 * 1024;
pub const MAX_STORED_PROPOSALS: usize = 1024;
pub const PEER_TIMEOUT_SECONDS: i64 = 1;
pub const CLIENT_TIMEOUT_SECONDS: i64 = 2;
pub const HUMAN_WORKERS: usize = 8;
pub const OP_SUBMIT: u8 = 1;
pub const OP_READ: u8 = 2;
/// Reserved for a future system-bus human decision path; both seqpacket channels deny it.
pub const OP_DECIDE: u8 = 3;
pub const STATUS_OK: u8 = 0;
pub const STATUS_DENIED: u8 = 1;
pub const STATUS_INVALID: u8 = 2;
pub const STATUS_MISSING: u8 = 3;
pub const STATUS_FULL: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Agent,
    Human,
}

#[derive(Clone, Copy, Debug)]
pub struct Identities {
    pub agent: u32,
    pub broker: u32,
    pub human: u32,
}

impl Identities {
    pub fn distinct(self) -> io::Result<Self> {
        if self.agent == 0
            || self.broker == 0
            || self.human == 0
            || self.agent == self.broker
            || self.agent == self.human
            || self.broker == self.human
        {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "distinct non-root host UIDs required",
            ));
        }
        Ok(self)
    }
    pub fn permits(self, channel: Channel, peer: u32) -> bool {
        match channel {
            Channel::Agent => peer == self.agent,
            Channel::Human => peer == self.human,
        }
    }
}

pub fn named_uid(name: &str) -> io::Result<u32> {
    let name = CString::new(name).map_err(|_| io::Error::from(ErrorKind::InvalidInput))?;
    // The account database is administrator controlled on the target.
    let entry = unsafe { libc::getpwnam(name.as_ptr()) };
    if entry.is_null() {
        return Err(io::Error::new(
            ErrorKind::NotFound,
            "required account missing",
        ));
    }
    Ok(unsafe { (*entry).pw_uid })
}

pub fn enrolled_human(path: &Path) -> io::Result<u32> {
    let path = CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|_| io::Error::from(ErrorKind::InvalidInput))?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let metadata = fd_metadata(fd.as_raw_fd())?;
    if metadata.st_uid != 0
        || metadata.st_mode & 0o022 != 0
        || metadata.st_mode & libc::S_IFMT != libc::S_IFREG
    {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "enrollment must be root-owned regular file",
        ));
    }
    let mut data = String::new();
    fs::File::from(fd).take(33).read_to_string(&mut data)?;
    if data.len() > 32 {
        return Err(io::Error::from(ErrorKind::InvalidData));
    }
    data.trim_end_matches('\n')
        .parse::<u32>()
        .map_err(|_| io::Error::from(ErrorKind::InvalidData))
}

fn fd_metadata(fd: RawFd) -> io::Result<libc::stat> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { stat.assume_init() })
    }
}

pub struct Store {
    directory: OwnedFd,
    submissions: Mutex<()>,
    sync_directory: fn(RawFd) -> io::Result<()>,
}

fn sync_directory(fd: RawFd) -> io::Result<()> {
    if unsafe { libc::fsync(fd) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

impl Store {
    /// The caller supplies a broker-private directory. Production checks ownership and mode.
    pub fn open(path: &Path, broker_uid: Option<u32>) -> io::Result<Self> {
        let cpath = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| io::Error::from(ErrorKind::InvalidInput))?;
        let fd = unsafe {
            libc::open(
                cpath.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let directory = unsafe { OwnedFd::from_raw_fd(fd) };
        let metadata = fd_metadata(directory.as_raw_fd())?;
        if let Some(uid) = broker_uid
            && (metadata.st_uid != uid || metadata.st_mode & 0o777 != 0o700)
        {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "store must be broker-owned mode 0700",
            ));
        }
        Ok(Self {
            directory,
            submissions: Mutex::new(()),
            sync_directory,
        })
    }

    fn has_capacity(&self) -> Result<bool, u8> {
        // Count every entry, including abandoned temporary files. Fail closed on
        // unreadable directories and stop scanning as soon as the limit is hit.
        let path = format!("/proc/self/fd/{}", self.directory.as_raw_fd());
        let entries = fs::read_dir(path).map_err(|_| STATUS_DENIED)?;
        let mut count = 0;
        for entry in entries {
            entry.map_err(|_| STATUS_DENIED)?;
            count += 1;
            if count >= MAX_STORED_PROPOSALS {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn submit(&self, raw: &[u8]) -> Result<String, u8> {
        if raw.is_empty() || raw.len() >= MAX_FRAME {
            return Err(STATUS_INVALID);
        }
        let proposal: Proposal = serde_json::from_slice(raw).map_err(|_| STATUS_INVALID)?;
        proposal.validate().map_err(|_| STATUS_INVALID)?;
        let digest = hex_digest(raw);
        let name = CString::new(format!("{digest}.proposal")).unwrap();
        let _guard = self.submissions.lock().map_err(|_| STATUS_DENIED)?;
        if self.read(&digest).as_deref() == Ok(raw) {
            (self.sync_directory)(self.directory.as_raw_fd()).map_err(|_| STATUS_DENIED)?;
            return Ok(digest);
        }
        if !self.has_capacity()? {
            return Err(STATUS_FULL);
        }
        let mut nonce = [0u8; 16];
        fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut nonce))
            .map_err(|_| STATUS_DENIED)?;
        let temporary = CString::new(format!(".pending-{}", hex_digest(&nonce))).unwrap();
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(STATUS_DENIED);
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let result = (|| {
            let mut offset = 0;
            while offset < raw.len() {
                let written = unsafe {
                    libc::write(
                        fd.as_raw_fd(),
                        raw[offset..].as_ptr().cast(),
                        raw.len() - offset,
                    )
                };
                if written <= 0 {
                    return Err(STATUS_DENIED);
                }
                offset += written as usize;
            }
            if unsafe { libc::fsync(fd.as_raw_fd()) } < 0 {
                return Err(STATUS_DENIED);
            }
            let renamed = unsafe {
                libc::renameat2(
                    self.directory.as_raw_fd(),
                    temporary.as_ptr(),
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            };
            if renamed < 0 {
                return if io::Error::last_os_error().kind() == ErrorKind::AlreadyExists
                    && self.read(&digest).as_deref() == Ok(raw)
                    && (self.sync_directory)(self.directory.as_raw_fd()).is_ok()
                {
                    Ok(digest)
                } else {
                    Err(STATUS_DENIED)
                };
            }
            (self.sync_directory)(self.directory.as_raw_fd()).map_err(|_| STATUS_DENIED)?;
            Ok(digest)
        })();
        unsafe {
            libc::unlinkat(self.directory.as_raw_fd(), temporary.as_ptr(), 0);
        }
        result
    }

    pub fn read(&self, digest: &str) -> Result<Vec<u8>, u8> {
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(STATUS_INVALID);
        }
        let name = CString::new(format!("{digest}.proposal")).unwrap();
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(
                if io::Error::last_os_error().kind() == ErrorKind::NotFound {
                    STATUS_MISSING
                } else {
                    STATUS_DENIED
                },
            );
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let stat = fd_metadata(fd.as_raw_fd()).map_err(|_| STATUS_DENIED)?;
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG
            || stat.st_size < 1
            || stat.st_size as usize >= MAX_FRAME
            || stat.st_mode & 0o077 != 0
        {
            return Err(STATUS_DENIED);
        }
        let mut bytes = vec![0; stat.st_size as usize];
        let mut offset = 0;
        while offset < bytes.len() {
            let n = unsafe {
                libc::read(
                    fd.as_raw_fd(),
                    bytes[offset..].as_mut_ptr().cast(),
                    bytes.len() - offset,
                )
            };
            if n <= 0 {
                return Err(STATUS_DENIED);
            }
            offset += n as usize;
        }
        if hex_digest(&bytes) != digest {
            return Err(STATUS_DENIED);
        }
        Ok(bytes)
    }
}

pub fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        write!(out, "{b:02x}").unwrap();
    }
    out
}

/// Read one packet and reject truncation and all ancillary data.
pub fn receive(fd: RawFd) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0u8; MAX_FRAME + 1];
    let mut control = vec![0u8; 4096];
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = control.len();
    let n = unsafe { libc::recvmsg(fd, &mut msg, libc::MSG_CMSG_CLOEXEC) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    // Close passed descriptors before returning, including denied messages.
    unsafe {
        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        while !cmsg.is_null() {
            if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
                let count = ((*cmsg).cmsg_len as usize - libc::CMSG_LEN(0) as usize)
                    / std::mem::size_of::<RawFd>();
                for i in 0..count {
                    libc::close(*libc::CMSG_DATA(cmsg).cast::<RawFd>().add(i));
                }
            }
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
        }
    }
    if msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
        || msg.msg_controllen != 0
        || n == 0
        || n as usize > MAX_FRAME
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "truncated or ancillary packet",
        ));
    }
    bytes.truncate(n as usize);
    Ok(bytes)
}

pub fn send(fd: RawFd, bytes: &[u8]) -> io::Result<()> {
    let n = unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), libc::MSG_NOSIGNAL) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    if n as usize != bytes.len() {
        return Err(io::Error::from(ErrorKind::WriteZero));
    }
    Ok(())
}

pub fn set_peer_timeouts(fd: RawFd) -> io::Result<()> {
    set_timeouts(fd, PEER_TIMEOUT_SECONDS)
}

pub fn set_client_timeouts(fd: RawFd) -> io::Result<()> {
    set_timeouts(fd, CLIENT_TIMEOUT_SECONDS)
}

fn set_timeouts(fd: RawFd, seconds: i64) -> io::Result<()> {
    let timeout = libc::timeval {
        tv_sec: seconds,
        tv_usec: 0,
    };
    for option in [libc::SO_RCVTIMEO, libc::SO_SNDTIMEO] {
        if unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                option,
                (&timeout as *const libc::timeval).cast(),
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub fn connect_local(fd: RawFd, path: &[u8]) -> io::Result<()> {
    if path.len() >= 108 || path.contains(&0) {
        return Err(io::Error::from(ErrorKind::InvalidInput));
    }
    set_client_timeouts(fd)?;
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in addr.sun_path.iter_mut().zip(path) {
        *slot = *byte as i8;
    }
    let result = unsafe {
        libc::connect(
            fd,
            (&addr as *const libc::sockaddr_un).cast(),
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        )
    };
    let connected = if result == 0 {
        Ok(())
    } else {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            Err(error)
        } else {
            let mut pollfd = libc::pollfd {
                fd,
                events: libc::POLLOUT,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut pollfd, 1, 2000) };
            if ready <= 0 {
                Err(if ready == 0 {
                    io::Error::from(ErrorKind::TimedOut)
                } else {
                    io::Error::last_os_error()
                })
            } else {
                let mut socket_error = 0;
                let mut len = std::mem::size_of::<i32>() as libc::socklen_t;
                if unsafe {
                    libc::getsockopt(
                        fd,
                        libc::SOL_SOCKET,
                        libc::SO_ERROR,
                        (&mut socket_error as *mut i32).cast(),
                        &mut len,
                    )
                } < 0
                {
                    Err(io::Error::last_os_error())
                } else if socket_error != 0 {
                    Err(io::Error::from_raw_os_error(socket_error))
                } else {
                    Ok(())
                }
            }
        }
    };
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags) } < 0 {
        return Err(io::Error::last_os_error());
    }
    connected
}

pub fn peer_uid(fd: RawFd) -> io::Result<u32> {
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd,
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
        return Err(io::Error::from(ErrorKind::InvalidData));
    }
    Ok(cred.uid)
}

pub fn handle_connection(
    fd: RawFd,
    channel: Channel,
    identities: Identities,
    store: &Store,
) -> io::Result<()> {
    if !identities.permits(channel, peer_uid(fd)?) {
        return send(fd, &[STATUS_DENIED]);
    }
    let frame = match receive(fd) {
        Ok(f) => f,
        Err(_) => return send(fd, &[STATUS_INVALID]),
    };
    let (&op, body) = frame
        .split_first()
        .ok_or_else(|| io::Error::from(ErrorKind::InvalidData))?;
    let result = match (channel, op) {
        (Channel::Agent, OP_SUBMIT) => store.submit(body).map(|s| s.into_bytes()),
        (Channel::Human, OP_READ) => std::str::from_utf8(body)
            .map_err(|_| STATUS_INVALID)
            .and_then(|s| store.read(s)),
        (_, OP_DECIDE) => Err(STATUS_DENIED),
        _ => Err(STATUS_DENIED),
    };
    match result {
        Ok(data) => {
            let mut packet = Vec::with_capacity(data.len() + 1);
            packet.push(STATUS_OK);
            packet.extend(data);
            send(fd, &packet)
        }
        Err(status) => send(fd, &[status]),
    }
}

pub fn accept_and_handle(
    listener: RawFd,
    channel: Channel,
    identities: Identities,
    store: &Store,
) -> io::Result<()> {
    let fd = unsafe {
        libc::accept4(
            listener,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            libc::SOCK_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let connection = unsafe { OwnedFd::from_raw_fd(fd) };
    set_peer_timeouts(connection.as_raw_fd())?;
    // A malformed or stalled peer is local to this channel.
    let _ = handle_connection(connection.as_raw_fd(), channel, identities, store);
    Ok(())
}

pub fn check_listener(fd: RawFd, expected_path: &str) -> io::Result<()> {
    let mut kind = 0i32;
    let mut len = std::mem::size_of::<i32>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut kind as *mut i32).cast(),
            &mut len,
        )
    } < 0
        || kind != libc::SOCK_SEQPACKET
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "seqpacket listener required",
        ));
    }
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    let mut alen = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
    if unsafe { libc::getsockname(fd, (&mut addr as *mut libc::sockaddr_un).cast(), &mut alen) } < 0
        || addr.sun_family as i32 != libc::AF_UNIX
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "Unix listener required",
        ));
    }
    let actual = addr
        .sun_path
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect::<Vec<_>>();
    if actual != expected_path.as_bytes() {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "unexpected listener path",
        ));
    }
    let mut accepting = 0i32;
    let mut olen = std::mem::size_of::<i32>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_ACCEPTCONN,
            (&mut accepting as *mut i32).cast(),
            &mut olen,
        )
    } < 0
        || accepting != 1
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "listening socket required",
        ));
    }
    Ok(())
}

pub fn check_store_path(path: &Path, broker_uid: u32) -> io::Result<Store> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != broker_uid
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(io::Error::from(ErrorKind::PermissionDenied));
    }
    Store::open(path, Some(broker_uid))
}

#[cfg(test)]
mod durability_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    mod common {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/mod.rs"));
    }

    static SYNC_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn fail_two_syncs(fd: RawFd) -> io::Result<()> {
        if SYNC_CALLS.fetch_add(1, Ordering::SeqCst) < 2 {
            Err(io::Error::other("injected directory sync failure"))
        } else {
            sync_directory(fd)
        }
    }

    #[test]
    fn uncertain_rename_requires_successful_directory_sync_before_replay_ack() {
        let path = std::env::temp_dir().join(format!(
            "archy-uncertain-sync-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir(&path).unwrap();
        let raw = common::valid();
        let digest = hex_digest(&raw);
        let mut store = Store::open(&path, None).unwrap();
        SYNC_CALLS.store(0, Ordering::SeqCst);
        store.sync_directory = fail_two_syncs;
        assert_eq!(store.submit(&raw), Err(STATUS_DENIED));
        assert_eq!(store.read(&digest), Ok(raw.clone()));
        assert_eq!(SYNC_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(store.submit(&raw), Err(STATUS_DENIED));
        assert_eq!(SYNC_CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(store.submit(&raw), Ok(digest.clone()));
        assert_eq!(SYNC_CALLS.load(Ordering::SeqCst), 3);
        drop(store);
        let reopened = Store::open(&path, None).unwrap();
        assert_eq!(reopened.submit(&raw), Ok(digest));
        fs::remove_dir_all(path).unwrap();
    }
}
