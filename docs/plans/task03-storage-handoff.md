# Task03 step 1 storage handoff

**Status:** Bounded core storage slice implemented on 29 October 2026 for independent review. This is not A05 acceptance. No commit, push, agent dispatch, task02 geometry/evaluation/generation edit, or CLI/report edit was made for this slice. Actual Omarchy execution is deferred.

## Boundary and API

- `ProjectStore::open(project_file)` pins the project parent directory. `lock()` opens a persistent `.archy/lock` inode with nonblocking `File::try_lock`; the lock is held by `LockedProject` through `begin`, artifact writes, and commit. `read_source()` reads through the pinned root with the 1 MiB input cap.
- `begin(RunRequest)` validates a bounded versioned request, reads and verifies the entire accepted revision chain, performs idempotency lookup before stale revision rejection, and returns either a prior `RunReceipt` or a fresh `PendingRun`. The request digest includes the supplied expected revision, destination, input digest, operation, typed bytes and source run. Callers must derive the digest from the actual source bytes they read; core cannot infer application semantics.
- `PendingRun::write` creates only new project relative artifact paths, hashes exact bytes, and accepts at most 4096 artifacts, 16 MiB per artifact and 256 MiB in aggregate. `record_outcome` stores bounded references to existing variant/evaluation/error artifacts. `commit(RunMetadata)` requires project and algorithm metadata, freezes the manifest, synchronizes files and directories, publishes the run with `RENAME_NOREPLACE`, then writes and publishes a numbered revision record with another `RENAME_NOREPLACE`. Revision 0 is implicit genesis.
- `read_run` returns a privately constructed `VerifiedRun` exposing a strict manifest and hash checked bytes through accessors. It validates the full contiguous chain and referenced inventory, including file type/link count, manifest identity, request binding, bounds, and outcome references. **It does not validate geometry or evaluation truth.** A forged `required_checks_passed` with consistently updated hashes remains bytes; the CLI integrity layer in task03 step 2 must parse canonical input/variants and recompute full evaluations before using any result as trusted.
- `run_manifest_schema`, `revision_schema`, and `run_request_schema` expose source derived JSON Schemas. `cargo run --locked --quiet -p archy-core --example storage_contracts` emits them. Schema artifact registration is scheduled for task03 step 4.

## Confinement and recovery

All managed child paths use the pinned directory descriptors and rustix `openat2` with `BENEATH | NO_SYMLINKS | NO_MAGICLINKS | NO_XDEV`. Components reject absolute, parent, empty, dot, backslash, NUL, overlong and deeply nested forms. Managed directories require private permissions and the project owner's UID; evidence files require a unique regular inode. No ordinary replacement rename or path based reopen is used. Root relocation after open is covered by a test. The implementation treats hostile same user writes as detectable corruption, not an authentication boundary.

Unaccepted staging remains for diagnosis. If a run directory was published but no revision was recorded, a same key and same request retry scans UUID named orphan directories, checks the complete inventory and manifest binding, and commits exactly one revision. A changed request or stale expected revision conflicts. If a revision was published before response, a retry returns its original receipt. An uncertain post publication synchronization or revision publication error is retriable with the same key; it is never reported as rollback. Corrupt accepted history blocks mutation. Staging/orphans are not automatically removed.

## Dependency resolution and license decision

`cargo fetch` resolved exact `rustix 1.1.4` and `uuid 1.29.0` in `Cargo.lock` with Rust 1.99.1. `cargo tree --locked -p archy-core --depth 2` showed rustix's direct `bitflags 2.13.2` and `linux-raw-sys 0.12.1`, and uuid's `getrandom 0.4.3` on this Linux graph. Installed package metadata declares rustix and linux-raw-sys `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`; uuid `Apache-2.0 OR MIT`; bitflags and getrandom `MIT OR Apache-2.0`. The core chooses the MIT option where offered. The lockfile and package metadata establish candidate resolution and declared licenses, not a complete distribution notices audit. `rustix` provides the explicit Linux resolution and no-replace primitives; `uuid` provides random execution IDs separate from deterministic variant IDs. No second capability library or generic database was added.

