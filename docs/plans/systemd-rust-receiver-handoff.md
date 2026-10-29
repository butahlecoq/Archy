# A09 real systemd-to-Rust scratch receiver proof

Date: 2026-10-29. Worktree `.worktrees/a09-service-receiver`, branch `codex/a09-service-receiver`, base `634a114`. The owned files are `crates/archy-adapters/tests/systemd_receiver.rs`, `tests/security/systemd_rust_receiver_helper.py`, and this handoff. The published receiver, protocol, production launcher, artifact capture, Cargo manifests and lockfile were not changed. No backend worker, sudo, host mount, system unit, commit, push or subagent was used.

## Fixture boundary

The opt-in Rust integration test creates a UUID-scoped transient user service with fixed `/usr/bin/systemd-run --user --pipe --wait --collect --service-type=exec --expand-environment=no`. The service's fixed `/usr/bin/python3 -I -B` fixture sends the marker and exactly one FD over its stdin seqpacket socket. The mount is a private tmpfs at a disposable path, with 65,536-byte and eight-inode caps and `nosuid,nodev,noexec`. The service has `RuntimeMaxSec=20s`, `MemoryMax=268435456`, `MemorySwapMax=0`, `TasksMax=16`, `KillMode=control-group` and a two-second stop grace.

For the positive case, the test calls the **public Rust `receive_quota_checked_scratch`** on the socket. Before `GO`, it independently checks the accepted FD's close-on-exec flag, UID, exact `0700` mode, tmpfs type, `statvfs` capacities and mount flags. It also checks the unit is active, its MainPID is in its cgroup, and actual `memory.max`, `memory.swap.max`, `pids.max` and `cgroup.events` match the fixture's requested limits. Only then does it send `GO`. After `systemd-run --wait` exits, the test requires inactive state, collected unit, and an empty or absent cgroup before opening and reading the payload through the retained Rust-owned FD. The host backing sentinel and outside sentinel must remain byte-identical.

The same real service path tests mode `0777` and a regular `/dev/null` FD. Both must return `BACKEND_UNAVAILABLE` from the public receiver; the test closes its socket's write side without sending `GO` and requires the fixed helper's `NO-GO` acknowledgement. Each case has a 30-second deadline, scoped cleanup, a manager runtime limit, and bounded `systemctl` output. Failure to establish availability fails the explicitly invoked test; it never falls back to an unconstrained service. The normal Cargo suite leaves this environment-dependent test ignored.

## Exact evidence

All Cargo commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-service-receiver`, `CARGO_BUILD_JOBS=2`, `RUSTUP_AUTO_INSTALL=0`, and `CARGO_NET_OFFLINE=true`.

| Command | Result |
| --- | --- |
| `cargo test -p archy-adapters --locked --test systemd_receiver -- --ignored --test-threads=1 --nocapture` | Exit 0; one test passed all three live service cases. The accepted FD reported UID 1000, mode `0700`, tmpfs, 65536 bytes, 8 inodes, `nosuid,nodev,noexec`, and close-on-exec. The unit was collected and its cgroup absent before the retained-FD payload read. Both unsafe cases were refused before `GO` and their units were collected. |
| `cargo test -p archy-adapters --locked` | Exit 0; 33 tests passed, 2 ignored (the pinned backend test and this explicit systemd test). |
| `cargo fmt -p archy-adapters -- --check` | Exit 0. |
| `cargo clippy -p archy-adapters --all-targets --locked -- -D warnings` | Exit 0. |
| `/usr/bin/python3 -B -c 'import ast,pathlib; ast.parse(pathlib.Path("tests/security/systemd_rust_receiver_helper.py").read_text())'` | Exit 0. |

The initial test-first run failed because the fixed helper did not yet exist; an earlier compile attempt also exposed and corrected a test harness type error in acknowledgement parsing. The completed fixture passed on WSL2 kernel `6.6.87.2-microsoft-standard-WSL2`, systemd `259 (259.5-0ubuntu3.4)`, and Python 3.14.4. A final `systemctl --user list-units 'archy-rust-receiver-*.service' --all --no-legend` reported no matching units; `/tmp/archy-rust-receiver-*` and `tests/security/__pycache__/*` were empty. The test checks its process FD count returns to baseline. No package or dependency version was added.

SHA-256: `tests/systemd_receiver.rs` `96823c6529a2f7f044cedc70910b323fc7695a1348be1e171ac653f6634c93d7`; `tests/security/systemd_rust_receiver_helper.py` `5804fc71df012661499d1a5c5c10065d79a089c3e5034db6c1bf2824068ced38`; published `src/isolation.rs` `cef81aaa9bc7434aa1d49bd16eaa3604dd956e8d388cb275c868ee321f0d81de`; `Cargo.lock` `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40`. Fixture and handoff are frozen for independent review.

## Limits

The test shows that this WSL user manager forwards a real private tmpfs FD through the public Rust receiver and that the fixed fixture can retain it across unit teardown. The earlier [kernel proof](scratch-fd-probe-handoff.md) observed real byte and inode `ENOSPC` at these capacities; this service test verifies capacities but does not force exhaustion again. The marker and MainPID/cgroup observations do not authenticate service identity or establish a production supervisor, bwrap containment, whole-writer teardown for arbitrary workloads, semantic artifact verification, or Omarchy acceptance. Those remain separate gates.
