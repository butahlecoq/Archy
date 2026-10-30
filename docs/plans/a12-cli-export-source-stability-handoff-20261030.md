# A12 CLI IFC export retry source stability

Base: `8e6c8987431a3c613563345ee38f95f530181b12`. This isolated candidate changes only CLI export selection and its tests. Independent review is pending.

Plan: inspect the public receipt/error/request contract and accepted history, reproduce the retry sequence, then bind an implicit source to the original request. No dependency changes.

The CLI accepts optional `--run`; MCP supplies an explicit `source_run`. An explicit idempotency key requires an expected revision. Accepted revisions record the key and request digest, while accepted export manifests record the parent source. The storage writer replays only an identical request digest, rejects stale new requests, and validates recovered orphans under the publication lock.

The CLI now takes an accepted export's recorded parent for an implicit retry. With no accepted export for the key, it selects the latest accepted generation or edit at or before the supplied expected revision. Explicit sources continue through their existing equality check and the same checked recovery path. A later generation cannot replace the source of a failed export retry; storage reports the stale revision before another worker attempt. The regression exercises failed export, later accepted generation, then exact-key implicit retry, including error fields and unchanged revision. The existing native orphan/replay case also retries implicitly after a later generation.

Verification: the new regression failed before the fix with `INVALID_SCHEMA` instead of `REVISION_CONFLICT`; after the fix it passed, including the final error-field assertions. `cargo test --locked --offline -p archy-cli --lib -p archy-cli --test core_workflow` passed 23 unit and 23 workflow tests; the native IFC tests in that run returned early because their opt-in environment was unset. A separate opt-in `native_valid_export_orphan_replays_exact_key` run passed after local offline installation of the pinned `web-ifc` package. `cargo fmt --all -- --check` and `cargo clippy --locked --offline -p archy-cli --all-targets -- -D warnings` passed.

Native replay command: `ARCHY_IFC_TEST_PYTHON=/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python ARCHY_IFC_TEST_NODE=/home/butahlecoq/.nvm/versions/node/v22.22.2/bin/node CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a12-cli-source-stability-20261030 cargo test --locked --offline -p archy-cli --lib native_valid_export_orphan_replays_exact_key -- --exact export_ifc::tests::native_valid_export_orphan_replays_exact_key` (1 passed, 0 failed). The same isolated target directory was used for the other Cargo checks.

The other unmerged runtime candidate also edits `crates/archy-cli/src/export_ifc.rs`. Reconcile the two diffs before integration. This freeze makes no host-service, push, release, target-runtime, professional-approval, or human-approval claim.
