//! Fail-closed peer check for credentials returned by GetConnectionCredentials.
//! The caller must pass the method header's bus-assigned unique sender, not
//! `Connection::peer_creds()` (which describes the broker's bus peer).
use std::fs;
use std::io::{self, ErrorKind};
use std::os::fd::{AsRawFd, OwnedFd};

fn denied(reason: &'static str) -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, reason)
}

/// This is process identity only. A logind active/local session and a fresh
/// Polkit challenge are separate prerequisites for a human decision.
pub struct PinnedBusPeer {
    sender: String,
    uid: u32,
    pid: u32,
    process: OwnedFd,
}

impl PinnedBusPeer {
    /// `process` must be the ProcessFD returned by the bus for `sender`.
    /// Numeric PID alone is never accepted, because it can be recycled.
    pub fn from_bus_credentials(
        sender: &str,
        uid: Option<u32>,
        pid: Option<u32>,
        process: Option<OwnedFd>,
        enrolled_uid: u32,
        agent_uid: u32,
        broker_uid: u32,
    ) -> io::Result<Self> {
        if !sender.starts_with(':')
            || sender.len() > 255
            || !sender
                .bytes()
                .skip(1)
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
            || sender.len() < 3
        {
            return Err(denied("unique bus sender required"));
        }
        let uid = uid.ok_or_else(|| denied("bus UID unavailable"))?;
        if uid == 0 || uid != enrolled_uid || uid == agent_uid || uid == broker_uid {
            return Err(denied("bus UID is not enrolled human"));
        }
        let pid = pid
            .filter(|p| *p != 0)
            .ok_or_else(|| denied("bus PID unavailable"))?;
        let process = process.ok_or_else(|| denied("bus ProcessFD unavailable"))?;
        let peer = Self {
            sender: sender.to_owned(),
            uid,
            pid,
            process,
        };
        peer.check_alive()?;
        Ok(peer)
    }

    pub fn sender(&self) -> &str {
        &self.sender
    }
    pub fn uid(&self) -> u32 {
        self.uid
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Check the kernel's pidfd target and exit status before and after each
    /// numeric-PID logind lookup. The lookup still requires a native race
    /// audit; this function alone is not session authentication.
    pub fn check_alive(&self) -> io::Result<()> {
        let fd = self.process.as_raw_fd();
        let link = fs::read_link(format!("/proc/self/fd/{fd}"))?;
        if link.to_str() != Some("anon_inode:[pidfd]") {
            return Err(denied("ProcessFD is not a pidfd"));
        }
        let info = fs::read_to_string(format!("/proc/self/fdinfo/{fd}"))?;
        let target = info
            .lines()
            .find_map(|line| line.strip_prefix("Pid:\t"))
            .and_then(|value| value.trim().parse::<u32>().ok());
        if target != Some(self.pid) {
            return Err(denied("bus PID and ProcessFD differ"));
        }
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut pfd, 1, 0) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        if result != 0 || pfd.revents != 0 {
            return Err(denied("bus process exited or pidfd invalid"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::FromRawFd;

    fn self_pidfd() -> OwnedFd {
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0) };
        assert!(fd >= 0, "pidfd required for this Linux fixture");
        unsafe { OwnedFd::from_raw_fd(fd as i32) }
    }

    #[test]
    fn kernel_pidfd_binds_bus_claim_and_rejects_missing_or_forged_credentials() {
        let uid = unsafe { libc::geteuid() };
        let pid = std::process::id();
        let valid = || {
            PinnedBusPeer::from_bus_credentials(
                ":1.42",
                Some(uid),
                Some(pid),
                Some(self_pidfd()),
                uid,
                uid.wrapping_add(1),
                uid.wrapping_add(2),
            )
        };
        assert!(valid().unwrap().check_alive().is_ok());
        assert!(
            PinnedBusPeer::from_bus_credentials(
                "org.archy.Client",
                Some(uid),
                Some(pid),
                Some(self_pidfd()),
                uid,
                uid + 1,
                uid + 2
            )
            .is_err()
        );
        assert!(
            PinnedBusPeer::from_bus_credentials(
                ":1.42",
                None,
                Some(pid),
                Some(self_pidfd()),
                uid,
                uid + 1,
                uid + 2
            )
            .is_err()
        );
        assert!(
            PinnedBusPeer::from_bus_credentials(
                ":1.42",
                Some(uid),
                Some(pid),
                None,
                uid,
                uid + 1,
                uid + 2
            )
            .is_err()
        );
        assert!(
            PinnedBusPeer::from_bus_credentials(
                ":1.42",
                Some(uid),
                Some(pid + 1),
                Some(self_pidfd()),
                uid,
                uid + 1,
                uid + 2
            )
            .is_err()
        );
        assert!(
            PinnedBusPeer::from_bus_credentials(
                ":1.42",
                Some(uid),
                Some(pid),
                Some(std::fs::File::open("/dev/null").unwrap().into()),
                uid,
                uid + 1,
                uid + 2
            )
            .is_err()
        );
        assert!(
            PinnedBusPeer::from_bus_credentials(
                ":1.42",
                Some(uid),
                Some(pid),
                Some(self_pidfd()),
                uid + 1,
                uid + 2,
                uid + 3
            )
            .is_err()
        );
    }
}
