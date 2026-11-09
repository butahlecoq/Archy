use archy_authority::*;
mod common;
use common::valid;
use std::ffi::CString;
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    path: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "archy-authority-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self {
            store: Store::open(&path, None).unwrap(),
            path,
        }
    }
    fn count(&self) -> usize {
        fs::read_dir(&self.path)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".proposal")
            })
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

fn pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [0; 2];
    assert_eq!(
        unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        },
        0
    );
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn listener(path: &std::path::Path) -> OwnedFd {
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(fd >= 0);
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    let path = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
    assert!(path.as_bytes().len() < addr.sun_path.len());
    for (slot, byte) in addr.sun_path.iter_mut().zip(path.as_bytes()) {
        *slot = *byte as i8;
    }
    assert_eq!(
        unsafe {
            libc::bind(
                fd,
                (&addr as *const libc::sockaddr_un).cast(),
                std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
            )
        },
        0
    );
    assert_eq!(unsafe { libc::listen(fd, 16) }, 0);
    socket
}

fn exchange(f: &Fixture, channel: Channel, ids: Identities, request: &[u8]) -> Vec<u8> {
    let (client, server) = pair();
    send(client.as_raw_fd(), request).unwrap();
    handle_connection(server.as_raw_fd(), channel, ids, &f.store).unwrap();
    receive(client.as_raw_fd()).unwrap()
}

#[test]
fn real_socket_peer_policy_and_read_only_review() {
    let f = Fixture::new();
    let uid = unsafe { libc::geteuid() };
    let wrong = Identities {
        agent: uid.wrapping_add(1),
        broker: uid.wrapping_add(2),
        human: uid.wrapping_add(3),
    };
    let mut request = vec![OP_SUBMIT];
    request.extend(valid());
    assert_eq!(
        exchange(&f, Channel::Agent, wrong, &request),
        [STATUS_DENIED]
    );
    assert_eq!(
        exchange(&f, Channel::Human, wrong, &request),
        [STATUS_DENIED]
    );
    assert_eq!(f.count(), 0);

    // This test only exercises the protocol. A production service rejects equal UIDs.
    let local = Identities {
        agent: uid,
        broker: uid,
        human: uid,
    };
    assert!(local.distinct().is_err());
    assert_eq!(
        exchange(&f, Channel::Human, local, &request),
        [STATUS_DENIED]
    );
    let accepted = exchange(&f, Channel::Agent, local, &request);
    assert_eq!(accepted[0], STATUS_OK);
    let digest = std::str::from_utf8(&accepted[1..]).unwrap();
    assert_eq!(digest, hex_digest(&valid()));
    assert_eq!(f.count(), 1);
    assert_eq!(exchange(&f, Channel::Agent, local, &request), accepted);
    assert_eq!(f.count(), 1);
    let mut read = vec![OP_READ];
    read.extend(digest.as_bytes());
    assert_eq!(exchange(&f, Channel::Agent, local, &read), [STATUS_DENIED]);
    let got = exchange(&f, Channel::Human, local, &read);
    assert_eq!(got[0], STATUS_OK);
    assert_eq!(&got[1..], valid());
    assert_eq!(
        exchange(&f, Channel::Human, local, &[OP_DECIDE]),
        [STATUS_DENIED]
    );
    assert_eq!(
        exchange(&f, Channel::Agent, local, &[OP_DECIDE]),
        [STATUS_DENIED]
    );
    assert_eq!(f.count(), 1);
}

#[test]
fn claims_paths_and_malformed_frames_never_create_proposals() {
    let f = Fixture::new();
    let uid = unsafe { libc::geteuid() };
    let ids = Identities {
        agent: uid,
        broker: uid,
        human: uid,
    };
    let mut forged: serde_json::Value = serde_json::from_slice(&valid()).unwrap();
    forged["actor"] = "human".into();
    for body in [
        serde_json::to_vec(&forged).unwrap(),
        b"{\"schema_version\":\"x\"}".to_vec(),
        vec![b'a'; MAX_FRAME],
    ] {
        let mut packet = vec![OP_SUBMIT];
        packet.extend(body);
        assert_eq!(exchange(&f, Channel::Agent, ids, &packet), [STATUS_INVALID]);
    }
    assert_eq!(
        exchange(&f, Channel::Human, ids, &[OP_READ, b'.', b'.', b'/']),
        [STATUS_INVALID]
    );
    assert_eq!(f.count(), 0);
}

