# A17 receipt-loss boundary: independent Sol review

**Verdict: PASS for the scoped post-commit, undelivered-receipt recovery test.** No verified finding against this boundary. This does not close A17 or establish release acceptance.

## Scope and exact inputs

Reviewed the uncommitted `recovery_staging.rs` and `docs/recovery.md` changes plus `docs/plans/a17-recovery-receipt-loss.md` in `/home/butahlecoq/work/archy/.worktrees/a17-recovery-boundary-20261029`, at `HEAD a5a61c8e565d50928a7ccbf950ac061691eccf04`. Read the production storage path without changing it. SHA-256 values independently recalculated from this worktree:

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `crates/archy-core/src/storage.rs` | `905a2934ae681e02710a3dccdfccde9a6cdb6aaae47cf3352ab417843e823d99` |
| `crates/archy-core/tests/recovery_staging.rs` | `f6ad132db25d46c688896925862be0edfc37cd48c02555e6031dd7e5d6f34f3a` |
| `docs/recovery.md` | `edca52bdc42f8fb9e39d94a731ac6aaab99b474795f16fc9589f041c3be46ce7` |

## Evidence and interpretation

- The child uses the public `ProjectStore` API, begins at expected revision 1, writes fixed `data` bytes, calls `pending.commit(metadata()).unwrap()`, discards the returned receipt, then calls `std::process::abort()`. The parent requires signal 6 and sees revision 2 after reopening. Thus the observed death is after a successful `commit` return and before any receipt is delivered to the parent. It is a real process abort, not a kill during publication.
- `commit_inner` builds one receipt, clones it into the revision record, calls `publish_revision`, and returns that receipt. The parent validates the accepted history, checks the recorded second receipt and its artifact, and compares same-key, same-request replay with that exact recorded receipt. The child does not transmit its return value, so direct parent comparison with a captured child receipt is impossible by design; the source establishes the identity of the returned and recorded receipts.
- The parent checks two receipts, revision 2 before and after replay, unchanged receipt list, unchanged revision-2 JSON bytes, empty staging, and unchanged accepted-file inventories. The inventory helper hashes every immediate file in each fixture run directory and revision 1; the test separately compares revision-2 bytes. This is complete for these flat fixture runs, not a general recursive project inventory.
- Reusing the key with changed request bytes and using a new key at stale expected revision 1 both return `RevisionConflict`. In `LockedProject::begin`, matching key/digest replay precedes the expected-revision check, while a changed digest conflicts and an unseen key with stale revision conflicts before staging creation.
- The parent reaps the aborting child through `try_wait`, clears the writer handle, and the `Fixture` drop removes its private root. `Writer::Drop` kills and waits if a child remains on an early parent unwind. The new case does not explicitly assert root deletion; teardown follows the existing fixture lifecycle.

## Verification

Ran from this worktree with `ulimit -c 0`, `RUSTUP_AUTO_INSTALL=0`, `CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=2`, and fresh `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-sol-review-20261029`:

```sh
cargo test --locked --offline -p archy-core --test recovery_staging -- --nocapture
cargo test --locked --offline -p archy-core --test recovery_staging lost_receipt_after_commit_is_replayed -- --exact --nocapture
git diff --check
```

Results: focused suite 10 passed, 0 failed; isolated receipt-loss test 1 passed, 0 failed; diff check clean. Several direct helper-test entries in the suite are guarded no-ops without the inherited fixture descriptor. The receipt-loss parent test exercises the child and its assertions. No further probe was warranted after the process result and source-path inspection agreed.

## Limits and integration note

This test does not terminate the child inside the run or revision rename/sync sequence. Existing test-only fault injections cover different publication boundaries, not this process-abort timing. It does not prove sudden power-loss durability, network filesystem behavior, Omarchy, backend cancellation or logs, MCP replay, or deterministic generated artifact equality; its second artifact is fixed fixture bytes. The recovery documentation states the process-death boundary accurately.

Integration commit `7b53b1001ae94725177e12c6383debcd9b281f4d` contains separate readiness and process-exit race fixes in the same test file. This review was run against the isolated `a5a61c8` worktree and does not validate the combined integration tree. No integration worktree, source file, commit, or remote was changed by this review.
