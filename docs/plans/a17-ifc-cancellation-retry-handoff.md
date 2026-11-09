# A17 IFC cancellation retry identity handoff

Base: `a098a7f57b20edb19f04627ec96b280291913d58`. Work was isolated in a new outer worktree.

Plan: inspect recovery and fault coverage; reproduce one ordinary local cancellation gap; change only the error path after an empty staged IFC export; verify accepted history, retry identity, and existing recovery suites. No new dependencies are needed.

Existing coverage includes a killed core staging writer (`recovery_staging.rs`), a killed staged edit (`edit_interruption.rs`), storage publication faults (`storage.rs`), and worker cancellation in the adapter process unit test. The IFC export unit tests covered worker failure without publication, but did not check the generated retry identity after the worker returned `BackendTimeout`.

Reproduction at the base: the new `cancelled_ifc_worker_returns_generated_retry_identity_without_publication` test failed with `ArchyError { code: BackendTimeout, message: "worker cancelled", field: "service", retriable: false }`. The runner is injected; no service or IfcOpenShell is required.

Fix: after the empty pending run is discarded, classify `BackendTimeout` as retriable and append the resolved idempotency key and expected revision. The test checks those values, uses the returned key for another attempt, checks staging is empty, and compares the accepted input, manifest, and revision bytes before and after cancellation. `BackendUnavailable` remains unchanged because it can report uncertain service teardown.

Validation on this worktree with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a17-cancellation-20261030`:

- `cargo test --locked --offline -p archy-cli --lib export_ifc::tests::cancelled_ifc_worker_returns_generated_retry_identity_without_publication`: failed before the fix as above; passed after.
- `cargo test --locked --offline -p archy-cli --lib`: 32 passed, 5 ignored.
- `cargo test --locked --offline -p archy-core --test recovery_staging -p archy-cli --test edit_interruption`: 11 and 2 passed.
- `cargo test --locked --offline -p archy-cli -p archy-core`: exit 0; package tests and doc tests completed, with the existing environment gated cases ignored.
- `cargo fmt --all -- --check`: passed.
- After narrowing classification to `BackendTimeout`, `cargo test --locked --offline -p archy-cli --lib` passed again (32 passed, 5 ignored), as did `cargo clippy --locked --offline -p archy-cli --lib -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check`.

Independent review remains pending. Review the retry classification and error identity at the worker failure branch, the unchanged accepted bytes, and the local runner test's boundary. This does not establish installed service teardown, native IfcOpenShell behavior, target Omarchy acceptance, or power loss durability.
