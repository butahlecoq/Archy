//! Administrator-invoked, broker-UID source ingestion. This creates no decision.
use archy_authority::{hex_digest, named_uid, protected_project::ProtectedProject};
use std::io::{self, ErrorKind, Read};
use std::path::Path;

const STATE: &str = "/var/lib/archy-authority";

fn main() {
    if let Err(error) = run() {
        eprintln!("protected project ingestion unavailable: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let owner = named_uid("archy-broker")?;
    if owner == 0 || unsafe { libc::geteuid() } != owner {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "broker UID required",
        ));
    }
    unsafe { libc::umask(0o077) };
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "usage: archy-project-ingest COUNT SEED < project.json",
        ));
    }
    let count: u32 = args[1]
        .parse()
        .map_err(|_| io::Error::from(ErrorKind::InvalidInput))?;
    let seed: u64 = args[2]
        .parse()
        .map_err(|_| io::Error::from(ErrorKind::InvalidInput))?;
    if !(1..=32).contains(&count) {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "COUNT must be 1..32",
        ));
    }
    let mut source = Vec::new();
    io::stdin().take(1024 * 1024 + 1).read_to_end(&mut source)?;
    if source.len() > 1024 * 1024 {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "source exceeds 1 MiB",
        ));
    }
    let writer = ProtectedProject::initialize(Path::new(STATE), owner, &source)?;
    let key = hex_digest(
        format!(
            "archy-broker-ingest-v1:{count}:{seed}:{}",
            hex_digest(&source)
        )
        .as_bytes(),
    );
    let receipt = writer.generate_accepted(count, seed, key, 0)?;
    let output = serde_json::to_string(&receipt).map_err(io::Error::other)?;
    println!("{output}");
    Ok(())
}
