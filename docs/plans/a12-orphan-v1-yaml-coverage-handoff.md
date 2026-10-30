# Selected IFC orphan retry coverage

Base: `5d02809a57a662785d07f753c820d173d7ed1a09`. Scope: direct native orphan retry for selected v1 JSON, v1 YAML, and v2 YAML IFC exports. No production logic, deadlines, or dependency versions changed.

Plan: reuse the existing native export/replay matrices. After a verified export, remove only its revision record to simulate an orphan, retry with the identical request and expected revision, and fail if the worker runs. Assert the original receipt and revision are recovered, and that the project source, verified input, manifest, and IFC model bytes are unchanged. Cover rectangle and L shapes for each missing format/profile case.

Dependency rationale: the tests use the existing Python 3.12/IfcOpenShell 0.9.0.post1 worker fixture, Node 22.22.2, and pinned local web-ifc 0.0.78 modules. No packages were installed. This isolated worktree used a temporary local link to the existing module installation, removed after testing.

Validation: `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a12-orphan-v1-yaml-coverage-20261030 ARCHY_IFC_TEST_PYTHON=/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python ARCHY_IFC_TEST_NODE=/home/butahlecoq/.nvm/versions/node/v22.22.2/bin/node ARCHY_IFC_READER_NODE_PATH=tests/backend/ifc-reader/node_modules TMPDIR=/var/tmp cargo test --locked --offline -p archy-cli --lib export_ifc::tests::native_ -- --nocapture` passed: 4 tests, 0 failed, 0 ignored, 18 filtered out, 191.91 seconds. The earlier attempt without the local web-ifc link failed 4 tests with `BackendUnavailable` before the new assertions; it is not counted as passing. The rerun with the pinned modules passed.

`cargo clippy --locked --offline -p archy-cli --all-targets -- -D warnings` failed in unchanged `archy-core/src/storage.rs` on two `type_complexity` findings and one `too_many_arguments` finding. The same command with `-A clippy::type_complexity -A clippy::too_many_arguments` passed, including the changed test code. Both used the same offline Cargo environment and dedicated target directory above.

Independent review: pending. This is a frozen coverage commit for review, not a target-runtime, approval, or integration claim.
