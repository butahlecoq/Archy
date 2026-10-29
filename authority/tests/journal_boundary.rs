use archy_authority::journal::Journal;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "archy-journal-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn open(&self) -> Journal {
        Journal::open(&self.0, unsafe { libc::geteuid() }).unwrap()
    }
    fn single_event(&self) -> PathBuf {
        fs::read_dir(&self.0)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn opaque_probe_record_is_create_only_and_exact_replay_is_idempotent() {
    let f = Fixture::new();
    let journal = f.open();
    let first = journal
        .append("probe-key", "non-decision probe record")
        .unwrap();
    assert_eq!(first.0, 1);
    assert_eq!(
        journal
            .append("probe-key", "non-decision probe record")
            .unwrap(),
        first
    );
    assert_eq!(
        journal
            .append("probe-key", "different probe")
            .unwrap_err()
            .kind(),
        ErrorKind::AlreadyExists
    );
    assert_eq!(journal.read_verified().unwrap().len(), 1);
    assert_eq!(
        f.open().read_verified().unwrap()[0].payload(),
        "non-decision probe record"
    );
}

#[test]
fn escaped_payload_over_event_limit_is_rejected_without_poisoning_journal() {
    let f = Fixture::new();
    let journal = f.open();
    let oversized = "\0".repeat(16 * 1024);
    assert_eq!(
        journal.append("oversized", &oversized).unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
    assert!(journal.read_verified().unwrap().is_empty());

    let accepted = "\0".repeat(10_000);
    assert_eq!(journal.append("accepted", &accepted).unwrap().0, 1);
    assert_eq!(f.open().read_verified().unwrap()[0].payload(), accepted);
}

#[test]
fn changed_or_unexpected_disk_state_blocks_further_writes() {
    let f = Fixture::new();
    let journal = f.open();
    journal.append("probe-1", "first probe").unwrap();
    fs::write(f.single_event(), b"tampered").unwrap();
    assert!(Journal::open(&f.0, unsafe { libc::geteuid() }).is_err());
    assert!(journal.append("probe-2", "second probe").is_err());
}

#[test]
fn renamed_valid_probe_record_cannot_rewrite_hash_chain_history() {
    let f = Fixture::new();
    let journal = f.open();
    let (_, old_hash) = journal.append("probe-first", "first probe").unwrap();
    journal.append("probe-second", "second probe").unwrap();
    let old_name = f.0.join(format!("{:020}-{old_hash}.event", 1));
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&old_name).unwrap()).unwrap();
    value["payload"] = "changed non-decision probe".into();
    let changed = serde_json::to_vec(&value).unwrap();
    let new_name = f.0.join(format!(
        "{:020}-{}.event",
        1,
        archy_authority::hex_digest(&changed)
    ));
    fs::write(&old_name, changed).unwrap();
    fs::rename(old_name, new_name).unwrap();
    assert!(Journal::open(&f.0, unsafe { libc::geteuid() }).is_err());
    assert!(journal.read_verified().is_err());
}

#[test]
fn pending_file_and_symlink_block_recovery_without_following_them() {
    let f = Fixture::new();
    fs::write(f.0.join(".pending-abandoned"), b"partial").unwrap();
    assert!(Journal::open(&f.0, unsafe { libc::geteuid() }).is_err());
    fs::remove_file(f.0.join(".pending-abandoned")).unwrap();
    let outside = f.0.with_extension("outside");
    fs::write(&outside, b"outside sentinel").unwrap();
    symlink(
        &outside,
        f.0.join(format!("{}-{}.event", "0".repeat(20), "0".repeat(64))),
    )
    .unwrap();
    assert!(Journal::open(&f.0, unsafe { libc::geteuid() }).is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"outside sentinel");
    fs::remove_file(outside).unwrap();
}

#[test]
fn broker_private_pre_rename_temp_is_rolled_back_on_open() {
    let f = Fixture::new();
    let pending = f.0.join(format!(".pending-{}", "a".repeat(64)));
    fs::write(&pending, b"incomplete non-decision probe").unwrap();
    fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
    let journal = f.open();
    assert!(!pending.exists());
    assert!(journal.read_verified().unwrap().is_empty());
    assert_eq!(
        journal
            .append("probe-after-crash", "recovery probe")
            .unwrap()
            .0,
        1
    );
}

#[test]
fn production_owner_and_mode_are_required() {
    let f = Fixture::new();
    let wrong_uid = unsafe { libc::geteuid() }.wrapping_add(1);
    assert_eq!(
        Journal::open(&f.0, wrong_uid).err().unwrap().kind(),
        ErrorKind::PermissionDenied
    );
}

#[test]
fn separate_openers_serialize_without_gaps_or_overwrites() {
    let f = Fixture::new();
    let barrier = Arc::new(Barrier::new(12));
    let journals: Vec<_> = (0..12).map(|_| f.open()).collect();
    let handles: Vec<_> = journals
        .into_iter()
        .enumerate()
        .map(|(i, journal)| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                journal.append(&format!("probe-{i}"), "non-decision concurrency probe")
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    let records = f.open().read_verified().unwrap();
    assert_eq!(records.len(), 12);
}
