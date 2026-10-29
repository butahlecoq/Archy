# Task03 storage round1 independent rereview

**Gate: addressed for the step-1 storage slice; CLI persistence integration may proceed.** This rereview is limited to the P1 revision-directory replay finding in `task03-storage-review-final.md` and breakage introduced by its fix. It does not accept A05 as a whole or certify power-loss durability on Omarchy. I edited only this review document; no source edits, commits, pushes or agents.

## P1 disposition

**Addressed.** `LockedProject::revisions` now returns the pinned `.archy/revisions` descriptor together with its validated chain (`crates/archy-core/src/storage.rs:527-577`). For an identical idempotency key and request, `begin` invokes the replay-side fault point, calls `sync_all()` on that descriptor, and only then returns the original receipt (`storage.rs:727-740`). A failed replay sync yields a retriable `INTERNAL_ERROR` with same-key retry guidance and no receipt. The existing changed-request conflict and stale-revision ordering remain after this branch.

`revision_rename_failure_replay_requires_successful_directory_sync` (`storage.rs:1118-1185`) covers both an injected failure immediately after revision rename and one at the committed revision-directory sync. It then injects failure at the replay sync, verifies no successful response, preserves the numbered record bytes and prior accepted run, and retries successfully to the same run ID and revision without another record. This directly exercises the failure branch missing from the prior matrix. The hook is test-only; production calls perform the real descriptor `sync_all()`.

I found no new source-level conflict in the changed `revisions` return value or its call sites: `revision`, `read_run` and `begin` use the updated tuple, and the focused suites compile and pass. The code and test demonstrate application ordering with synthetic EIO; they do not prove a real kernel `fsync` fault or sudden-power-loss survival.

## Independent focused evidence

All commands ran in `repository` on this frozen source snapshot and exited 0:

- `cargo test --locked -p archy-core --lib storage::interruption_tests::revision_rename_failure_replay_requires_successful_directory_sync` — 1 passed.
- `cargo test --locked -p archy-core --lib storage::` — 7 passed.
- `cargo test --locked -p archy-core --test storage --test run_integrity` — 17 storage integration and 2 run-integrity tests passed.

The updated `task03-storage-handoff.md` reports final workspace fmt, clippy and tests exiting 0. I did not rerun those broad commands in this scoped rereview.

## Next boundary

The step-1 core reader verifies structural metadata and hashes only. CLI step 2 must require the appropriate input/source evidence, check supported recorded algorithms, validate canonical variants and recompute full evaluations before using any status or approval flag. Consistently rehashed forged evaluation bytes remain untrusted. Full A05 reports/edits, committed storage schema artifacts, large aggregate-limit boundary tests, actual Omarchy/filesystem durability and backend acceptance remain separate gates.