#[test]
fn passed_descriptor_and_truncated_packet_are_denied() {
    let f = Fixture::new();
    let uid = unsafe { libc::geteuid() };
    let ids = Identities {
        agent: uid,
        broker: uid,
        human: uid,
    };
    let (client, server) = pair();
    let file = fs::File::open("/dev/null").unwrap();
    let marker = [OP_SUBMIT, b'X'];
    let mut iov = libc::iovec {
        iov_base: marker.as_ptr() as *mut _,
        iov_len: marker.len(),
    };
    let mut control = [0usize; 8];
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = unsafe { libc::CMSG_SPACE(std::mem::size_of::<i32>() as u32) } as usize;
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as usize;
        *libc::CMSG_DATA(cmsg).cast::<i32>() = file.as_raw_fd();
        assert_eq!(
            libc::sendmsg(client.as_raw_fd(), &msg, 0),
            marker.len() as isize
        );
    }
    handle_connection(server.as_raw_fd(), Channel::Agent, ids, &f.store).unwrap();
    assert_eq!(receive(client.as_raw_fd()).unwrap(), [STATUS_INVALID]);
    assert_eq!(f.count(), 0);
    let (client, server) = pair();
    send(client.as_raw_fd(), &vec![b'X'; MAX_FRAME + 1]).unwrap();
    handle_connection(server.as_raw_fd(), Channel::Agent, ids, &f.store).unwrap();
    assert_eq!(receive(client.as_raw_fd()).unwrap(), [STATUS_INVALID]);
    assert_eq!(f.count(), 0);
}

#[test]
fn damaged_proposal_bytes_are_not_served() {
    let f = Fixture::new();
    let digest = f.store.submit(&valid()).unwrap();
    fs::write(f.path.join(format!("{digest}.proposal")), b"tampered").unwrap();
    assert_eq!(f.store.read(&digest), Err(STATUS_DENIED));
}

#[test]
fn store_never_follows_a_proposal_symlink() {
    let f = Fixture::new();
    let digest = hex_digest(&valid());
    let outside = f.path.with_extension("outside");
    fs::write(&outside, b"outside sentinel").unwrap();
    symlink(&outside, f.path.join(format!("{digest}.proposal"))).unwrap();
    assert_eq!(f.store.submit(&valid()), Err(STATUS_DENIED));
    assert_eq!(f.store.read(&digest), Err(STATUS_DENIED));
    assert_eq!(fs::read(&outside).unwrap(), b"outside sentinel");
    fs::remove_file(outside).unwrap();
}

#[test]
fn a_connected_socket_is_not_a_systemd_listener() {
    let (client, _) = pair();
    assert!(check_listener(client.as_raw_fd(), "/run/archy/agent.sock").is_err());
}

#[test]
fn capacity_denies_new_proposals_but_keeps_replay_and_reads_available() {
    let f = Fixture::new();
    let raw = valid();
    let digest = f.store.submit(&raw).unwrap();
    for n in 1..MAX_STORED_PROPOSALS {
        fs::write(f.path.join(format!("occupied-{n}")), b"").unwrap();
    }
    let mut different = raw.clone();
    different.push(b' ');
    assert_eq!(f.store.submit(&different), Err(STATUS_FULL));
    assert_eq!(f.store.submit(&raw), Ok(digest.clone()));
    assert_eq!(f.store.read(&digest), Ok(raw));
    assert_eq!(fs::read_dir(&f.path).unwrap().count(), MAX_STORED_PROPOSALS);
}

#[test]
fn stalled_agent_does_not_hold_the_human_channel() {
    let f = Fixture::new();
    let uid = unsafe { libc::geteuid() };
    let ids = Identities {
        agent: uid,
        broker: uid,
        human: uid,
    };
    let digest = f.store.submit(&valid()).unwrap();
    let (agent_client, agent_server) = pair();
    let (human_client, human_server) = pair();
    let started = Instant::now();
    std::thread::scope(|scope| {
        let stalled = scope.spawn(|| {
            set_peer_timeouts(agent_server.as_raw_fd()).unwrap();
            handle_connection(agent_server.as_raw_fd(), Channel::Agent, ids, &f.store).unwrap();
        });
        let mut request = vec![OP_READ];
        request.extend(digest.as_bytes());
        send(human_client.as_raw_fd(), &request).unwrap();
        set_peer_timeouts(human_server.as_raw_fd()).unwrap();
        handle_connection(human_server.as_raw_fd(), Channel::Human, ids, &f.store).unwrap();
        let response = receive(human_client.as_raw_fd()).unwrap();
        assert_eq!(response[0], STATUS_OK);
        assert_eq!(&response[1..], valid());
        assert!(started.elapsed() < Duration::from_millis(800));
        stalled.join().unwrap();
    });
    assert_eq!(receive(agent_client.as_raw_fd()).unwrap(), [STATUS_INVALID]);
    assert!(started.elapsed() >= Duration::from_millis(800));
    let mut request = vec![OP_SUBMIT];
    request.extend(valid());
    let recovered = exchange(&f, Channel::Agent, ids, &request);
    assert_eq!(recovered[0], STATUS_OK);
}

#[test]
fn timeout_setup_rejects_non_socket() {
    let file = fs::File::open("/dev/null").unwrap();
    assert!(set_peer_timeouts(file.as_raw_fd()).is_err());
    assert!(set_client_timeouts(file.as_raw_fd()).is_err());
}

