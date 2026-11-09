# Independent round-two review: escaped pipeholder fixture

## Verdict

**Approve the revised test-only fixture for this scope.** The earlier 600 ms post-spawn race is closed by a channel gate before the capture deadline. The test establishes that the real direct group leader is an unreaped zombie and that the escaped child is a live separate-session leader before capture can send TERM. Fresh timing, failure, adapter, and load checks below passed. This is a local review of the uncommitted worktree, not a claim that the red GitHub Actions run has turned green.

## Frozen inputs

- Worktree: `/home/butahlecoq/work/archy/.worktrees/ci-escaped-pipeholder-40945971854`; HEAD `8e36e11d1399dee37dc2b85a8e2225b473571602`.
- `sha256sum crates/archy-adapters/src/process.rs`: `a519321c480723e09eb2e417c5ac10c7d231373c5159ff0407653c31d66197ee`.
- `git diff -- crates/archy-adapters/src/process.rs | sha256sum`: `0e1c5e17367129fd8e41ae45fb3dd4b9296304dec2036dee760da7782476a4f2`.
- `git show HEAD:crates/archy-adapters/src/process.rs | sha256sum`: `b1e4aa6c8071f4ed55f3776d937c862baf2e2ce69fb67c223fe32bb3b9179251`.
- Read `AGENTS.md`, the round-two handoff, the previous independent review, and the full `process.rs` diff. No source edit, commit, push, install, other worktree write, or child agent. Cargo and diagnostic outputs stayed under this worktree's `target/`; `findmnt` reported `/dev/sdd ext4 /` for that target.

## Ordering and signal evidence

- `capture_with_term_pause` spawns the real child, opens its pidfd, prepares its pipes, calls `before_deadline(pid)`, and only then takes `Instant::now()` for the 250 ms wall limit (`process.rs:192-222`). The capture thread blocks in `start_rx.recv()` (`:553-563`). The previous delayed-handshake failure cannot consume that deadline while fixture setup runs.
- The background shell writes its own `$!` into `pid` while waiting for `go`; `EscapedFixture::pin` opens and retains that PID's pidfd before writing `go` (`:395-411`, `:545-548`). Its shell then `exec`s `setsid`; the test waits for `ready`, checks `/proc/<pid>/stat` is non-zombie, and checks session ID equals PID (`:568-576`). The pidfd remains held through capture and is used by `Drop` to kill the escaped child (`:420-426`). The checked PID matches the captured `escaped:` line (`:607-617`).
- The test also opens a pidfd for the direct leader, releases `ack`, waits until `/proc/<leader>/stat` reports `Z`, and only then sends the capture gate token (`:565-588`). Capture's first `waitid(Pid, EXITED|NOHANG|NOWAIT)` sees the unreaped leader before the timeout signal path (`:231-272`). Only capture can reap it, after group signaling (`:310`). The test checks `TimedOut`, capture pidfd opened, leader pinned at a group signal, TERM group syscall success, direct reaping, and at least one pipe without EOF (`:601-606`). The live escaped child inherited the output pipes and remains outside the signaled process group until capture returns (`:590-600`).
- `term_group_sent` means `kill_process_group` returned success; it does not prove a live member received TERM. Here the relevant behavior is that capture attempted the group signal against the pinned group identity while a separate-session descendant still held a pipe. The earlier OR-based `leader_pinned_at_group_signal` alone was insufficient, but the pre-release zombie observation and gate now establish first-signal ordering.
- `process.rs` is included by `lib.rs:6-7` only under `#[cfg(all(test, target_os = "linux"))]`. Normal `capture` passes a zero pause and no-op gate; production builds do not include this module. No production runtime behavior changed.

## Fresh reproduction and checks

All commands used this worktree. Timing probes used the already present, inspected `pthread_create` and `execve` interposers in `target/ci-review-probe/`; each test process had an external `timeout 12s` guard. Logs and markers are in `target/ci-review-probe/round2/`.

| Check | Fresh result |
| --- | --- |
| 600 ms after `pthread_create`, exact escaped test, three runs | 3/3 exit 0; each marker had two pauses; each log reports one passed in about 2.07-2.08 s (`spawn-{1,2,3}.*`). |
| 600 ms before `/usr/bin/setsid` exec | Exit 0; marker present; one passed in 2.09 s (`setsid.*`). |
| 4 s before `setsid` exec, exceeding readiness bound | Exit 101 with `escaped child did not enter its session` at `:406`, one failed in 4.46 s; external timeout did not fire (`setsid-4s.*`). |
| Forced `setsid` `execve` failure, so `ready` never appears | Exit 101 with the same bounded readiness panic in 4.46 s; external timeout did not fire (`deny.*`). |
| Four `yes > /dev/null` CPU workers; 16 exact-test processes at `xargs -P 8` | 16/16 passed; `xargs` exit 0 (`load/`). Workers were killed and waited by an EXIT trap. |
| `CARGO_TARGET_DIR="$PWD/target" cargo test --locked -p archy-adapters` | Exit 0: 23 unit, 6 artifact, 4 isolation, 8 protocol passed; two environment-gated integration tests ignored. |
| `cargo fmt --all -- --check` with same target; `cargo clippy --locked -p archy-adapters --all-targets -- -D warnings`; `git diff --check` | Each exit 0. |

After the failure and load probes, `pgrep -af '^/bin/sleep 10$'` and `find /tmp -maxdepth 1 -type d -name 'archy-escaped-*' -print` returned no entries. The failed setup unwinds the scoped sender: `start_tx` drops, capture resumes and terminates its original group; the retained escaped pidfd is signaled in `Drop`. The fixture does not write `ack` on a failure path. Before `pidfd_open`, the child waits for `go` in the original group, so failure there leaves group teardown available. These probes cover the delayed and failed `setsid` paths and the resulting panic cleanup; they do not establish behavior under arbitrary kernel, filesystem, or scheduler stalls. The 3 s setup waits, 250 ms capture wall limit, grace periods, 1.2 s test pause, and child 10 s fallback are finite under ordinary scheduling, but no wall-clock guarantee is claimed for a wedged syscall or process.

## Remaining scope

The fixture can still fail under extreme scheduling beyond its 3 s readiness bound or if the child lives beyond its 10 s post-ack fallback before capture completes. Such a failure is bounded in the exercised paths and does not recreate the prior false positive. The original GitHub Actions run is still red; this review does not claim a new CI result. Full workspace tests and real backend gates were outside this test-only review.
