use archy_authority::journal::Journal;
use archy_authority::{
    Channel, HUMAN_WORKERS, Identities, Store, accept_and_handle, check_listener, enrolled_human,
    named_uid,
};
use std::fs;
use std::io::{self, ErrorKind};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::sync::Arc;

fn main() {
    if let Err(e) = run() {
        eprintln!("authority unavailable: {e}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let ids = Identities {
        agent: named_uid("archy-agent")?,
        broker: named_uid("archy-broker")?,
        human: enrolled_human(Path::new("/etc/archy/authority-human-uid"))?,
    }
    .distinct()?;
    if unsafe { libc::geteuid() } != ids.broker {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "wrong service identity",
        ));
    }
    unsafe { libc::umask(0o077) };
    let store = Store::open(Path::new("/var/lib/archy-authority"), Some(ids.broker))?;
    let journal_path = Path::new("/var/lib/archy-authority/journal");
    match fs::create_dir(journal_path) {
        Ok(()) => (),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error),
    }
    // Startup rejects an altered or partially committed future decision log.
    // No decision operation is wired to this journal.
    let _journal = Journal::open(journal_path, ids.broker)?;
    let pid = std::env::var("LISTEN_PID").map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    let count = std::env::var("LISTEN_FDS").map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    let names =
        std::env::var("LISTEN_FDNAMES").map_err(|_| io::Error::from(ErrorKind::InvalidData))?;
    if pid != std::process::id().to_string() || count != "2" || names != "agent:human" {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "two named systemd listeners required",
        ));
    }
    check_listener(3, "/run/archy/agent.sock")?;
    check_listener(4, "/run/archy/human.sock")?;
    let listeners = [unsafe { OwnedFd::from_raw_fd(3) }, unsafe {
        OwnedFd::from_raw_fd(4)
    }];
    let store = Arc::new(store);
    let [agent, human] = listeners;
    let human = Arc::new(human);
    let agent_store = Arc::clone(&store);
    std::thread::spawn(move || {
        if let Err(e) = serve(agent.as_raw_fd(), Channel::Agent, ids, &agent_store) {
            eprintln!("authority unavailable: agent listener: {e}");
            std::process::exit(1);
        }
    });
    for _ in 1..HUMAN_WORKERS {
        let listener = Arc::clone(&human);
        let worker_store = Arc::clone(&store);
        std::thread::Builder::new()
            .name("archy-human-review".into())
            .spawn(move || {
                if let Err(e) = serve(listener.as_raw_fd(), Channel::Human, ids, &worker_store) {
                    eprintln!("authority unavailable: human listener: {e}");
                    std::process::exit(1);
                }
            })?;
    }
    serve(human.as_raw_fd(), Channel::Human, ids, &store)
}

fn serve(listener: i32, channel: Channel, ids: Identities, store: &Store) -> io::Result<()> {
    loop {
        match accept_and_handle(listener, channel, ids, store) {
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            result => result?,
        }
    }
}