## Exact local checks and moving-workspace limit

- `cargo test --offline -p archy-core && cargo clippy --offline -p archy-core --all-targets -- -D warnings`: exit 0 after the final required-metadata API change. Storage unit tests 4, storage integration tests 11, run integrity tests 2, preexisting core validation 12. The child abort test terminates a child after run publication and the parent recovers its orphan once. Other tests cover real process lock contention, stale/different key requests, rename target occupancy, traversal, symlink/hardlink substitution, pinned root replacement, payload tamper and revision gap.
- `rustfmt --check --edition 2024 --config skip_children=true` on all seven new/modified core storage source, test and example files: exit 0.
- An earlier integrated checkpoint on the then-current source ran `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace`: all exit 0. That checkpoint preceded the final `commit(RunMetadata)` API change and later concurrent doctor edits, so it is **not** final integrated evidence.
- Latest integrated rerun: `cargo fmt --all -- --check` exit 1 from concurrent formatting edits in `archy-cli/src/doctor.rs`; `cargo clippy --locked --workspace --all-targets -- -D warnings` and `cargo test --locked --workspace` exit 101 from five missing `Instant` arguments in the same actively edited doctor module. The doctor author owns those edits; no CLI file was modified by this storage slice.
- `cargo run --locked --quiet -p archy-core --example storage_contracts`: exit 0; emits `revision`, `run-manifest`, `run-request` schemas.
- `python3 scripts/check-schemas.py`: exit 1, `KeyError: 'doctor'` at the observed checkpoint. The doctor author owns checker registration; this remains a workspace gate pending its rerun.
- `git diff --check -- Cargo.toml crates/archy-core/Cargo.toml crates/archy-core/src/lib.rs`: exit 2 from preserved preexisting CRLF tracked file changes in both Cargo.toml files. No EOL normalization was applied.
- Environment: Linux 6.6.87.2-microsoft-standard-WSL2 x86_64; rustc 1.99.1. Target Omarchy/filesystem acceptance has not run.

## Review focus and genuine gaps

Review descriptor handling, no replace ordering, orphan selection under adversarial directory contents, bounded allocations, source file replacement, and all uncertain commit responses. Tests exercise failures at occupied file and rename targets and injected interruption after run/revision publication; they do **not** inject actual `fsync` failures or prove sudden power loss durability. Network filesystem durability is unclaimed. The manifest carries outcome references, but step 2 still owns semantic evaluation recomputation and explicit rejection of unknown algorithm versions. There are no CLI persisted runs or reports yet. Step 4 still owns committed JSON schema artifacts and checker registration. Independent storage review is required before integration is accepted.

## Preliminary review fixes and final test snapshot

This section supersedes the moving-workspace check results and fsync coverage statement above. The independent preliminary review remains unchanged in `task-03-storage-review-preliminary.md`; its four findings were addressed in this source snapshot:

1. **Orphan durability ordering:** same-key recovery now synchronizes both the staging parent and destination directory after checking the orphan and before publishing its revision. `orphan_retries_both_rename_parent_syncs_before_revision_commit` injects an EIO at each original post-rename parent sync, then at the corresponding recovery sync. It observes the prior run readable, the orphan preserved, revision still 1 and a retriable error; a subsequent same-key retry accepts that orphan as revision 2 exactly once.
2. **Allocation bounds:** `begin` checks `source_run` as a 36-byte UUID before request serialization; `commit` checks `source_format` as `json`/`yaml`, `parent_variant_id` as a 72-byte canonical `variant-` ID, and optional normalized digest before manifest serialization. Named tests check valid boundary values, one-over values, no manifest creation on invalid metadata, and no extra staging on invalid request. Public byte boundary tests accept exactly 1 MiB input/request and 16 MiB artifact and reject one byte over.
3. **Accepted-history error boundary:** malformed persisted receipt paths and unsafe numbered revision files now return `ARTIFACT_INVALID` from `revision`, `read_run` and `begin`. `corrupt_receipt_path_and_revision_link_are_artifact_invalid` covers both valid-JSON `../escape` and a symlinked numbered record without following the outside link.
4. **Versioned request fingerprint:** `RunRequest.schema_version` is explicitly `archy-run-request-v1`; manifest and revision records persist that version. The SHA-256 fingerprint is domain separated with `archy-run-request-fingerprint-v1` before typed serialized request bytes. Unsupported request versions fail before mutation with an explicit schema error. `unsupported_request_version_preserves_v1_receipt_and_bytes` checks the serialized v1 marker, rejection of v2, unchanged record bytes and original readable payload.

