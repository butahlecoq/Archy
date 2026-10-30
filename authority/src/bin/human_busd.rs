use archy_authority::human_bus::HumanBus;
use archy_authority::protected_project::ProtectedProject;
use archy_authority::{Identities, Store, enrolled_human, named_uid};
use std::io::{self, ErrorKind};
use std::path::Path;
use std::sync::Arc;
use zbus::connection::Builder;

fn retry_delay(attempt: u8) -> Option<std::time::Duration> {
    [1, 2, 4]
        .get(attempt as usize)
        .map(|seconds| std::time::Duration::from_secs(*seconds))
}

const STABLE_CONNECTION: std::time::Duration = std::time::Duration::from_secs(60);

fn next_incident(previous: u8, connected_for: std::time::Duration) -> u8 {
    if connected_for >= STABLE_CONNECTION {
        1
    } else {
        previous.saturating_add(1)
    }
}

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 7 && args[1] == "--snapshot-worker" {
        let state = std::path::PathBuf::from(&args[2]);
        let owner = args[3].to_string_lossy().parse::<u32>();
        let result = owner.map_err(io::Error::other).and_then(|owner| {
            archy_authority::snapshot_worker::child_main(
                &state,
                owner,
                &args[4].to_string_lossy(),
                &args[5].to_string_lossy(),
                &args[6].to_string_lossy(),
            )
        });
        if result.is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.len() != 1 {
        std::process::exit(1);
    }
    if let Err(error) = zbus::block_on(run()) {
        eprintln!("human bus unavailable: {error}");
        std::process::exit(1);
    }
}

async fn run() -> io::Result<()> {
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
    let state = Path::new("/var/lib/archy-authority");
    let _journal = archy_authority::journal::Journal::open(&state.join("journal"), ids.broker)?;
    // The procfs link refers to this running image even if its package path
    // changes before a worker is spawned.
    let worker = std::path::PathBuf::from("/proc/self/exe");
    // A stable connection starts a new finite incident window. A burst still
    // exits to the bounded external supervisor; it cannot spin here forever.
    let mut incidents = 0;
    loop {
        let _store = Store::open(state, Some(ids.broker))?;
        let project = Arc::new(ProtectedProject::open(
            &state.join("project/project.json"),
            ids.broker,
        )?);
        let interface = HumanBus::new(project, ids, worker.clone());
        let connection = async {
            Builder::system()?
                .max_message_size(64 * 1024)
                .max_queued(4)
                .method_timeout(std::time::Duration::from_secs(1))
                .name("org.archy.Authority1")?
                .serve_at("/org/archy/Authority1", interface)?
                .build()
                .await
        }
        .await;
        let connected_for = if let Ok(connection) = connection {
            let connected_at = std::time::Instant::now();
            connection.closed().await;
            connected_at.elapsed()
        } else {
            std::time::Duration::ZERO
        };
        incidents = next_incident(incidents, connected_for);
        if let Some(delay) = retry_delay(incidents - 1) {
            async_io::Timer::after(delay).await;
        } else {
            break;
        }
    }
    Err(io::Error::new(
        ErrorKind::ConnectionAborted,
        "human bus connection closed",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_budget_is_finite_and_rate_limited() {
        assert_eq!(retry_delay(0), Some(std::time::Duration::from_secs(1)));
        assert_eq!(retry_delay(1), Some(std::time::Duration::from_secs(2)));
        assert_eq!(retry_delay(2), Some(std::time::Duration::from_secs(4)));
        assert_eq!(retry_delay(3), None);
        assert_eq!(retry_delay(u8::MAX), None);
        assert_eq!(next_incident(3, std::time::Duration::from_secs(59)), 4);
        assert_eq!(next_incident(3, STABLE_CONNECTION), 1);
        assert_eq!(next_incident(1, std::time::Duration::ZERO), 2);
        let mut incidents = 0;
        for expected in 1..=4 {
            incidents = next_incident(incidents, std::time::Duration::ZERO);
            assert_eq!(incidents, expected);
            assert_eq!(retry_delay(incidents - 1).is_some(), expected < 4);
        }
        incidents = next_incident(incidents, STABLE_CONNECTION);
        assert_eq!(incidents, 1);
        assert_eq!(
            retry_delay(incidents - 1),
            Some(std::time::Duration::from_secs(1))
        );
    }
}
