use archy_core::{ArchyError, ErrorCode};
use rustix::fs::{self, FileType, StatVfsMountFlags};
use rustix::io::{Errno, FdFlags, fcntl_getfd};
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SocketType, recvmsg, sockopt,
};
use rustix::process::getuid;
use std::fs::File;
use std::io::IoSliceMut;
use std::mem::MaybeUninit;
use std::os::fd::AsFd;
use std::thread;
use std::time::{Duration, Instant};

const FRAME: &[u8] = b"ARCHY-SCRATCH-1";
const TMPFS_MAGIC: u64 = 0x0102_1994;
const MAX_SCRATCH_BYTES: u64 = 128 * 1024 * 1024;
const MAX_SCRATCH_INODES: u64 = 256;

#[derive(Debug)]
pub struct QuotaCheckedScratch {
    file: File,
    byte_capacity: u64,
    inode_capacity: u64,
}

impl QuotaCheckedScratch {
    pub fn as_file(&self) -> &File {
        &self.file
    }

    pub fn byte_capacity(&self) -> u64 {
        self.byte_capacity
    }

    pub fn inode_capacity(&self) -> u64 {
        self.inode_capacity
    }
}

fn unavailable(message: impl Into<String>) -> ArchyError {
    ArchyError::new(ErrorCode::BackendUnavailable, "scratch", message)
}

fn receive_frame(channel: &impl AsFd, deadline: Instant) -> Result<File, ArchyError> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(8));
    let socket_type = sockopt::socket_type(channel)
        .map_err(|e| unavailable(format!("control socket type: {e}")))?;
    if socket_type != SocketType::SEQPACKET {
        return Err(unavailable("control socket is not seqpacket"));
    }
    loop {
        if Instant::now() >= deadline {
            return Err(unavailable("control frame deadline"));
        }
        let mut frame = [0_u8; 64];
        let mut iov = [IoSliceMut::new(&mut frame)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        match recvmsg(
            channel,
            &mut iov,
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(message) => {
                let mut descriptors = Vec::new();
                let mut unexpected = false;
                for item in ancillary.drain() {
                    match item {
                        RecvAncillaryMessage::ScmRights(fds) => descriptors.extend(fds),
                        _ => unexpected = true,
                    }
                }
                if message
                    .flags
                    .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                    || unexpected
                    || descriptors.len() != 1
                {
                    return Err(unavailable("control frame requires exactly one descriptor"));
                }
                if message.bytes != FRAME.len() || &frame[..message.bytes] != FRAME {
                    return Err(unavailable("control frame marker mismatch"));
                }
                let descriptor = descriptors.pop().expect("length checked");
                let flags = fcntl_getfd(&descriptor)
                    .map_err(|e| unavailable(format!("descriptor flags: {e}")))?;
                if !flags.contains(FdFlags::CLOEXEC) {
                    return Err(unavailable("scratch descriptor lacks close-on-exec"));
                }
                return Ok(File::from(descriptor));
            }
            Err(Errno::AGAIN | Errno::INTR) => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(unavailable(format!("control receive: {error}"))),
        }
    }
}

fn check_quota(
    file: File,
    max_bytes: u64,
    max_inodes: u64,
) -> Result<QuotaCheckedScratch, ArchyError> {
    let metadata = fs::fstat(&file).map_err(|e| unavailable(format!("scratch metadata: {e}")))?;
    if !FileType::from_raw_mode(metadata.st_mode).is_dir() || metadata.st_uid != getuid().as_raw() {
        return Err(unavailable("scratch descriptor is not an owned directory"));
    }
    if metadata.st_mode & 0o7777 != 0o700 {
        return Err(unavailable("scratch directory private mode must be 0700"));
    }
    let filesystem =
        fs::fstatfs(&file).map_err(|e| unavailable(format!("scratch filesystem: {e}")))?;
    if filesystem.f_type as u64 != TMPFS_MAGIC {
        return Err(unavailable("scratch filesystem is not tmpfs"));
    }
    let stats = fs::fstatvfs(&file).map_err(|e| unavailable(format!("scratch quota: {e}")))?;
    let bytes = stats
        .f_blocks
        .checked_mul(stats.f_frsize)
        .ok_or_else(|| unavailable("scratch quota overflow"))?;
    if bytes == 0
        || bytes > max_bytes
        || stats.f_files == 0
        || stats.f_files > max_inodes
        || !stats.f_flag.contains(
            StatVfsMountFlags::NOSUID | StatVfsMountFlags::NODEV | StatVfsMountFlags::NOEXEC,
        )
    {
        return Err(unavailable(
            "scratch tmpfs quota or mount flags unavailable",
        ));
    }
    Ok(QuotaCheckedScratch {
        file,
        byte_capacity: bytes,
        inode_capacity: stats.f_files,
    })
}

pub fn receive_quota_checked_scratch(
    channel: &impl AsFd,
    deadline: Instant,
) -> Result<QuotaCheckedScratch, ArchyError> {
    let file = receive_frame(channel, deadline)?;
    check_quota(file, MAX_SCRATCH_BYTES, MAX_SCRATCH_INODES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::net::{
        AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, sendmsg,
        socketpair,
    };
    use std::io::IoSlice;

    #[test]
    fn exact_control_frame_transfers_one_cloexec_directory_before_quota_check() {
        let (receiver, sender) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let directory = File::open("/dev/shm").unwrap();
        let borrowed = [directory.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&borrowed)));
        sendmsg(
            &sender,
            &[IoSlice::new(FRAME)],
            &mut ancillary,
            SendFlags::empty(),
        )
        .unwrap();
        let file = receive_frame(&receiver, Instant::now() + Duration::from_secs(1)).unwrap();
        assert!(FileType::from_raw_mode(fs::fstat(file).unwrap().st_mode).is_dir());
    }
}
