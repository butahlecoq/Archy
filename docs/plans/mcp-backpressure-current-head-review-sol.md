# Current-head MCP backpressure: independent review

30 October 2026. Reviewed only `/home/butahlecoq/work/archy/.worktrees/mcp-backpressure-current-head-20261030-1` at `15446eeaf316846936a093377041b8c1d2874092`. **Verdict: pass for the frozen repair on the current A15 head.** The prior round-five integration blocker is resolved by accepting only A15's exact global tool-limit response. No source edits, commit, push, install, or child agents were used.

## Integrity and source review

Read `AGENTS.md`, the current-head handoff, review request, full patch, prior round-five Sol review, complete Python test and observer example, and the shipping main, framing, bounds, and server code. `sha256sum -c docs/plans/mcp-backpressure-current-head-freeze.sha256` passed for **all nine entries** before and after validation: test `3c5bfd3c01fb6c3b6dd8eda0f87f843218f49d290319a8c3ef40d63fcbdab846`, example `5bdf8993ad67abcf7e256829ab53a6a796de4149129a34e4b97c32cb59a148b1`, main `dc36c3c2fe0b08e90ad7d0ffd48f344720eadf576240a68b09e6c6d07f26a45b`, framing `c14ce7035f913929532afb2e8c88732093b834a450b9c9753b0128f8703631ba`, bounds `55a330dd9f04561e9d870a5a9fdc9b9f550c5dd7ec724e4fba729144de34443a`, server `d77dd8a5a82ee725026b3e97984a9bb8f275d1e26825170f2df7718b447028ad`, patch `5c853ae5e7303156160ffbaaa95339128e8846695918a4a944f1339f2c88bd52`, handoff `8f71165a7b07f337466965d835e8989b4ac7028517e36f358669ae78254148e0`, request `833f04f3e5bc22ee02d7385d7cd822ab0ada95f75e64dee7487e8c02d8a4b3b2`. `git apply --reverse --check docs/plans/mcp-backpressure-current-head.patch`, `git diff --check`, and unchanged HEAD passed.

The A15 `ArchyServer` has four shared `slots` permits. On `try_acquire` failure it returns a tool result with `isError=true` and `structuredContent={"ok":false,"data":null,"error":{"code":"BACKEND_UNAVAILABLE","field":"tool","message":"Concurrent tool limit reached","retriable":true}}`. `saturated_call_reply` requires that exact structure, no JSON-RPC `error`, and one text item decoding to the identical structure. The three captured old-head integration failures at `.tools/mcp-round5-15446-stress-{2,5,7}.log` carry that exact A15 result; the old oracle expected `field=read` and `Concurrent read limit reached`. The current patch changes those two expectation strings and retains strict error checks.

The observer delegates the original buffer and result to the inner `StdioWriter::poll_write`; its `P` follows an actual inner `Pending` after a positive write, and `R` follows a later positive inner write. It sits inside `FrameWriter`, so mutex waiting alone cannot emit `P`. Its separate inherited control pipe is inode-checked against stdout/stderr. The test's source-expression checks cover `stdio_pipes`, `FrameWriter`, `JsonLineFilter`, `DepthGuard`, `ResponseGuard`, transport line limit, and server wiring in shipping and example mains. That check is a drift tripwire, not a semantic equivalence proof.

The observer receives 16 `tools/list` plus 16 malformed frames before its two `P`/`R` cycles, then sends the remaining 48 `get_project` calls serially; it requires all 64 distinct valid IDs, 16 exact `-32700` parse errors, complete successful results, and no marker echo. A separate shipping-binary batch requires all 64 valid IDs to succeed and exactly 16 parse errors. The shipping mixed burst sends all 16 lists, 48 calls, and 16 malformed frames together; its 80 replies require unique expected IDs, exactly 16 exact parse errors, valid list schemas, and either complete successful calls or only the exact A15 saturation result. It requires 48 call outcomes in total, no marker echo, clean exit, empty stderr, and no buffered or later extra stdout. The fixture bytes and directory contents are checked at the end.

## Independent live evidence

All live commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-current-head-review-sol-20261030` from this worktree.

| Command or probe | Observed output and result |
| --- | --- |
| `cargo build --locked -p archy-mcp` then `python3 -B crates/archy-mcp/tests/parse_errors.py` | Both exit 0. `MCP stdout transport Pending/resumed writes: 2 pipe bytes: 4096`; mixed burst `success 48 bounded saturation 0`; final `nonmutation: PASS`. |
| 16 focused-script invocations via `ThreadPoolExecutor(max_workers=8)` | 16/16 exit 0 with two-cycle and final PASS markers. Mixed `(success,saturation)` by run: `(47,1), (48,0), (48,0), (48,0), (46,2), (48,0), (48,0), (48,0), (48,0), (48,0), (47,1), (48,0), (48,0), (48,0), (43,5), (48,0)`. Nine exact saturation replies total. |
| `python3 -B scripts/check-schemas.py` | Exit 0. Live MCP backpressure stage printed two cycles and mixed `success 48 bounded saturation 0`; nine MCP input/output schema pairs matched SDK descriptors; 16 source schemas valid; 50 serde/domain cases, 26 CLI negative fixtures, and six demo tests passed (`Ran 6 tests`, `OK`). |
| `cargo fmt --all -- --check`; `git diff --check`; Python AST parse | Exit 0 each. |
| `cargo clippy --locked -p archy-mcp --example stdout_backpressure_probe -- -D warnings` | Exit 0; `Finished dev profile` in 1m 13s. |
| In-memory JSON-RPC error at ID 1000 on observer, shipping serial, and shipping mixed stages | Each rejected with `AssertionError`; respectively 4, 5, and 6 children reaped. |
| In-memory SIGSTOP before observer sender; withheld observer stdout drain | Rejected at `P` and `R` event timeouts respectively; four children reaped in each case. |
| Direct saturation oracle probes | Exact A15 reply accepted; old `field=read`, old read-limit message, and unrelated tool error rejected. |
| Additional in-memory unrelated `isError` tool result at ID 1001's third occurrence, in shipping mixed stage | Rejected by exact saturation oracle; six children reaped. |

The stress driver used `python3 -B -` with `ThreadPoolExecutor(max_workers=8)` and 16 `subprocess.run(['python3','-B','crates/archy-mcp/tests/parse_errors.py'], ..., timeout=120)` calls. The standard negative runner command was `python3 -B /home/butahlecoq/work/archy/.tools/mcp-current-head-negative-probes.py`; I inspected its source before running it and separately injected the mixed-stage tool fault with an in-memory `python3 -B -` harness. Fault probes compiled changed Python source only in memory and did not edit frozen files or fabricate child stdout. Each stress process launched its own shipping and observer children.

## Limits

The control-pipe readiness check and first stdout drain read are separate syscalls, leaving a narrow ordering window. Tokio `Poll::Pending` establishes a transport-level wait, not a measured kernel `EAGAIN`. Scheduler-dependent zero saturation in an individual run is acceptable only because all 48 calls then pass the complete success oracle; five of my stress runs exercised the exact saturation branch. The observer and separate successful shipping batch do not independently assert no trailing frames; the mixed shipping burst does. The source-expression check cannot prove the example and shipping binary are semantically identical. These limits do not change the verdict for the requested repair.

No other worktree was modified. This report is the only review output in the target worktree.
