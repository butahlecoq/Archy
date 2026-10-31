use archy_authority::{MAX_FRAME, OP_SUBMIT, STATUS_FULL, STATUS_OK, connect_local, receive, send};
use archy_core::decision_contract::Proposal;
use std::io::{self, ErrorKind, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

fn main() {
    if let Err(e) = run() {
        eprintln!("proposal unavailable: {e}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let mut input = Vec::new();
    io::stdin()
        .take((MAX_FRAME + 1) as u64)
        .read_to_end(&mut input)?;
    if input.len() > MAX_FRAME {
        return Err(io::Error::from(ErrorKind::InvalidData));
    }
    let proposal: Proposal =
        serde_json::from_slice(&input).map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    proposal
        .validate()
        .map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    connect_local(socket.as_raw_fd(), b"/run/archy/agent.sock")?;
    let mut request = vec![OP_SUBMIT];
    request.extend(input);
    send(socket.as_raw_fd(), &request)?;
    let response = receive(socket.as_raw_fd())?;
    if response == [STATUS_FULL] {
        return Err(io::Error::new(
            ErrorKind::StorageFull,
            "proposal store full",
        ));
    }
    if response.first() != Some(&STATUS_OK) {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "proposal rejected",
        ));
    }
    let digest =
        std::str::from_utf8(&response[1..]).map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    if digest != archy_authority::hex_digest(&request[1..]) {
        return Err(io::Error::from(ErrorKind::InvalidData));
    }
    println!("{digest}");
    Ok(())
}
