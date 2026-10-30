# Selected IFC orphan retry coverage

Base: `5d02809a57a662785d07f753c820d173d7ed1a09`. Scope: direct native orphan retry for selected v1 JSON, v1 YAML, and v2 YAML IFC exports. No production logic, deadlines, or dependency versions changed.

Plan: reuse the existing native export/replay matrices. After a verified export, remove only its revision record to simulate an orphan, retry with the identical request and expected revision, and fail if the worker runs. Assert the original receipt and revision are recovered, and that the project source, verified input, manifest, and IFC model bytes are unchanged. Cover rectangle and L shapes for each missing format/profile case.

Dependency rationale: the tests use the existing Python 3.12/IfcOpenShell 0.9.0.post1 worker fixture, Node 22.22.2, and pinned local web-ifc 0.0.78 modules. No packages were installed. This isolated worktree used a temporary local link to the existing module installation, removed after testing.

Validation: `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a12-orphan-v1-yaml-coverage-20261030 ARCHY_IFC_TEST_PYTHON=/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python ARCHY_IFC_TEST_NODE=/home/butahlecoq/.nvm/versions/node/v22.22.2/bin/node ARCHY_IFC_READER_NODE_PATH=tests/backend/ifc-reader/node_modules TMPDIR=/var/tmp cargo test --locked --offline -p archy-cli --lib export_ifc::tests::native_ -- --nocapture` passed: 4 tests, 0 failed, 0 ignored, 18 filtered out, 191.91 seconds. The earlier attempt without the local web-ifc link failed 4 tests with `BackendUnavailable` before the new assertions; it is not counted as passing. The rerun with the pinned modules passed.

`cargo clippy --locked --offline -p archy-cli --all-targets -- -D warnings` failed in unchanged `archy-core/src/storage.rs` on two `type_complexity` findings and one `too_many_arguments` finding. The same command with `-A clippy::type_complexity -A clippy::too_many_arguments` passed, including the changed test code. Both used the same offline Cargo environment and dedicated target directory above.

Independent review: pending. This is a frozen coverage commit for review, not a target-runtime, approval, or integration claim.

## Published-base follow-up

The user subsequently reported that `1452c449b8d0bea0d7efee34ea98769b9540b4f3` passed independent review and a pinned native 4/4 run. Its coverage commit was cherry-picked onto `54bc0b80aaf1635112f6a3d2c79b5bd9c499b1ef` as `0a06fc1`; the IFC test diff was compared byte for byte with the original and matched. The earlier pending-review statement describes the original handoff, before that report.

All follow-up Cargo commands used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a12-orphan-coverage-54bc-followup-20261030`. The native test also used `ARCHY_IFC_TEST_PYTHON=/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python`, `ARCHY_IFC_TEST_NODE=/home/butahlecoq/.nvm/versions/node/v22.22.2/bin/node`, `ARCHY_IFC_READER_NODE_PATH=tests/backend/ifc-reader/node_modules`, and `TMPDIR=/var/tmp`. A temporary link in this worktree pointed to the existing pinned web-ifc modules; it was removed after the run. Python was 3.12.16 with IfcOpenShell 0.9.0.post1, Node was 22.22.2, and the web-ifc JS, WASM, and package hashes matched the verifier's pinned hashes.

- `cargo test --locked --offline -p archy-cli --lib export_ifc::tests::native_ -- --nocapture`: passed 4, failed 0, ignored 0, filtered out 18; test execution took 111.84 seconds.
- `cargo clippy --locked --offline -p archy-cli --all-targets -- -D warnings`: passed, without suppressions.
- `cargo fmt --all -- --check` and `git diff --check 54bc0b80aaf1635112f6a3d2c79b5bd9c499b1ef HEAD`: passed.

This run used the native fixture and did not exercise a host service. The follow-up was not independently reviewed in this session.
