# A09 worker integration on published A15 mutation: independent Sol review handoff

Date: 2026-10-30. Isolated detached worktree: `/home/butahlecoq/work/archy/.worktrees/a09-a15-160787-integration-20261030`. Exact base HEAD: `160787d1f6c65a85d0e022a111d56ee7b0eb2b86` (`feat: enable scoped MCP generation and evaluation`). This is an uncommitted integration for separate Sol review. No other worktree was edited; no commit, push, host install, or child agent was used.

## Inputs and frozen result

Read `AGENTS.md`, the A09 current-head handoff, independent Sol review, freeze, and patch. In the prior A09 worktree, its 30-path source freeze (`def291c138778c8a41da75ba10f3c613a17297d3c857968c429a484acc3e40c7`) passed `sha256sum -c`; its ff7 product freeze (`5db3783b944f1754783695850bea091e8027c4f525a6ebc2bb6a68df52e8b147`) passed all 21 entries. Its patch SHA-256 was independently checked as `42a9f7b86a1bc0862d9c0a7fa43c6973bacad7b21ce96f1e208fa310b37c98f2`. The source handoff and Sol review remain in `.worktrees/a09-current-head-integration-20261029/docs/plans/`.

Applied that exact patch with `git apply --3way` to exact 160787. Only `crates/archy-cli/src/project_read.rs` conflicted. Resolution retains A15's `snapshot`, `check_binding`, and `transact` code in place and adds A09's `with_verified_locked_run` as a separate method. The final diff from 160787 for that file is only this method. CLI `lib.rs` retains published compare and A15 exports and adds the Linux accepted-worker exports. `Cargo.lock` retains published MCP dependencies and adds only `archy-adapters` to CLI and the `archy-service-gate` package. The other 19 A09 frozen product paths remain byte-identical to the old 21-path freeze; `lib.rs` and `project_read.rs` differ for the expected A15 combination.

The current exact-head 21-path product patch is `a09-a15-160787-integration.patch`, SHA-256 `da5644ad8c9b490c176c9c8ffc1582e79cda14b27e6d9bfb7f0d2f785fec34cb`. It was generated from `git diff --cached --binary` against 160787; `git apply --reverse --check` and `git diff --cached --check` passed. The current 21-path file freeze is `a09-a15-160787-integration-freeze.sha256`, SHA-256 `bdb3146e6dde1ff02c3583d7a3e688dd18a757c3e022f0890cda28d919135f91`; all 21 entries passed `sha256sum -c`. Both files and this handoff are untracked; the product diff is staged, with no commit.

## Provenance and behavior to review

- The published escaped pipeholder fixture in `crates/archy-adapters/src/process.rs` is byte-identical to the inherited head (`a519321c480723e09eb2e417c5ac10c7d231373c5159ff0407653c31d66197ee`); its test passed. This fixture is separate from the intermittent MCP parse-error pipe test.
- `crates/archy-cli/src/generate.rs`, `evaluate.rs`, `crates/archy-core/src/storage.rs`, MCP `server.rs` and `tools.rs`, `mutations.py`, `stdio_contract.py`, the two mutation schemas, `scripts/check-schemas.py`, and the MCP README have no product diff from 160787. Thus A15's locked transaction, publication, live mutation dispatch, and compare path are inherited intact. The live mutation and comparison probes passed against the combined binaries.
- A09's accepted-worker wrapper takes the cooperating project lock, verifies the stored accepted run and pinned source, and builds an owned Blender/IFC request. The public adapter constructors and `ServiceSession::start` remain fixed-policy/syntactic entry points; they do **not** independently prove accepted provenance or human approval. Captured bytes are not immutable publication or A13 IFC verification.
- A09's gate requires worker exit and response agreement after bounded capture and verified teardown. The package notice graph includes the CLI and gate locked normal dependencies; the static gate ELF row and prewrite staging checks are retained. The local dynamic gate is only suitable for the negative prelaunch probe, not package admission or positive installed execution.

## Validation on this exact worktree

Rust commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-a15-160787-20261030`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`, `--locked --offline`. Python used `PYTHONDONTWRITEBYTECODE=1` and `-B` where applicable.

- Focused `cargo test -p archy-adapters --lib -p archy-adapters --test artifacts -p archy-service-gate --bin archy-service-gate -p archy-cli --test accepted_worker`: exit 0; 26 adapter library, 9 artifact, 8 gate, and 4 accepted-worker tests passed. The artifact tests invoke the actual IFC/Blender Python entry points with native backend generation stubbed.
- `cargo test -p archy-cli --test desktop_readers`: exit 0; seven A15 bound-reader tests passed.
- `python3 -B -m unittest -q tests.packaging.test_package tests.packaging.test_notices`: exit 0; 32 tests, including gate ELF admission, notice coverage, and prewrite staging. This is not a clean Arch package build.
- `cargo build -p archy-cli -p archy-mcp -p archy-service-gate`: exit 0.
- `mutations.py`: exit 0; live generate/evaluate/compare, historical read, replay, conflicts, scope, tamper, and replacement. `compare_stdio.py`: exit 0; CLI parity, paging, tamper refusal, fixed source, no writes. `stdio_contract.py`: exit 0; nine live schema/contract paths, bounds, paging, stale cursor, pinned source replacement, and no-write checks.
- `scripts/check-schemas.py`: exit 0; nine MCP schema pairs, 50 decision cases, 16 schema files, CLI serializers/fixtures, parse-error stdio, comparison, mutations, and six demo tests. The known intermittent parse-error backpressure test passed in this one run; this does not establish its stability. No pipe test repair was copied.
- `tests/security/service_gate_unavailable.py <unique-target>/debug/archy-service-gate`: exit 0; five negative prelaunch cases with bounded FD, unavailable response, collected unit, unchanged host sentinel.
- `cargo fmt --all -- --check`, JSON parsing of the three generate/evaluate/compare MCP schemas, source and product freezes, reverse patch check, and staged diff check: exit 0.
- `cargo clippy --locked --offline --workspace --all-targets -- -D warnings`: exit 0.

## Limits and requested review

Please independently review the exact 160787 patch and freeze, especially the CLI project-read merge, A15 locked transaction path, `Cargo.lock` MCP dependency retention, A09 accepted provenance scope, gate exit-20/error agreement, notice graph, and preserved process fixture. Re-run the freeze and patch checks before review if this worktree changes. This handoff asks for a separate Sol integration review; it does not represent that review.

Installed runtime and publication remain open. The fixed gate, inventory, Blender, and IFC Python runtime are absent on this host. No positive installed systemd/bwrap controller-to-real-worker run, native Blender/IfcOpenShell generation, complete transitive/font inventory, clean Arch package, root-owned runtime install, immutable artifact publication, A13 independent IFC verification, or Omarchy acceptance was exercised. A self-consistent revision chain does not authenticate human approval.
