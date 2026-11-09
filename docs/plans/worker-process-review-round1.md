# A09 process lifecycle slice independent rereview, round 1

Date: 2026-10-29. **Scoped verdict: PASS for the test-only process slice.** The P1/P2 findings in `worker-process-review-final.md` are addressed on frozen `process.rs` SHA-256 `b1e4aa6c8071f4ed55f3776d937c862baf2e2ce69fb67c223fe32bb3b9179251`. This review changed only this document; no source, dependency, commit, push or agent action was performed.

## Finding adjudication

1. **P1 numeric group-ID reuse — resolved in this mechanism.** The loop now observes the direct child with `waitid(Pid, EXITED|NOHANG|NOWAIT)` without reaping it. The child remains waitable, pinning its numeric PID through all process-group TERM/KILL calls; `Child::try_wait` occurs only after the final possible group signal. `signal_group_if_pinned` refuses a numeric group signal if child identity was lost, while the pidfd remains only a direct-child signal reference. A new escaped-session pipeholder test exercises the exited-but-unreaped leader path, and a separate test checks refusal with a false pinned flag. This closes the reviewed PID-reuse code path; it does not make process groups a containment boundary for escaped descendants.
2. **P2 completion classification — resolved.** `new_reason` preserves a prior failure, then assigns newly observed output overflow, then recognizes a fully observed child exit with both pipes closed before considering new cancel/deadline signals. A completed exit can therefore retain its actual exit status, while I/O and overflow failures still win. The boundary regression covers completion with cancel/deadline and prior/new failures; existing timeout and cancellation tests remain passing.

## Fresh focused validation

| Command | Observed result |
| --- | --- |
| `sha256sum crates/archy-adapters/src/process.rs` | Matched the handoff's round1 source identity above. |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo test -p archy-adapters --locked` | Exit 0; 12 process unit tests and eight protocol integration tests passed, zero failures; one pre-existing native IFC integration test ignored by its explicit backend gate. |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo fmt -p archy-adapters -- --check` | Exit 0, no mutation. |

The author reported that current whole-workspace formatting fails in changing core/desktop files and strict adapter Clippy is blocked by `clippy::type_complexity` in `archy-core/src/storage.rs:481`. I did not rerun those shared gates or call them passing; Bacon owns their integration fix. The earlier full-workspace run on a different snapshot is not current evidence.

The module remains private under `#[cfg(all(test, target_os = "linux"))]` with rustix as a dev dependency. The observed bounded loop and honest reaping/EOF flags are development evidence only. Production fixed launch, cgroup/service descendant authority, isolation, quota enforcement, artifact verification, publication and actual Omarchy acceptance remain separate gates; this scoped pass closes none of them.
