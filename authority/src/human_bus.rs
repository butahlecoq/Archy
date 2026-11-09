//! System-bus review boundary. This module deliberately has no decision writer.
use crate::Identities;
use crate::bus_identity::PinnedBusPeer;
use crate::protected_project::ProtectedProject;
use crate::snapshot_worker;
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use zbus::fdo::{DBusProxy, Error};
use zbus::message::Header;
use zbus::names::BusName;
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy, interface};

fn denied() -> Error {
    Error::AccessDenied("human confirmation boundary unavailable".into())
}

pub struct HumanBus {
    pub project: Arc<ProtectedProject>,
    pub identities: Identities,
    in_flight: Arc<AtomicUsize>,
    worker_executable: PathBuf,
}

const MAX_REVIEW_CALLS: usize = 4;
const REVIEW_DEADLINE: Duration = Duration::from_secs(3);

struct ReviewSlot(Arc<AtomicUsize>);

struct Completion(Arc<AtomicBool>);

impl Drop for Completion {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

impl ReviewSlot {
    fn acquire(counter: &Arc<AtomicUsize>) -> Result<Self, Error> {
        counter
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_REVIEW_CALLS).then_some(n + 1)
            })
            .map_err(|_| denied())?;
        Ok(Self(counter.clone()))
    }
}

impl Drop for ReviewSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl HumanBus {
    pub fn new(
        project: Arc<ProtectedProject>,
        identities: Identities,
        worker_executable: PathBuf,
    ) -> Self {
        Self {
            project,
            identities,
            in_flight: Arc::new(AtomicUsize::new(0)),
            worker_executable,
        }
    }

    fn review_slot(&self) -> Result<ReviewSlot, Error> {
        ReviewSlot::acquire(&self.in_flight)
    }

    async fn peer(&self, conn: &Connection, header: Header<'_>) -> Result<PinnedBusPeer, Error> {
        // The sender comes from this method's header. Connection::peer_credentials
        // would describe the message bus, not the method caller.
        let sender = header.sender().ok_or_else(denied)?.to_string();
        let name = BusName::try_from(sender.as_str()).map_err(|_| denied())?;
        let bus = DBusProxy::new(conn).await.map_err(|_| denied())?;
        let credentials = bus
            .get_connection_credentials(name)
            .await
            .map_err(|_| denied())?;
        let fd = credentials
            .process_fd()
            .ok_or_else(denied)?
            .as_fd()
            .try_clone_to_owned()
            .map_err(|_| denied())?;
        let peer = PinnedBusPeer::from_bus_credentials(
            &sender,
            credentials.unix_user_id(),
            credentials.process_id(),
            Some(fd),
            self.identities.human,
            self.identities.agent,
            self.identities.broker,
        )
        .map_err(|_| denied())?;
        self.active_local_session(conn, &peer).await?;
        // A disconnect between credential and session queries must fail.
        let again = bus
            .get_connection_credentials(BusName::try_from(sender.as_str()).map_err(|_| denied())?)
            .await
            .map_err(|_| denied())?;
        if again.unix_user_id() != Some(peer.uid()) || again.process_id() != Some(peer.pid()) {
            return Err(denied());
        }
        peer.check_alive().map_err(|_| denied())?;
        Ok(peer)
    }

