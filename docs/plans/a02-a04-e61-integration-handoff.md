# A02/A04 on published e61c977: integration handoff

30 October 2026. Detached isolated worktree: `/home/butahlecoq/work/archy/.worktrees/a02-a04-e61-integration-20261030`, base `e61c977d3e8b504dfafd2e81689e97f62c934e76`. Input is the 106-path binary patch at `.tools/patches/a02-a04-published-1e2313-20261030.patch`, SHA-256 `5b4dea7445a0a55c611497cb0ecb9527070daa4988f44263434ff1766039d3d0`. The independent [source integration review](/home/butahlecoq/work/archy/.tools/patches/a02-a04-published-integration-review-sol.md) passed the original `1e2313f` candidate only; it does not review this tree.

## Reconciliation

`git apply --3way --index` gave two conflicts: `crates/archy-mcp/tests/mutations.py` and `scripts/check-schemas.py`. The first retains the published timeout helper's 15-second default and explicit 60-second allowance for the 100-variant retry/replay; the second retains the source candidate's 360-second cap for that mutation script, beside the current desktop schema checks. No product transport timeout changed. Strict Clippy exposed four redundant references in the imported `generate.rs` and `evaluate.rs`; these were removed without changing control flow. The original frozen 105-path manifest matches 98 integrated paths byte for byte. Seven expected differences are `README.md`, `crates/archy-cli/src/lib.rs`, `crates/archy-cli/src/project_read.rs`, `crates/archy-cli/src/generate.rs`, `crates/archy-cli/src/evaluate.rs`, and the two timeout test files.

`README.md` keeps the published A18 contributor quickstart link and demo boundary while updating the input contract to v2. A20's acceptance matrix and review files remain at published bytes; their historical source/evidence dates are not new acceptance evidence. The A22 artifact opening, plugin, installer, desktop docs and tests remain at published bytes. Shared CLI files retain `desktop_open` and verified artifact access beside A02/A04 canonical and historical read handling. The A09 service gate, A15 locked mutation, A16 authority, and MCP backpressure sources were not replaced.

## Exact-tree checks

All commands ran against this worktree on ext4 with short `TMPDIR=/home/butahlecoq/work/archy/.t`. Rust used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-a04-e61-20261030`.

- `cargo test --locked --offline --workspace -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable`: exit 0 on the final source bytes; full output at `.tools/targets/a02-a04-e61-20261030/workspace-final.log` (workspace root). The named 64-plus-run history stress case was filtered and is not claimed as passing.
- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --locked --offline --workspace --all-targets -- -D warnings`: first run failed on four `needless_borrow` findings in imported code; after the four reference edits, exit 0 (`clippy-final.log`).
- `python3 -m unittest tests/omarchy/integration/test_install.py tests/omarchy/integration/test_plugin_contract.py -v`: exit 0, 19 tests.
- `node --test tests/omarchy/integration/*.test.js`: exit 0, 17 tests.
- `python3 -m unittest discover -s tests/packaging -p 'test_*.py'`: exit 0, 32 tests.
- `python3 -B scripts/check-schemas.py`: exit 0 on the final source bytes; full log at `.tools/targets/a02-a04-e61-20261030/schema-final.log` (workspace root). It covers nine live MCP schema pairs, current/historical reads, framed transport and backpressure, live mutation/replay, desktop schemas, 26 CLI negative fixtures, and six demo tests.

The source review's 64-plus-run stress omission remains. Passing local and WSL tests do not establish installed Omarchy operation, backend artifact publication, human approval or regulatory compliance. This candidate needs its own independent source integration review. No commit, push, or final approval is claimed.
