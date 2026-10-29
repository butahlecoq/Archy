# A15 MCP first read slice: independent round-six review

29 October 2026. **Scoped verdict: no new issue found in the round-five depth-framing fix.** This reviews the frozen first read-only MCP slice, not full A15/A16 acceptance or Omarchy execution.

## Identity and method

Worktree `/home/butahlecoq/work/archy/.worktrees/a15-mcp`, base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`. I read `mcp-first-slice-handoff.md`, the round-five review, and the refreshed freeze. `sha256sum -c docs/plans/mcp-first-slice-freeze.sha256` passed all 31 entries. Manifest SHA-256: `eb22a4240529c03a7d37121d39ec23eabb984ee7ab898c7ba76329f8c7e55106`. All Cargo commands used `RUSTUP_AUTO_INSTALL=0`, `CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=2`, `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-mcp-round6-review`, and `PYTHONDONTWRITEBYTECODE=1`. That target was fresh for this review; `cargo build --locked -p archy-mcp -p archy-cli` completed before live probing. No pre-existing binary was used.

## Depth ordering and termination

The previous defect was loss of a complete valid request before a depth-65 frame in one stdin write. The current `DepthGuard` stops at each newline and retains no more than one 8192-byte chunk. Its next read scans the next line, including quote and escape state across chunks. `JsonLineFilter` then forwards complete valid lines unchanged. Independent real-binary probes sent 1, 3, 32, and 64 complete `tools/list` requests followed by a depth-65 frame in one write. All preceding IDs received complete result frames; the 64-request batch returned 1,651,574 stdout bytes while stdout draining was delayed. Three-request variants split before the first newline and immediately before the 65th opening bracket; both returned all three results. A temporary-project inventory and source bytes were unchanged. The probe was `/tmp/archy-a15-round6-probe.py`.

The checked-in live parse suite also passed its single-write and fragmented three-request regressions, concurrent SDK/parse-error responses under stdout backpressure, and EOF cases. A standalone depth-64 array reached the SDK and received `-32600`; a standalone depth-65 array emitted no frame and terminated the stream. An incomplete trailing line at EOF emitted no frame. When these were the only inputs before initialization, the process exited 1 with `MCP transport startup failed`; after accepted requests, the batch-depth probes exited 0. Thus the depth failure closes input rather than producing a JSON-RPC depth error. This behavior is consistent with the documented depth policy, but process exit status depends on whether transport startup completed.

## Bounds, contracts, and authority

The live parse suite passed malformed JSON, invalid UTF-8, lone surrogate, BOM/CRLF, fragment, oversize recovery, and `-32700` with explicit `id:null`. An independent exact-boundary probe sent a syntactically invalid line of exactly 1,048,576 bytes and received `-32700`/`id:null`, then sent a 16 MiB over-limit line and received no response for it; subsequent valid IDs 1 and 2 were answered. Observed peak `/proc` VmRSS during that process was 14,848 KiB. This is one workload observation, not a general memory proof. The source caps the line vector at 1 MiB, keeps one 8192-byte depth chunk and one 8192-byte filter chunk, and caps encoded response buffering at 1 MiB; parsing a permitted frame and SDK work can allocate further bounded data. The 64-request test establishes complete frames under the exercised pipe backpressure, not arbitrary client stalls.

`stdio_contract.py` passed modern and legacy stdio, exact equality of all nine advertised input/output schema pairs with `schemas/mcp`, three verified reads, six typed `BACKEND_UNAVAILABLE` prerequisites, bounded arguments and Unicode, paging/cursor conflicts, history corruption and symlink refusal, pinned-source replacement, historical revision-zero refusal, and unchanged inventories. Source inspection confirms `main.rs` opens one `BoundProjectRead` from `--project`; `server.rs` checks that project ID, uses four read permits, and routes the six unfinished tools to typed prerequisites. `BoundProjectRead` pins the source and checks root/source identity before snapshots. This supports fixed-root, passive authority for the exercised paths. It does not test simultaneous in-place source edits at every read boundary or a real Omarchy client/backend.

The pinned SDK still emits `-32600` without an `id` member for a parseable invalid request shape, such as `{"jsonrpc":"2.0","method":1}`. The adapter's explicit-null correction applies to `-32700` syntax errors only; this is the previously documented SDK interoperability limit, not a round-five regression. The previous round-five desktop-reader lock failure came from this worktree's old base, before upstream `f4aed22`; I did not attribute it to framing or rerun that unrelated flaky test.

## Commands and outcome

- `cargo test --locked -p archy-mcp --test bounds --test framing`: 9 passed, 0 failed.
- `cargo build --locked -p archy-mcp -p archy-cli`: passed in the fresh target.
- `python3 crates/archy-mcp/tests/parse_errors.py`: passed.
- `python3 crates/archy-mcp/tests/stdio_contract.py`: passed.
- `python3 scripts/check-schemas.py`: passed; compared nine MCP pairs and reran both live stdio suites. Full output: `/tmp/archy-a15-round6-schemas.log`.
- Independent subprocess probes above: all assertions passed.

Only this report was added in the worktree. No source edits, commit, push, or installation were made.
