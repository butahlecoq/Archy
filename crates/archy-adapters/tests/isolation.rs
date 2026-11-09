use archy_adapters::receive_quota_checked_scratch;
use archy_core::ErrorCode;
use rustix::net::{
    AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType,
    sendmsg, socketpair,
};
use std::fs::File;
use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::AsFd;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn pair() -> (std::os::fd::OwnedFd, std::os::fd::OwnedFd) {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap()
}

fn send(sender: &impl AsFd, frame: &[u8], files: &[&File]) {
    let borrowed: Vec<_> = files.iter().map(|file| file.as_fd()).collect();
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !borrowed.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&borrowed)));
    }
    assert_eq!(
        sendmsg(
            sender,
            &[IoSlice::new(frame)],
            &mut ancillary,
            SendFlags::empty()
        )
        .unwrap(),
        frame.len()
    );
}

#[test]
fn rejects_extra_descriptors_and_wrong_frame() {
    let directory = File::open("/dev/shm").unwrap();
    let (receiver, sender) = pair();
    send(&sender, b"ARCHY-SCRATCH-1", &[&directory, &directory]);
    let error = receive_quota_checked_scratch(&receiver, Instant::now() + Duration::from_secs(1))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::BackendUnavailable);
    assert!(error.message.contains("one descriptor"));

    let (receiver, sender) = pair();
    send(&sender, b"wrong-frame", &[&directory]);
    let error = receive_quota_checked_scratch(&receiver, Instant::now() + Duration::from_secs(1))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::BackendUnavailable);
    assert!(error.message.contains("frame"));
}

#[test]
fn rejects_non_tmpfs_and_deadline() {
    let root = std::env::temp_dir().join(format!("archy-isolation-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let directory = File::open(&root).unwrap();
    let (receiver, sender) = pair();
    send(&sender, b"ARCHY-SCRATCH-1", &[&directory]);
    let error = receive_quota_checked_scratch(&receiver, Instant::now() + Duration::from_secs(1))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::BackendUnavailable);
    assert!(error.message.contains("tmpfs") || error.message.contains("quota"));
    std::fs::remove_dir(root).unwrap();

    let (receiver, _sender) = pair();
    let error = receive_quota_checked_scratch(&receiver, Instant::now()).unwrap_err();
    assert_eq!(error.code, ErrorCode::BackendUnavailable);
    assert!(error.message.contains("deadline"));
}

#[test]
fn rejects_missing_descriptor_and_regular_file() {
    let (receiver, sender) = pair();
    send(&sender, b"ARCHY-SCRATCH-1", &[]);
    let error = receive_quota_checked_scratch(&receiver, Instant::now() + Duration::from_secs(1))
        .unwrap_err();
    assert!(error.message.contains("one descriptor"));

    let regular = File::open("Cargo.toml").unwrap();
    let (receiver, sender) = pair();
    send(&sender, b"ARCHY-SCRATCH-1", &[&regular]);
    let error = receive_quota_checked_scratch(&receiver, Instant::now() + Duration::from_secs(1))
        .unwrap_err();
    assert!(error.message.contains("owned directory"));
}

#[test]
fn passed_directory_requires_exact_private_mode() {
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    let scratch =
        Scratch(std::env::temp_dir().join(format!("archy-isolation-mode-{}", std::process::id())));
    std::fs::create_dir(&scratch.0).unwrap();
    let directory = File::open(&scratch.0).unwrap();
    for mode in [0o700, 0o777, 0o750, 0o701, 0o1700] {
        directory
            .set_permissions(std::fs::Permissions::from_mode(mode))
            .unwrap();
        assert_eq!(
            directory.metadata().unwrap().permissions().mode() & 0o7777,
            mode
        );
        let (receiver, sender) = pair();
        send(&sender, b"ARCHY-SCRATCH-1", &[&directory]);
        let result =
            receive_quota_checked_scratch(&receiver, Instant::now() + Duration::from_secs(1));
        if mode == 0o700 {
            if let Err(error) = result {
                assert!(
                    error.message.contains("tmpfs") || error.message.contains("quota"),
                    "private directory rejected before filesystem checks: {error}"
                );
            }
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, ErrorCode::BackendUnavailable);
            assert!(
                error.message.contains("private mode"),
                "mode {mode:o}: {error}"
            );
        }
    }
}
