# CI escaped pipeholder: round-two Sol handoff

## Scope

- Worktree: `/home/butahlecoq/work/archy/.worktrees/ci-escaped-pipeholder-40945971854`, HEAD `8e36e11d1399dee37dc2b85a8e2225b473571602`.
- Code change: `crates/archy-adapters/src/process.rs`, SHA-256 `a519321c480723e09eb2e417c5ac10c7d231373c5159ff0407653c31d66197ee`.
- `git diff -- crates/archy-adapters/src/process.rs` SHA-256: `0e1c5e17367129fd8e41ae45fb3dd4b9296304dec2036dee760da7782476a4f2`.
- No other code file changed. `process` is included only under `#[cfg(all(test, target_os = "linux"))]` in `lib.rs`; production builds do not include this module. No commit, push, install, or child agent.
- Cargo target: this worktree's `target/` on `/dev/sdd` ext4. Diagnostic preload files/logs are under `target/ci-review-probe/`.

## Finding and correction

Original `gh run view 40945971854 --repo butahlecoq/Archy --log-failed` failed the old fixture's open-pipe assertion at `process.rs:487` (22/23 adapter unit tests passed). The first handoff recorded two old-fixture reproduction classes: a 1.2 s post-TERM scheduling pause and delayed `setsid`; the exact CI scheduling remains unknown. Sol's first review found another race: capture's 250 ms clock started before the test had pinned the escaped child and released the shell leader. Sol's `pthread_create` interposer paused the test thread 600 ms after capture thread creation; the old round-one fixture failed 3/3 with missing `escaped:` output. I reproduced that same failure 3/3 before this edit (`target/ci-review-probe/repro-*.log`).

The revised fixture gates capture immediately before its deadline starts. The capture thread has already spawned the real direct child and opened its pidfd, but waits on a test channel. The test receives the real group leader PID and retains a second pidfd. The background shell writes its PID before it is permitted to exec `setsid`; the test opens and retains that child's pidfd, then permits exec. After seeing its `ready` file, the test reads `/proc/<pid>/stat` and asserts the child is alive and its session ID equals its PID. It writes `ack`, waits until the real direct group leader is a zombie while its pidfd remains retained, and only then releases capture's gate. Thus the first TERM sees an already exited, pinned leader; the test also asserts `TimedOut`, capture's pidfd, successful TERM group signal, `leader_pinned_at_group_signal`, direct reaping, an open pipe, and the matching `escaped:` PID. The 1.2 s post-TERM pause continues to exercise late capture scheduling. The gate is test-only and cannot manufacture a production pass.

The channel sender is created inside `thread::scope`. A setup panic drops it, allowing capture to continue and terminate its original process group; this was verified after an initial diagnostic exposed and prompted correction of a sender-lifetime deadlock. The escaped child cannot leave that process group before the test has retained its pidfd. Once it has escaped, `EscapedFixture::drop` kills through the retained pidfd, without reopening a numeric PID, and removes its directory. There is no `ack` in Drop. The child has a bounded 10 s sleep after acknowledgment as a fallback if the test process itself is killed.

## Verification

- Old round-one binary with 600 ms post-`pthread_create` delay: 3/3 failed as reviewed; logs `target/ci-review-probe/repro-*.log`.
- Corrected binary with same delay: 3/3 passed after final edit; logs `target/ci-review-probe/final-*.log`, each with marker.
- Corrected binary with 600 ms delayed `/usr/bin/setsid` exec: passed, `target/ci-review-probe/fixed-setsid.log`.
- Deliberate 4 s delayed `setsid` exceeded the fixture's 3 s readiness bound and failed with `escaped child did not enter its session` in 4.46 s, as expected. The scope/RAII cleanup left zero `/bin/sleep 10` processes and no new `archy-escaped-*` directory. An earlier run exposed the channel deadlock and was stopped; its exact stale directory was removed. Log of corrected failure path: `target/ci-review-probe/fixed-4s-setsid-2.log`.
- Final corrected binary: 16/16 parallel repetitions with four CPU load workers passed; logs `target/ci-review-probe/final-parallel/`. A prior 32/32 loaded run passed before the channel-lifetime edit.
- `CARGO_TARGET_DIR="$PWD/target" cargo test --locked -p archy-adapters`: exit 0, 23 unit, 6 artifact, 4 isolation, 8 protocol tests passed; two environment-gated integration tests ignored.
- `CARGO_TARGET_DIR="$PWD/target" cargo fmt --all -- --check`: exit 0.
- `CARGO_TARGET_DIR="$PWD/target" cargo clippy --locked -p archy-adapters --all-targets -- -D warnings`: exit 0.
- `git diff --check`: exit 0. Final `pgrep -fc '^/bin/sleep 10$'` was 0 and `find /tmp -maxdepth 1 -type d -name 'archy-escaped-*'` returned no entries.
- Full workspace suite was not rerun; the focused adapter suite and review probes cover this test-only change.

## Independent review request

Please check gate placement relative to the deadline and first group signal; shell PID preservation through `exec setsid`; `/proc` session and zombie checks; retained pidfds and failure cleanup, especially panic during readiness; whether the assertions establish an actual open pipe and preexisting exited leader. Re-run the 600 ms post-spawn probe if needed. No new CI pass is claimed.
