# A09 systemd-to-Rust scratch receiver review

**Scoped verdict: PASS.** The frozen fixture exercises a real WSL transient user service and the public Rust `receive_quota_checked_scratch()` API. It establishes the descriptor handoff and the tested fixture's teardown sequence. It does not establish production service authentication or containment.

The Rust test creates a UUID-scoped user unit with a private bounded tmpfs and passes a seqpacket socket to the fixed Python service helper. The helper sends the marker and an FD with `SCM_RIGHTS`. The test calls the public receiver before sending `GO`, then independently checks ownership, exact `0700` mode, tmpfs, byte and inode capacities, mount flags, and close-on-exec. It records the active unit's cgroup path and confirms its MainPID and effective limits before the token.

After `systemd-run --wait` exits, the test requires the unit to be inactive and collected. It checks the **previously recorded** cgroup path for `populated 0` or absence before it reads the payload through the retained Rust-owned FD. Thus the read does not rely on a newly empty `ControlGroup` property after collection. This proves the ordering for the fixed fixture and unique unit; it is not a whole-writer teardown proof for arbitrary workers.

I ran this command in `/home/butahlecoq/work/archy/.worktrees/a09-service-receiver`:

```sh
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-service-review CARGO_BUILD_JOBS=2 /usr/bin/timeout --signal=TERM --kill-after=10s 120s cargo test --locked -p archy-adapters --test systemd_receiver -- --ignored --test-threads=1 --nocapture
```

It exited 0. The positive case reported UID 1000, mode `0700`, tmpfs, 65,536 bytes, eight inodes, `nosuid,nodev,noexec`, close-on-exec, inactive collected unit, absent cgroup, and unchanged host sentinels. The `0777` directory and regular-file cases returned `BACKEND_UNAVAILABLE` from the public receiver. The test closed the socket's write side and received `NO-GO` from the real helper before waiting for each service. All three cases reported an absent cgroup and collected unit. The test's FD count returned to baseline.

The fixture has a 30-second per-case deadline, the receiver's eight-second cap, a 20-second service runtime limit, bounded systemctl reads, and scoped stop/reap cleanup on failure. The command above also had an outer 120-second deadline. After the run, `/tmp/archy-rust-receiver-*` and `tests/security/__pycache__/*` were empty, and `systemctl --user list-units --all --no-legend 'archy-rust-receiver-*.service'` returned no units. These checks cover this run, not every possible manager failure.

The live worktree was at base `634a1149ceb757d63c4f9c4aefb53d6ac30abaac`. Before and after the test, its source hashes matched the handoff:

```text
96823c6529a2f7f044cedc70910b323fc7695a1348be1e171ac653f6634c93d7  crates/archy-adapters/tests/systemd_receiver.rs
5804fc71df012661499d1a5c5c10065d79a089c3e5034db6c1bf2824068ced38  tests/security/systemd_rust_receiver_helper.py
cef81aaa9bc7434aa1d49bd16eaa3604dd956e8d388cb275c868ee321f0d81de  crates/archy-adapters/src/isolation.rs
13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40  Cargo.lock
ff741ec41f722f51754749cf6c0d00a063a0e8379d468fd7e54914b22361d705  docs/plans/systemd-rust-receiver-handoff.md
```

The fixed marker is not authentication. Service identity binding, production cgroup policy, arbitrary-worker teardown, bwrap containment, semantic artifact verification, and Omarchy acceptance remain separate gates. The author reports adapter tests, format, and strict clippy passing; I did not rerun those broader gates. Pstack's **Boundary Discipline** guided the FD and token checks, and **Prove It Works** guided the live service run.
