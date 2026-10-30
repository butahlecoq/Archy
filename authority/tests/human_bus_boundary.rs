use archy_authority::bus_identity::PinnedBusPeer;
use archy_authority::human_bus::HumanBus;
use archy_authority::protected_project::ProtectedProject;
use archy_authority::{Identities, Store, hex_digest};
use archy_core::ProjectStore;
use archy_core::decision_contract::{
    ConstraintEvidence, PROPOSAL_SCHEMA_VERSION, Proposal, ReviewEvidence,
};
use std::fs;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use zbus::connection::Builder;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::zvariant::OwnedObjectPath;

const SESSION_PATH: &str = "/org/freedesktop/login1/session/mock";

struct MockManager {
    lookups: Arc<AtomicUsize>,
    stall: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl MockManager {
    #[zbus(name = "GetSessionByPID")]
    async fn get_session_by_pid(&self, _pid: u32) -> OwnedObjectPath {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        if self.stall.load(Ordering::SeqCst) {
            async_io::Timer::after(Duration::from_secs(5)).await;
        }
        OwnedObjectPath::try_from(SESSION_PATH).unwrap()
    }
}

struct MockSession {
    uid: Arc<AtomicU32>,
    active: Arc<AtomicBool>,
    remote: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl MockSession {
    #[zbus(property)]
    fn active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
    #[zbus(property)]
    fn remote(&self) -> bool {
        self.remote.load(Ordering::SeqCst)
    }
    #[zbus(property)]
    fn user(&self) -> (u32, OwnedObjectPath) {
        (
            self.uid.load(Ordering::SeqCst),
            OwnedObjectPath::try_from("/org/freedesktop/login1/user/mock").unwrap(),
        )
    }
    #[zbus(property)]
    fn seat(&self) -> (String, OwnedObjectPath) {
        (
            "seat0".into(),
            OwnedObjectPath::try_from("/org/freedesktop/login1/seat/seat0").unwrap(),
        )
    }
    #[zbus(property)]
    fn class(&self) -> String {
        "user".into()
    }
}

fn accepted_proposal(base: &Path, project: &ProtectedProject) -> Vec<u8> {
    let receipt = project
        .generate_accepted(3, 42, "bus-review-fixture".into(), 0)
        .unwrap();
    let project_store = ProjectStore::open(&base.join("project/project.json")).unwrap();
    let history = project_store.read_history().unwrap();
    let run = archy_cli::verify_history(&history, &receipt.run_id).unwrap();
    let source = history.read_run(&receipt.run_id).unwrap();
    let (variant, evaluation) = run
        .variants()
        .iter()
        .zip(run.evaluations())
        .find(|(_, evaluation)| evaluation.required_checks_passed)
        .unwrap();
    let variant_bytes = source
        .artifact(&format!("variants/{}.json", variant.id))
        .unwrap();
    let evaluation_bytes = source
        .artifact(&format!("evaluations/{}.json", variant.id))
        .unwrap();
    let manifest_hash = history
        .receipts()
        .find(|r| r.run_id == receipt.run_id)
        .unwrap()
        .manifest_sha256
        .clone();
    let comparison = archy_cli::render_verified_comparison(
        &run,
        &manifest_hash,
        history.revision(),
        history.head_sha256().map(str::to_owned),
    )
    .unwrap();
    let proposal = Proposal {
        schema_version: PROPOSAL_SCHEMA_VERSION.into(),
        evidence: ReviewEvidence {
            project_id: run.spec().project_id.clone(),
            variant_id: variant.id.clone(),
            source_run_id: receipt.run_id,
            expected_project_revision: history.revision(),
            expected_head_sha256: history.head_sha256().unwrap().into(),
            input_sha256: hex_digest(run.input()),
            normalized_sha256: hex_digest(source.artifact("normalized.json").unwrap()),
            variant_sha256: hex_digest(variant_bytes),
            evaluation_sha256: hex_digest(evaluation_bytes),
            comparison_sha256: hex_digest(&comparison.json),
            constraints: run
                .spec()
                .constraints
                .iter()
                .map(ConstraintEvidence::from_normalized)
                .collect(),
            checks: evaluation.checks.clone(),
        },
        suggested_reason: "Review candidate".into(),
    };
    serde_json::to_vec(&proposal).unwrap()
}

#[test]
fn private_real_bus_review_requires_local_session_and_never_confirms() {
    let base = PathBuf::from(std::env::var_os("HOME").unwrap()).join(format!(
        "archy-human-bus-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&base).unwrap();
    fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
    let config = base.join("private-bus.conf");
    fs::write(
        &config,
        format!(r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir={}</listen>
  <limit name="max_message_size">2097152</limit>
  <limit name="max_incoming_bytes">4194304</limit>
  <limit name="max_outgoing_bytes">4194304</limit>
  <policy context="default">
    <allow send_destination="*"/>
    <allow receive_sender="*"/>
    <allow own="*"/>
  </policy>
</busconfig>"#, base.display()),
    )
    .unwrap();
    let output = Command::new("dbus-daemon")
        .args([
            "--config-file",
            config.to_str().unwrap(),
            "--fork",
            "--print-address=1",
            "--print-pid=1",
        ])
        .output()
        .expect("disposable dbus-daemon required");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = String::from_utf8(output.stdout).unwrap();
    let mut lines = lines.lines();
    let address = lines.next().unwrap().to_owned();
    let bus_pid: i32 = lines.next().unwrap().parse().unwrap();
    let uid = unsafe { libc::geteuid() };
    let project = ProtectedProject::initialize(
        &base,
        uid,
        include_bytes!("../../tests/fixtures/briefs/courtyard.json"),
    )
    .unwrap();
    let proposal = accepted_proposal(&base, &project);
    let store = Store::open(&base, Some(uid)).unwrap();
    let proposal_digest = store.submit(&proposal).unwrap();
    let expected_snapshot = project.snapshot(&proposal).unwrap();
    let project = Arc::new(project);
    let worker = base.join("gated-snapshot-worker");
    let hold = base.join("hold-snapshot");
    let ready = base.join("snapshot-ready");
    fs::write(
        &worker,
        format!(
            "#!/bin/sh\nset -eu\nreply=$(mktemp '{}/reply.XXXXXX')\n'{}' \"$@\" > \"$reply\"\nif test -e '{}'; then\n  touch '{}'\n  while test -e '{}'; do sleep 0.02; done\nfi\ncat \"$reply\"\nrm -f \"$reply\"\n",
            base.display(),
            env!("CARGO_BIN_EXE_archy-human-busd"),
            hold.display(),
            ready.display(),
            hold.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let interface = HumanBus::new(
        project.clone(),
        Identities {
            agent: uid.wrapping_add(1),
            broker: uid.wrapping_add(2),
            human: uid,
        },
        worker,
    );
    zbus::block_on(async {
        let service = Builder::address(address.as_str())
            .unwrap()
            .max_message_size(64 * 1024)
            .max_queued(4)
            .method_timeout(std::time::Duration::from_secs(1))
            .name("org.archy.Authority1")
            .unwrap()
            .serve_at("/org/archy/Authority1", interface)
            .unwrap()
            .build()
            .await
            .unwrap();
        let first = Builder::address(address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap();
        let second = Builder::address(address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap();
        let lookups = Arc::new(AtomicUsize::new(0));
        let stall = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(true));
        let remote = Arc::new(AtomicBool::new(false));
        let session_uid = Arc::new(AtomicU32::new(uid));
        let logind = Builder::address(address.as_str())
            .unwrap()
            .name("org.freedesktop.login1")
            .unwrap()
            .serve_at(
                "/org/freedesktop/login1",
                MockManager {
                    lookups: lookups.clone(),
                    stall: stall.clone(),
                },
            )
            .unwrap()
            .serve_at(
                SESSION_PATH,
                MockSession {
                    uid: session_uid.clone(),
                    active: active.clone(),
                    remote: remote.clone(),
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let manager_probe = zbus::Proxy::new(
            &service,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await
        .unwrap();
        let probe_path: OwnedObjectPath = manager_probe
            .call("GetSessionByPID", &std::process::id())
            .await
            .unwrap();
        assert_eq!(probe_path.as_str(), SESSION_PATH);
        assert_ne!(first.unique_name(), second.unique_name());
        let dbus = DBusProxy::new(&service).await.unwrap();
        for conn in [&first, &second] {
            let sender = conn.unique_name().unwrap().to_string();
            let creds = dbus
                .get_connection_credentials(BusName::try_from(sender.as_str()).unwrap())
                .await
                .unwrap();
            let process_fd = creds
                .process_fd()
                .expect("private bus must provide ProcessFD")
                .as_fd()
                .as_raw_fd();
            let flags = unsafe { libc::fcntl(process_fd, libc::F_GETFD) };
            assert_ne!(flags, -1);
            let exec_probe = Command::new("/bin/sh")
                .arg("-c")
                .arg("readlink /proc/self/fd/$1 2>/dev/null || true")
                .arg("sh")
                .arg(process_fd.to_string())
                .output()
                .unwrap();
            assert!(exec_probe.status.success());
            assert_ne!(
                exec_probe.stdout, b"anon_inode:[pidfd]\n",
                "bus ProcessFD crossed worker-style exec"
            );
            assert_ne!(
                flags & libc::FD_CLOEXEC,
                0,
                "bus ProcessFD must be close-on-exec"
            );
            let fd = creds
                .process_fd()
                .expect("private bus must provide ProcessFD")
                .as_fd()
                .try_clone_to_owned()
                .unwrap();
            let peer = PinnedBusPeer::from_bus_credentials(
                &sender,
                creds.unix_user_id(),
                creds.process_id(),
                Some(fd),
                uid,
                uid.wrapping_add(1),
                uid.wrapping_add(2),
            )
            .unwrap();
            assert_eq!(peer.sender(), sender);
            peer.check_alive().unwrap();
            let wrong_fd = creds
                .process_fd()
                .unwrap()
                .as_fd()
                .try_clone_to_owned()
                .unwrap();
            assert!(
                PinnedBusPeer::from_bus_credentials(
                    &sender,
                    creds.unix_user_id(),
                    creds.process_id().map(|p| p + 1),
                    Some(wrong_fd),
                    uid,
                    uid.wrapping_add(1),
                    uid.wrapping_add(2),
                )
                .is_err(),
                "mismatched numeric PID and bus pidfd must fail"
            );
            let wrong_uid_fd = creds
                .process_fd()
                .unwrap()
                .as_fd()
                .try_clone_to_owned()
                .unwrap();
            assert!(
                PinnedBusPeer::from_bus_credentials(
                    &sender,
                    creds.unix_user_id(),
                    creds.process_id(),
                    Some(wrong_uid_fd),
                    uid.wrapping_add(3),
                    uid.wrapping_add(1),
                    uid.wrapping_add(2),
                )
                .is_err(),
                "wrong enrolled UID must fail"
            );
            assert!(
                PinnedBusPeer::from_bus_credentials(
                    &sender,
                    creds.unix_user_id(),
                    creds.process_id(),
                    None,
                    uid,
                    uid.wrapping_add(1),
                    uid.wrapping_add(2),
                )
                .is_err()
            );
        }
        let first_proxy = zbus::Proxy::new(
            &first,
            "org.archy.Authority1",
            "/org/archy/Authority1",
            "org.archy.Authority1",
        )
        .await
        .unwrap();
        let second_proxy = zbus::Proxy::new(
            &second,
            "org.archy.Authority1",
            "/org/archy/Authority1",
            "org.archy.Authority1",
        )
        .await
        .unwrap();
        for proxy in [&first_proxy, &second_proxy] {
            let confirmation: zbus::Result<()> =
                proxy.call("Confirm", &expected_snapshot.digest()).await;
            assert!(
                confirmation.is_err(),
                "caller-supplied digest must never authorize"
            );
            let review: (String, String) = proxy.call("Review", &proposal_digest).await.unwrap();
            assert_eq!(review.0.as_bytes(), expected_snapshot.bytes());
            assert_eq!(review.1, expected_snapshot.digest());
            assert!(
                lookups.load(Ordering::SeqCst) > 0,
                "valid digest must reach mock logind"
            );
            let before = lookups.load(Ordering::SeqCst);
            for digest in ["short".to_owned(), "x".repeat(64), "A".repeat(64)] {
                let rejected: zbus::Result<(String, String)> =
                    proxy.call("Review", &(digest)).await;
                assert!(rejected.is_err());
                assert_eq!(
                    lookups.load(Ordering::SeqCst),
                    before,
                    "invalid input must not query logind"
                );
            }
        }
        let held_store = ProjectStore::open(&base.join("project/project.json")).unwrap();
        let held_lock = held_store.lock().unwrap();
        let blocked_at = Instant::now();
        let blocked_snapshot: zbus::Result<(String, String)> =
            first_proxy.call("Review", &proposal_digest).await;
        assert!(
            blocked_snapshot.is_err(),
            "held project lock must deny review"
        );
        assert!(blocked_at.elapsed() < Duration::from_secs(4));
        drop(held_lock);
        let after_lock: (String, String) =
            first_proxy.call("Review", &proposal_digest).await.unwrap();
        assert_eq!(after_lock.0.as_bytes(), expected_snapshot.bytes());
        // Keep the child snapshot blocked after admission, then remove the
        // session before allowing protected bytes to reach the bus method.
        fs::write(&hold, b"").unwrap();
        let caller = first.clone();
        let digest = proposal_digest.clone();
        let pending = std::thread::spawn(move || {
            zbus::block_on(async {
                let proxy = zbus::Proxy::new(
                    &caller,
                    "org.archy.Authority1",
                    "/org/archy/Authority1",
                    "org.archy.Authority1",
                )
                .await
                .unwrap();
                proxy
                    .call::<_, _, (String, String)>("Review", &digest)
                    .await
            })
        });
        let transition_deadline = Instant::now() + Duration::from_secs(2);
        while !ready.exists() && Instant::now() < transition_deadline {
            async_io::Timer::after(Duration::from_millis(5)).await;
        }
        assert!(
            ready.exists(),
            "worker must finish snapshot before transition"
        );
        assert!(!pending.is_finished(), "snapshot reply must still be gated");
        active.store(false, Ordering::SeqCst);
        fs::remove_file(&hold).unwrap();
        assert!(
            pending.join().unwrap().is_err(),
            "session transition during snapshot must deny protected bytes"
        );
        active.store(true, Ordering::SeqCst);
        active.store(false, Ordering::SeqCst);
        let inactive: zbus::Result<(String, String)> =
            first_proxy.call("Review", &proposal_digest).await;
        assert!(
            inactive.is_err(),
            "inactive session must deny valid proposal"
        );
        active.store(true, Ordering::SeqCst);
        remote.store(true, Ordering::SeqCst);
        let remote_review: zbus::Result<(String, String)> =
            first_proxy.call("Review", &proposal_digest).await;
        assert!(
            remote_review.is_err(),
            "remote session must deny valid proposal"
        );
        remote.store(false, Ordering::SeqCst);
        session_uid.store(uid.wrapping_add(10), Ordering::SeqCst);
        let wrong_uid: zbus::Result<(String, String)> =
            first_proxy.call("Review", &proposal_digest).await;
        assert!(
            wrong_uid.is_err(),
            "wrong session UID must deny valid proposal"
        );
        session_uid.store(uid, Ordering::SeqCst);
        stall.store(true, Ordering::SeqCst);
        let started = Instant::now();
        let stalled: zbus::Result<(String, String)> =
            first_proxy.call("Review", &proposal_digest).await;
        assert!(stalled.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "stalled logind must be bounded"
        );
        let started = Instant::now();
        let invalid: zbus::Result<(String, String)> = first_proxy.call("Review", &("short")).await;
        assert!(invalid.is_err());
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "invalid input must bypass stalled logind"
        );
        stall.store(false, Ordering::SeqCst);
        let recovered: (String, String) =
            first_proxy.call("Review", &proposal_digest).await.unwrap();
        assert_eq!(recovered.0.as_bytes(), expected_snapshot.bytes());
        assert_eq!(recovered.1, expected_snapshot.digest());
        let before = lookups.load(Ordering::SeqCst);
        let oversized = Builder::address(address.as_str())
            .unwrap()
            .method_timeout(Duration::from_secs(2))
            .build()
            .await
            .unwrap();
        let proxy = zbus::Proxy::new(
            &oversized,
            "org.archy.Authority1",
            "/org/archy/Authority1",
            "org.archy.Authority1",
        )
        .await
        .unwrap();
        let near_limit: zbus::Result<(String, String)> =
            proxy.call("Review", &("a".repeat(60 * 1024))).await;
        assert!(
            matches!(near_limit, Err(zbus::Error::MethodError(..))),
            "valid transport-sized call must reach handler: {near_limit:?}"
        );
        assert_eq!(lookups.load(Ordering::SeqCst), before);
        let rejected: zbus::Result<(String, String)> =
            proxy.call("Review", &("a".repeat(70 * 1024))).await;
        assert!(
            matches!(rejected, Err(ref error) if !matches!(error, zbus::Error::MethodError(..))),
            "service transport must reject before dispatch: {rejected:?}"
        );
        assert_eq!(lookups.load(Ordering::SeqCst), before);
        assert!(
            service.is_closed(),
            "oversize frame must close service connection"
        );
        drop(service);
        let mut recovered_service = Builder::address(address.as_str())
            .unwrap()
            .max_message_size(64 * 1024)
            .max_queued(4)
            .method_timeout(Duration::from_secs(1))
            .name("org.archy.Authority1")
            .unwrap()
            .serve_at(
                "/org/archy/Authority1",
                HumanBus::new(
                    project.clone(),
                    Identities {
                        agent: uid.wrapping_add(1),
                        broker: uid.wrapping_add(2),
                        human: uid,
                    },
                    PathBuf::from(env!("CARGO_BIN_EXE_archy-human-busd")),
                ),
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        for _ in 0..3 {
            let oversized = Builder::address(address.as_str())
                .unwrap()
                .method_timeout(Duration::from_secs(2))
                .build()
                .await
                .unwrap();
            let oversized_proxy = zbus::Proxy::new(
                &oversized,
                "org.archy.Authority1",
                "/org/archy/Authority1",
                "org.archy.Authority1",
            )
            .await
            .unwrap();
            let rejected: zbus::Result<(String, String)> = oversized_proxy
                .call("Review", &("a".repeat(70 * 1024)))
                .await;
            assert!(rejected.is_err());
            assert!(
                recovered_service.is_closed(),
                "oversized frame closes one service connection"
            );
            drop(recovered_service);
            recovered_service = Builder::address(address.as_str())
                .unwrap()
                .max_message_size(64 * 1024)
                .max_queued(4)
                .method_timeout(Duration::from_secs(1))
                .name("org.archy.Authority1")
                .unwrap()
                .serve_at(
                    "/org/archy/Authority1",
                    HumanBus::new(
                        project.clone(),
                        Identities {
                            agent: uid.wrapping_add(1),
                            broker: uid.wrapping_add(2),
                            human: uid,
                        },
                        PathBuf::from(env!("CARGO_BIN_EXE_archy-human-busd")),
                    ),
                )
                .unwrap()
                .build()
                .await
                .unwrap();
        }
        let recovered_proxy = zbus::Proxy::new(
            &first,
            "org.archy.Authority1",
            "/org/archy/Authority1",
            "org.archy.Authority1",
        )
        .await
        .unwrap();
        active.store(false, Ordering::SeqCst);
        let stale_session: zbus::Result<(String, String)> =
            recovered_proxy.call("Review", &proposal_digest).await;
        assert!(
            stale_session.is_err(),
            "new connection must recheck session"
        );
        active.store(true, Ordering::SeqCst);
        let fresh: (String, String) = recovered_proxy
            .call("Review", &proposal_digest)
            .await
            .unwrap();
        assert_eq!(fresh.0.as_bytes(), expected_snapshot.bytes());
        assert_eq!(fresh.1, expected_snapshot.digest());
        assert!(
            recovered_proxy
                .call::<_, _, ()>("Confirm", &fresh.1)
                .await
                .is_err()
        );
        drop(logind);
        drop(recovered_service);
    });
    fs::remove_dir_all(&base).unwrap();
    unsafe { libc::kill(bus_pid, libc::SIGTERM) };
}
