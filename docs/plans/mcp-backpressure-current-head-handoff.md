# MCP backpressure repair on current A15 head — independent Sol handoff

30 October 2026. Isolated detached worktree: `/home/butahlecoq/work/archy/.worktrees/mcp-backpressure-current-head-20261030-1`; base and unchanged HEAD: `15446eeaf316846936a093377041b8c1d2874092`. All build and live test commands use the unique `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-current-head-20261030-1`. No commit, push, install, child agent, or other worktree edit was made.

## Cause and source

`sha256sum -c` passed for all eight entries in the original `mcp-backpressure-repair-round5-freeze.sha256` before copying. The copied `parse_errors.py` and `stdout_backpressure_probe.rs` had exactly the frozen round-five hashes `8d879ed39ec5ba04a118ede3585d1123320201ebc2f2f2e9a073f7b19decfa0e` and `5bdf8993ad67abcf7e256829ab53a6a796de4149129a34e4b97c32cb59a148b1`. The original worktree was not edited.

The three captured integrated-head failures (`.tools/mcp-round5-15446-stress-{2,5,7}.log`) show real `BACKEND_UNAVAILABLE` tool results with `field=tool`, `message=Concurrent tool limit reached`, and `retriable=true`. The current A15 server at `crates/archy-mcp/src/server.rs:53-59` emits exactly that result when its four `slots` permits are busy. The frozen test expected the older server's `field=read` and `Concurrent read limit reached`. The sole semantic edit changes those two exact expected strings in `saturated_call_reply`. It still rejects arbitrary tool errors and JSON-RPC errors and checks that text content decodes to the identical structured error. The complete successful-call oracle remains unchanged.

The patch is `docs/plans/mcp-backpressure-current-head.patch`; `git apply --reverse --check` passes on this worktree. The source freeze is `docs/plans/mcp-backpressure-current-head-freeze.sha256`.

## Live validation

| Gate | Exact result |
| --- | --- |
| `cargo build --locked -p archy-mcp` | Exit 0. |
| Focused `python3 -B crates/archy-mcp/tests/parse_errors.py` | Exit 0; two observed Pending/resume cycles with 4,096-byte stdout pipe; 48 successful and zero saturation calls in mixed burst; final nonmutation PASS. |
| 16 focused-script invocations with `ThreadPoolExecutor(max_workers=8)` | 16/16 exit 0; each had two-cycle, mixed-burst, and final PASS markers. Five runs observed saturation; counts by invocation: `1,0,0,0,1,0,0,0,0,0,2,0,3,7,0,0`. |
| In-memory fabricated JSON-RPC error at ID 1000 in observer, shipping serial, shipping mixed stage | Each rejected; respectively 4, 5, and 6 children reaped. |
| In-memory SIGSTOP before observer sender; in-memory withheld stdout drain | Rejected at `P` and `R` event timeouts respectively; four children reaped in each case. |
| Direct saturation oracle probes | Exact current A15 reply accepted; old `field=read`, old read-limit message, and unrelated tool error rejected. |
| Full `python3 -B scripts/check-schemas.py` | Exit 0; nine MCP input/output schema pairs, 16 source schemas, live MCP suites including the backpressure test (46 successes/two exact saturation replies), 26 CLI negative fixtures, and six demo tests (`Ran 6 tests`, `OK`). |
| `cargo fmt --all -- --check`; Python AST parse; `git diff --check` | Exit 0 each. |
| `cargo clippy --locked -p archy-mcp --example stdout_backpressure_probe -- -D warnings` | Exit 0. |

Focused, stress, and negative logs are `.tools/mcp-current-head-{focused,stress,negative}.log`; the reproducible in-memory probe runner is `.tools/mcp-current-head-negative-probes.py`. The schema log is `.tools/mcp-current-head-schema.log`. They live outside this worktree. The runner mutates Python source only in memory; it never modifies the frozen test or fabricates child stdout.

## Review scope

Independently compare the two-field expectation to the current server and captured actual replies. Check the patch and frozen source hashes. Confirm both original shapes remain: the observer's 64 valid IDs and 16 exact parse errors with two real `StdioWriter::poll_write` Pending/resume cycles, and the separate shipping-binary mixed burst of 16 lists, 48 `get_project` calls, and 16 malformed frames. The serial shipping stage must still require all valid successes. Verify unique IDs, full success payloads, only exact A15 saturation errors, no extra frames or marker echoes, clean exit, empty stderr, and unchanged fixture. Re-run focused, 16-way stress, schema, format, clippy, and negative faults independently.

The control-pipe check and first stdout read remain separate syscalls; the event ordering is not an atomic proof. Tokio `Pending` does not prove kernel `EAGAIN`. Scheduler-dependent zero saturation in one run is valid only when all 48 calls pass the complete success oracle.
