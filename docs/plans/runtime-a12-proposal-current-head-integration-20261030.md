# Runtime, A12 and proposal current-head integration

Base: PR head `54aa9f78719ec11e72ef22379b6391611dfa6810`. Source: independently reviewed runtime+A12 candidate `28c24cbc4c1b0c8047a413fe4fe6ae1948c59407`. This is a review candidate, not a release or target acceptance. During validation, the PR head advanced to `c41caa695af9a939a829f04db3c3ab3bca17c737`; its desktop export status change requires a subsequent reconciliation and is not included here.

Plan: merge in a new worktree; keep the identical A12 patch once, resolve the IFC retry overlap, verify proposal and runtime contracts, and hand off the resulting commit for separate review. The A12 edits in `5796a9a` and `32818b7` have the same stable patch ID (`30bdb46730390ab7408dfd272329d92a2ea147bd`). No dependency was added by the resolution; the runtime candidate's `rustix` `fs` feature is retained.

The only textual conflict was in `export_with_reader` in `export_ifc.rs`. The resolution maps snapshot, accepted-head, key-scan and implicit-source-scan runtime failures to the A23 pre-lookup retry identity. A12's recorded parent selection for an implicit retry remains in place, as do the authoritative locked key check and checked recovery. The automatic `run_integrity.rs` and `project_read.rs` merges retain proposal verification and add the A23 test failure hooks. `proposal.rs`, storage, the MCP proposal contract, and its schema are byte-identical to the PR head. The runtime pin manifest, staging code and verifier are byte-identical to the reviewed runtime candidate.

Validation completed before commit:

- `cargo fmt --all -- --check`: passed.
- `cargo test -p archy-cli --lib export_ifc::tests -- --nocapture`: 12 passed, 2 ignored (native fixture requirements).
- `python3 -m unittest -v tests.packaging.test_runtime_a12_contract`: 2 passed.
- `python3 -m unittest -v tests.packaging.test_runtime tests.packaging.test_runtime_snapshot`: 9 passed.
- `python3 -m unittest -v tests.packaging.test_runtime_confinement tests.packaging.test_package`: 33 passed.
- `cargo build -p archy-cli -p archy-mcp -p archy-service-gate`: passed.
- `python3 crates/archy-mcp/tests/propose_approval.py`: passed live proposal transaction, replay, conflicts, no approval projection, and historical v1 authority refusal.
- `python3 crates/archy-mcp/tests/export_ifc.py`: passed MCP contract with the absent backend; no false receipt or revision.
- `cargo test -p archy-cli --test desktop_readers`: 8 passed.
- `cargo test -p archy-service-gate`: 9 passed, 1 ignored (fixed-path Blender fixture in disposable container).
- `python3 scripts/check-schemas.py`: passed schema, live CLI/MCP, proposal, export, and 12 demo tests; doctor target acceptance remained deferred.
- `packaging/arch/runtime_manifest.py --source . --identity`: input identity `ae620ba494bef9e7639200e00e29ba1128bce840c13df460c773ac1d96880abc`; `build_ready: false`.
- Stable patch ID comparison and byte comparisons against both parent candidates: passed.

The A12 and distribution review allowlists remain empty. Build readiness remains false. No host service, package installation, review allowlist, or push is part of this candidate. Full packaged backend operation and target Omarchy validation remain separate gates.
