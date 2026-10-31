use archy_authority::{MAX_FRAME, OP_READ, STATUS_OK, connect_local, receive, send};
use archy_core::decision_contract::Proposal;
use std::io::{self, ErrorKind};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

fn main() {
    if let Err(e) = run() {
        eprintln!("review unavailable: {e}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let digest = std::env::args().nth(1).ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidInput,
            "usage: archy-review <proposal-sha256>",
        )
    })?;
    if std::env::args().count() != 2
        || digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(io::Error::from(ErrorKind::InvalidInput));
    }
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    connect_local(socket.as_raw_fd(), b"/run/archy/human.sock")?;
    let mut request = vec![OP_READ];
    request.extend(digest.as_bytes());
    send(socket.as_raw_fd(), &request)?;
    let response = receive(socket.as_raw_fd())?;
    if response.first() != Some(&STATUS_OK) {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "proposal not available",
        ));
    }
    let bytes = &response[1..];
    if bytes.len() > MAX_FRAME || archy_authority::hex_digest(bytes) != digest {
        return Err(io::Error::from(ErrorKind::InvalidData));
    }
    let proposal: Proposal =
        serde_json::from_slice(bytes).map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    proposal
        .validate()
        .map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    println!("UNVERIFIED AGENT PROPOSAL — NO HUMAN APPROVAL AVAILABLE");
    println!("SHA-256: {digest}");
    // escape_default prevents terminal controls, bidi marks and non-ASCII format characters
    // from altering what the reviewer sees. The broker's bytes remain the source of truth.
    let formatted = serde_json::to_string_pretty(&proposal)
        .map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    println!("{}", safe_display(&formatted));
    Ok(())
}

fn safe_display(formatted: &str) -> String {
    formatted
        .chars()
        .map(|c| {
            if c == '\n' || c.is_ascii_graphic() || c == ' ' {
                c.to_string()
            } else {
                c.escape_default().to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::safe_display;

    #[test]
    fn control_and_bidi_text_cannot_change_terminal_display() {
        let rendered = safe_display("{\n  \"reason\": \"A\u{202e}B\u{1b}[31m\"\n}");
        assert!(rendered.contains("\\u{202e}"));
        assert!(rendered.contains("\\u{1b}"));
        assert!(rendered.contains("\n  \"reason\""));
        assert!(!rendered.contains('\u{202e}'));
        assert!(!rendered.contains('\u{1b}'));
    }
}
