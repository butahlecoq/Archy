# A17 receipt-loss boundary

## Plan

Use the existing process-test harness and public `ProjectStore` API. Accept revision 1, then run a child that commits revision 2 and aborts before it can deliver a receipt to its caller. Reopen the store, retry the identical request, and compare the returned receipt, revision count, and prior accepted byte inventory. Check a changed request and a stale new key. Keep the production storage implementation unchanged unless the reproduction exposes a root-cause defect.

This adds process-death evidence at the accepted-revision/undelivered-receipt boundary. The existing synthetic revision-publication test and staged-writer test exercise different boundaries. No new dependency is needed.

## Review and handoff

Base: `origin/feat/release-0.1` at `a5a61c8e565d50928a7ccbf950ac061691eccf04`. Isolated worktree: `/home/butahlecoq/work/archy/.worktrees/a17-recovery-boundary-20261029`. No product source, dependencies, CI, host installation, other worktree, commit, or remote was changed.

The initial reproduction added only a child that used the public store API to commit revision 2 and then called `abort()` before delivering its receipt, plus a parent that checked the abort signal and reopened revision 2. `lost_receipt_after_commit_is_replayed` exited 0 with signal 6 observed and revision 2 readable. The final test adds the accepted-byte inventory, original receipt replay, single-apply history, retained artifact, empty staging, changed-request conflict, and stale-new-key conflict assertions. The final focused run and full core suite exited 0. No root-cause product fix was needed for this exercised boundary.

Changed files: `crates/archy-core/tests/recovery_staging.rs` (two test functions) and `docs/recovery.md` (boundary and limits). This plan is the review handoff. The existing fixture and child supervision code is reused; no new dependency was introduced.

Commands from this worktree, with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-boundary-20261029` for every Cargo command:

```sh
ulimit -c 0; cargo test --locked --offline -p archy-core --test recovery_staging
ulimit -c 0; cargo test --locked --offline -p archy-core --test recovery_staging lost_receipt_after_commit_is_replayed -- --nocapture
ulimit -c 0; cargo test --locked --offline -p archy-core
cargo clippy --locked --offline -p archy-core --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The baseline focused suite passed 8/8 before the change. The initial reproduction and final focused case each passed 1/1. The full core suite passed 7 unit, 10 `recovery_staging`, 2 run-integrity, 21 storage, 12 validation, and the `fork_lock` lifecycle checks. Clippy, formatting, and diff checks exited 0. The host was Linux `6.6.87.2-microsoft-standard-WSL2`, Rust `1.99.1`.

Final SHA-256 review inputs:

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `crates/archy-core/src/storage.rs` | `905a2934ae681e02710a3dccdfccde9a6cdb6aaae47cf3352ab417843e823d99` |
| `crates/archy-core/tests/recovery_staging.rs` | `f6ad132db25d46c688896925862be0edfc37cd48c02555e6031dd7e5d6f34f3a` |
| `docs/recovery.md` | `edca52bdc42f8fb9e39d94a731ac6aaab99b474795f16fc9589f041c3be46ce7` |

This tests process death **after `commit` returned inside the child** and before any receipt reached its parent. It does not kill within the revision rename or sync sequence, and it does not test sudden power loss, network filesystem durability, Omarchy, backend failure logs or cancellation, MCP replay, or complete deterministic artifact equality. The second artifact is fixed fixture bytes, not a generated design. Independent review remains open; reread the exact source at the hashes above, then run the focused test and check that the child actually aborts and that the parent observes no additional revision on replay. Full A17 acceptance remains open for the other fault classes and transaction boundaries.