    async fn active_local_session(
        &self,
        conn: &Connection,
        peer: &PinnedBusPeer,
    ) -> Result<(), Error> {
        peer.check_alive().map_err(|_| denied())?;
        let manager = Proxy::new(
            conn,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await
        .map_err(|_| denied())?;
        let path: OwnedObjectPath = manager
            .call("GetSessionByPID", &peer.pid())
            .await
            .map_err(|_| denied())?;
        peer.check_alive().map_err(|_| denied())?;
        let session = Proxy::new(
            conn,
            "org.freedesktop.login1",
            path.as_str(),
            "org.freedesktop.login1.Session",
        )
        .await
        .map_err(|_| denied())?;
        let active: bool = session.get_property("Active").await.map_err(|_| denied())?;
        let remote: bool = session.get_property("Remote").await.map_err(|_| denied())?;
        let user: (u32, OwnedObjectPath) =
            session.get_property("User").await.map_err(|_| denied())?;
        let seat: (String, OwnedObjectPath) =
            session.get_property("Seat").await.map_err(|_| denied())?;
        let class: String = session.get_property("Class").await.map_err(|_| denied())?;
        peer.check_alive().map_err(|_| denied())?;
        if !active || remote || user.0 != peer.uid() || seat.0.is_empty() || class != "user" {
            return Err(denied());
        }
        Ok(())
    }

    async fn still_authorized(
        &self,
        conn: &Connection,
        header: Header<'_>,
        peer: &PinnedBusPeer,
    ) -> Result<(), Error> {
        if header.sender().map(|name| name.as_str()) != Some(peer.sender()) {
            return Err(denied());
        }
        peer.check_alive().map_err(|_| denied())?;
        let bus = DBusProxy::new(conn).await.map_err(|_| denied())?;
        let name = BusName::try_from(peer.sender()).map_err(|_| denied())?;
        let credentials = bus
            .get_connection_credentials(name)
            .await
            .map_err(|_| denied())?;
        if credentials.unix_user_id() != Some(peer.uid())
            || credentials.process_id() != Some(peer.pid())
        {
            return Err(denied());
        }
        self.active_local_session(conn, peer).await?;
        let credentials = bus
            .get_connection_credentials(BusName::try_from(peer.sender()).map_err(|_| denied())?)
            .await
            .map_err(|_| denied())?;
        if credentials.unix_user_id() != Some(peer.uid())
            || credentials.process_id() != Some(peer.pid())
        {
            return Err(denied());
        }
        peer.check_alive().map_err(|_| denied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_slots_reject_excess_and_recover_after_drop() {
        let counter = Arc::new(AtomicUsize::new(0));
        let held: Vec<_> = (0..MAX_REVIEW_CALLS)
            .map(|_| ReviewSlot::acquire(&counter).unwrap())
            .collect();
        assert!(ReviewSlot::acquire(&counter).is_err());
        assert_eq!(counter.load(Ordering::Acquire), MAX_REVIEW_CALLS);
        drop(held);
        assert_eq!(counter.load(Ordering::Acquire), 0);
        assert!(ReviewSlot::acquire(&counter).is_ok());
    }

    #[test]
    fn concurrent_review_slots_reject_fifth_caller() {
        let counter = Arc::new(AtomicUsize::new(0));
        let barrier = std::sync::Barrier::new(MAX_REVIEW_CALLS + 1);
        std::thread::scope(|scope| {
            for _ in 0..MAX_REVIEW_CALLS {
                scope.spawn(|| {
                    let _slot = ReviewSlot::acquire(&counter).unwrap();
                    barrier.wait();
                    barrier.wait();
                });
            }
            barrier.wait();
            assert!(ReviewSlot::acquire(&counter).is_err());
            barrier.wait();
        });
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }
}

// Dispatch on the connection task: zbus otherwise spawns one task per method
// before Review can acquire its slot, including malformed and unknown calls.
#[interface(name = "org.archy.Authority1", spawn = false)]
impl HumanBus {
    /// Only a live, enrolled local human may fetch a broker-recomputed snapshot.
    /// The caller's supplied digest names an untrusted proposal, never authority.
    async fn review(
        &self,
        proposal_digest: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> Result<(String, String), Error> {
        if proposal_digest.len() != 64
            || !proposal_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(denied());
        }
        let deadline = Instant::now() + REVIEW_DEADLINE;
        let slot = self.review_slot()?;
        let peer = futures_lite::future::or(self.peer(conn, header.clone()), async {
            async_io::Timer::at(deadline).await;
            Err(denied())
        })
        .await?;
        if Instant::now() >= deadline {
            return Err(denied());
        }
        let project = self.project.clone();
        let executable = self.worker_executable.clone();
        let owner = unsafe { libc::geteuid() };
        let digest = proposal_digest.to_owned();
        let (send, receive) = futures_channel::oneshot::channel();
        let finished = Arc::new(AtomicBool::new(false));
        let watched = finished.clone();
        std::thread::Builder::new()
            .name("archy-snapshot-watchdog".into())
            .spawn(move || {
                std::thread::sleep(
                    deadline.saturating_duration_since(Instant::now()) + Duration::from_secs(1),
                );
                if !watched.load(Ordering::Acquire) {
                    std::process::abort();
                }
            })
            .unwrap_or_else(|_| std::process::abort());
        let completion = finished.clone();
        let worker = std::thread::Builder::new()
            .name("archy-review-snapshot".into())
            .spawn(move || {
                let _completion = Completion(completion);
                let _slot = slot;
                let result = snapshot_worker::run(&executable, &project, owner, &digest, deadline)
                    .map_err(|_| denied());
                let _ = send.send(result);
            });
        if worker.is_err() {
            finished.store(true, Ordering::Release);
            return Err(denied());
        }
        let snapshot =
            futures_lite::future::or(async { receive.await.map_err(|_| denied())? }, async {
                async_io::Timer::at(deadline).await;
                Err(denied())
            })
            .await?;
        futures_lite::future::or(self.still_authorized(conn, header, &peer), async {
            async_io::Timer::at(deadline).await;
            Err(denied())
        })
        .await?;
        if Instant::now() >= deadline {
            return Err(denied());
        }
        Ok(snapshot)
    }

    /// No authentication result or caller-supplied assertion can approve a design.
    async fn confirm(
        &self,
        _snapshot_digest: &str,
        #[zbus(header)] _header: Header<'_>,
    ) -> Result<(), Error> {
        Err(denied())
    }
}