The private `FaultPoint` setter is compiled only under `cfg(test)`; production callers cannot invoke it. `injected_sync_and_publication_failures_preserve_history_and_replay_once` injects synthetic EIO at manifest file sync, staging directory sync, before/after run rename, both run-rename parent syncs, before/after revision rename and committed revision directory sync. The test runs real filesystem writes and renames around each injected point, then checks prior evidence, uncertain status where publication may have occurred, same-key receipt recovery and one accepted revision. The existing child-abort test remains separate. These tests exercise application ordering and error handling; they are **not** actual kernel `fsync` failure or sudden-power-loss proof.

Fresh commands on this final source snapshot, all exit 0:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace` (core storage unit 6, storage integration 17, run integrity 2, existing core validation 12; all other workspace suites passed)
- `python3 scripts/check-schemas.py` (doctor registration now present; seven schemas and CLI fixtures checked)
- `cargo run --locked --quiet -p archy-core --example storage_contracts` (run request, manifest and revision schemas emitted)

Still open for independent storage rereview: whether the sync ordering and synthetic fault model meet the first-slice gate; target Omarchy/filesystem durability; optional source/normalized artifact semantics and supported algorithm recomputation in step 2; committed storage schema artifacts in step 4. Large aggregate run, manifest, revision and directory-entry limits are enforced in code but do not have at/over boundary tests at their full sizes. No power-loss or network-filesystem durability claim is made.

## Final review P1 fix: revision replay synchronization

The preserved `task03-storage-review-final.md` found that a revision record can be visible after `RENAME_NOREPLACE` while the revision directory sync has failed. The same-key replay path previously returned the visible receipt without retrying that sync. `LockedProject::revisions` now returns the same pinned `.archy/revisions` descriptor used to validate the chain. When `begin` finds an identical key and request, it synchronizes that descriptor before returning `Replay`. A replay-side sync failure returns `INTERNAL_ERROR`, retriable true, with same-key retry guidance and no receipt. A subsequent successful sync returns the original recorded receipt; no new revision is written.

`revision_rename_failure_replay_requires_successful_directory_sync` covers both `AfterRevisionRename` and `RevisionDirectorySync` synthetic EIO. It captures the original numbered record bytes, injects a second failure at `ReplayRevisionDirectorySync`, checks the uncertain error and absence of a successful response, then retries successfully. Both retries preserve the original record bytes and run ID; the accepted revision remains 2 and the earlier run remains readable. The replay fault hook is private and test-only; production callers cannot arm it. This checks application ordering, not sudden-power-loss durability or a real kernel `fsync` failure.

Red evidence: `cargo test --locked -p archy-core --lib storage::interruption_tests::revision_rename_failure_replay_requires_successful_directory_sync` exited 101 with `Option::unwrap()` on `None`: the old replay path returned a receipt while the replay sync fault was armed. After the fix, the same command exited 0 (1 passed).

Final checks on this frozen storage source snapshot, all exit 0:

- `cargo test --locked -p archy-core --lib storage::` — 7 passed.
- `cargo test --locked -p archy-core --test storage --test run_integrity` — 17 storage and 2 run-integrity tests passed.
- `cargo fmt --all -- --check`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`.
- `cargo test --locked --workspace` — all suites passed, including the seven storage unit tests.

The storage source is frozen for Newton’s independent rereview of this P1 fix. Full A05 CLI/report integration and actual Omarchy/filesystem acceptance remain pending.
