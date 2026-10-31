# A09 current-head integration handoff for separate Sol review

Date: 2026-10-29. New isolated detached worktree: `/home/butahlecoq/work/archy/.worktrees/a09-current-head-integration-20261029`. Exact HEAD: `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1`; source A09 HEAD: `f1e2477047dbafbacfba3229daddab0ca58ebe02`. No commit, push, host install, child agent, or edit to any other worktree. Source history and both Sol reviews are copied beside this handoff. This handoff requests a separate Sol integration review; it does not represent that review.

## Inputs, method, and reproducible freezes

The 30-path source manifest `a09-end-to-end-integration-freeze.sha256` has SHA-256 `def291c138778c8a41da75ba10f3c613a17297d3c857968c429a484acc3e40c7`. `sha256sum -c` passed in the source worktree and in this current-head worktree: **all 30 source paths remain byte-identical** after integration. The product patch produced by `git diff --binary` from the source worktree at f1e has SHA-256 `389e1a8ba61c7d41536c8c3bee2fdeeec751b2512c0674ac617bfc6254c169a8`; this was independently reproduced before application. It was applied with `git apply --3way` onto exact ff7. No conflict markers or manual product edit were needed.

The 21-path current-head product patch is `a09-current-head-integration.patch`, SHA-256 `42a9f7b86a1bc0862d9c0a7fa43c6973bacad7b21ce96f1e208fa310b37c98f2`, generated with `git diff --cached --binary` relative to ff7. `git apply --reverse --check` passed. Its current-head source freeze is `a09-current-head-integration-freeze.sha256`, SHA-256 `5db3783b944f1754783695850bea091e8027c4f525a6ebc2bb6a68df52e8b147`; all 21 entries passed `sha256sum -c`. The product patch is staged for inspection but uncommitted; the handoff and freezes are untracked documents. Re-run the checks before review if another actor edits this tree.

## Explicit overlap map

| Boundary | Current head | Resolution in this worktree |
| --- | --- | --- |
| Published A15 compare and MCP | ff7 carries the published compare follow-on in `compare.rs`, CLI export, MCP `Cargo.toml`, `server.rs`, `tools.rs`, `compare_stdio.py`, `stdio_contract.py`, schema, schema script, and MCP README | All A09 source freeze paths in this set already equal ff7 and remain byte-identical. CLI `lib.rs` adds A09 accepted-worker module and exports alongside `compare` and `render_verified_comparison`. |
| Workspace dependency lock | ff7 `Cargo.lock` already includes the MCP/report change | A09 adds `archy-adapters` under CLI and the `archy-service-gate` package; the published MCP dependency remains. Resulting lockfile equals the A09 frozen file. |
| Escaped pipeholder fixture | ff7 changes `crates/archy-adapters/src/process.rs` after f1e | Untouched and byte-identical to ff7 (SHA-256 `a519321c480723e09eb2e417c5ac10c7d231373c5159ff0407653c31d66197ee`). Its library test passed in the focused run. This is distinct from the intermittent MCP parse-error backpressure test. |
| A21 package notice graph | Inherited by f1e and ff7 | A09 extends locked normal graph roots to CLI plus gate, requires the fixed static gate ELF row, and retains notice coverage and prewrite source capture. Packaging tests passed. |
| CLI pinned project read and accepted history | A09 changes `project_read.rs`, `accepted_worker.rs`, CLI `lib.rs` | The accepted worker wrapper constructs an owned request while verifying a locked accepted run and pinned source; compare exports remain. Its four binding cases passed. |
| Future A02 v2 and A15 mutation | Neither is in ff7's f1e-to-head source delta | Still a separate reconciliation. A02 v1 verification does not admit v2 evidence. Future CLI reader/export and MCP/schema changes need another overlap review. |

`git diff ff7 -- crates/archy-adapters/src/process.rs crates/archy-cli/src/compare.rs crates/archy-mcp/src/server.rs crates/archy-mcp/src/tools.rs schemas/mcp/compare_variants.schema.json` was empty. The full source freeze check covers the other shared compare/MCP paths as well.

## Exercised boundary and result

Cargo commands use `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-current-head-ff7-20261029` (unique SSD target), `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`, `--locked --offline`.

- `cargo test -p archy-adapters --test artifacts -p archy-adapters --lib -p archy-service-gate --bin archy-service-gate -p archy-cli --test accepted_worker`: exit 0. Adapter library 26 passed, artifacts 9 passed, accepted worker 4 passed, gate 8 passed. The artifact fixture invokes the actual IFC and Blender Python entry points with native backend generation stubbed; it exercises IFC `output/ifc`, flat Blender error output, success 0 and handled error 20, bounded capture, forged/missing response refusal, extra root file refusal, and symlink refusal. Gate unit checks require exit/status agreement and reject crash/signal classes. This is a fixture, not an installed controller-to-worker run.
- `python3 -m unittest -q tests.packaging.test_package tests.packaging.test_notices`: exit 0, 32 tests. Includes static gate ELF admission, notice coverage and prewrite staging checks. It is not a clean package build.
- `cargo fmt -p archy-adapters -p archy-cli -p archy-service-gate -p archy-mcp -- --check`: exit 0.
- `git diff --cached --check`, `git apply --reverse --check` for the current-head patch, and JSON parsing of compare schema: exit 0.
- `cargo clippy --locked --offline -p archy-adapters -p archy-cli -p archy-service-gate -p archy-mcp --all-targets -- -D warnings`: exit 0.
- `cargo build --locked --offline -p archy-cli -p archy-mcp`: exit 0 in the unique target. `python3 crates/archy-mcp/tests/compare_stdio.py`: exit 0; verified comparisons, CLI parity, paging, tamper refusal, fixed source, and no writes passed. `python3 crates/archy-mcp/tests/stdio_contract.py`: exit 0; bounded schemas, nine live schema/contract paths, paging, stale cursor, pinned-source replacement and no-write cases passed. Neither probe is the intermittent parse-error backpressure test.

The known intermittent MCP parse-error pipe-fill/backpressure test was **not rerun**. The source handoff records an initial `scripts/check-schemas.py` failure at 59,753 bytes versus its 61,440 byte threshold and a later pass. That failure is under separate repair. This integration has no claim that the flaky test or full schema sweep is stable.

## Review focus and release limits

Review the combined `Cargo.lock` and CLI exports, preserved ff7 compare/MCP and process fixture, A21 notice graph/prewrite admission, accepted-run request lifetime, and gate exit-20 response through verified teardown and bounded capture. Verify that the 30-path source and 21-path current freezes still pass. The current patch must be reviewed separately by Sol before integration elsewhere.

Public `BlenderRequest::new`, `IfcRequest::new`, `ServiceRequest`, and `ServiceSession::start` remain syntactic/fixed-policy entry points for direct callers; they do **not** independently verify accepted provenance. Only the CLI accepted-run wrapper checks stored provenance under its cooperating lock. A self-consistent revision chain does not authenticate human approval. Capture returns typed bytes and diagnostics after teardown; it is not immutable artifact publication or A13 independent IFC verification.

On this host `/usr/lib/archy/archy-service-gate`, `/usr/lib/archy/runtime/inventory.json`, `/usr/bin/blender`, and `/usr/lib/archy/runtime/ifc-venv/bin/python` are absent. No root-owned installed runtime, reviewed complete transitive dependency/font inventory, positive systemd/bwrap controller-to-real-worker execution, clean Arch build, native Blender or IfcOpenShell generation, or Omarchy acceptance was exercised. The gate hashes selected executables and entry scripts, not every transitive dependency. This work is ready for **separate integration review only**, not production readiness.