#[test]
fn client_receive_timeout_allows_a_second_broker_turn() {
    let (client, _server) = pair();
    set_client_timeouts(client.as_raw_fd()).unwrap();
    let started = Instant::now();
    assert!(receive(client.as_raw_fd()).is_err());
    assert!(started.elapsed() >= Duration::from_millis(1800));
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn accepted_stalled_client_times_out_then_listener_recovers() {
    let f = Fixture::new();
    let path = f.path.join("agent.sock");
    let listener = listener(&path);
    let uid = unsafe { libc::geteuid() };
    let ids = Identities {
        agent: uid,
        broker: uid,
        human: uid,
    };
    let client_fd =
        unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(client_fd >= 0);
    let client = unsafe { OwnedFd::from_raw_fd(client_fd) };
    connect_local(client.as_raw_fd(), path.as_os_str().as_encoded_bytes()).unwrap();
    let started = Instant::now();
    accept_and_handle(listener.as_raw_fd(), Channel::Agent, ids, &f.store).unwrap();
    assert_eq!(receive(client.as_raw_fd()).unwrap(), [STATUS_INVALID]);
    assert!(started.elapsed() >= Duration::from_millis(800));
    assert!(started.elapsed() < Duration::from_secs(3));
    let next_fd =
        unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(next_fd >= 0);
    let next = unsafe { OwnedFd::from_raw_fd(next_fd) };
    connect_local(next.as_raw_fd(), path.as_os_str().as_encoded_bytes()).unwrap();
    let mut request = vec![OP_SUBMIT];
    request.extend(valid());
    send(next.as_raw_fd(), &request).unwrap();
    accept_and_handle(listener.as_raw_fd(), Channel::Agent, ids, &f.store).unwrap();
    assert_eq!(receive(next.as_raw_fd()).unwrap()[0], STATUS_OK);
}

#[test]
fn serial_human_handler_reproduces_four_peer_client_timeout() {
    let f = Fixture::new();
    let path = f.path.join("human-serial.sock");
    let listener = listener(&path);
    let uid = unsafe { libc::geteuid() };
    let ids = Identities {
        agent: uid,
        broker: uid,
        human: uid,
    };
    let digest = f.store.submit(&valid()).unwrap();
    let connect = || {
        let fd =
            unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
        assert!(fd >= 0);
        let peer = unsafe { OwnedFd::from_raw_fd(fd) };
        connect_local(peer.as_raw_fd(), path.as_os_str().as_encoded_bytes()).unwrap();
        peer
    };
    let _silent: Vec<_> = (0..4).map(|_| connect()).collect();
    let reader = connect();
    let mut request = vec![OP_READ];
    request.extend(digest.as_bytes());
    send(reader.as_raw_fd(), &request).unwrap();
    set_peer_timeouts(reader.as_raw_fd()).unwrap();
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            for _ in 0..5 {
                accept_and_handle(listener.as_raw_fd(), Channel::Human, ids, &f.store).unwrap();
            }
        });
        let started = Instant::now();
        assert_eq!(
            receive(reader.as_raw_fd()).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert!(started.elapsed() >= Duration::from_millis(800));
        worker.join().unwrap();
    });
    let response = receive(reader.as_raw_fd()).unwrap();
    assert_eq!(response[0], STATUS_OK);
    assert_eq!(&response[1..], valid());
}

#[test]
fn four_queued_silent_human_peers_do_not_delay_a_valid_read() {
    let f = Fixture::new();
    let path = f.path.join("human.sock");
    let listener = listener(&path);
    let uid = unsafe { libc::geteuid() };
    let ids = Identities {
        agent: uid,
        broker: uid,
        human: uid,
    };
    let digest = f.store.submit(&valid()).unwrap();
    let connect = || {
        let fd =
            unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
        assert!(fd >= 0);
        let peer = unsafe { OwnedFd::from_raw_fd(fd) };
        connect_local(peer.as_raw_fd(), path.as_os_str().as_encoded_bytes()).unwrap();
        peer
    };
    let silent: Vec<_> = (0..4).map(|_| connect()).collect();
    let reader = connect();
    let mut request = vec![OP_READ];
    request.extend(digest.as_bytes());
    send(reader.as_raw_fd(), &request).unwrap();
    let extras: Vec<_> = (5..HUMAN_WORKERS)
        .map(|_| {
            let peer = connect();
            send(peer.as_raw_fd(), &[3]).unwrap();
            peer
        })
        .collect();
    let started = Instant::now();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..HUMAN_WORKERS)
            .map(|_| {
                scope.spawn(|| {
                    accept_and_handle(listener.as_raw_fd(), Channel::Human, ids, &f.store).unwrap()
                })
            })
            .collect();
        let response = receive(reader.as_raw_fd()).unwrap();
        assert_eq!(response[0], STATUS_OK);
        assert_eq!(&response[1..], valid());
        assert!(started.elapsed() < Duration::from_millis(800));
        for peer in &extras {
            assert_eq!(receive(peer.as_raw_fd()).unwrap(), [STATUS_DENIED]);
        }
        for worker in workers {
            worker.join().unwrap();
        }
    });
    drop(silent);
}
