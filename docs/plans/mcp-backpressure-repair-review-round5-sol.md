# MCP backpressure repair: independent round-five review

30 October 2026. Reviewed the frozen repair worktree at `8e36e11d1399dee37dc2b85a8e2225b473571602`. **Verdict: the repair passes on its frozen base and restores the original mixed-burst coverage, but is blocked for integration/publication on `15446eeaf316846936a093377041b8c1d2874092`.** Its saturation oracle encodes the old server's exact overload response and rejects the different, legitimate A15 response. The failure must be repaired and validated on the combined head before approval. No source was edited in this review.

## Integrity and source evidence

`sha256sum -c docs/plans/mcp-backpressure-repair-round5-freeze.sha256` passed for all eight entries before and after validation:

| File | SHA-256 |
| --- | --- |
| `crates/archy-mcp/tests/parse_errors.py` | `8d879ed39ec5ba04a118ede3585d1123320201ebc2f2f2e9a073f7b19decfa0e` |
| `crates/archy-mcp/examples/stdout_backpressure_probe.rs` | `5bdf8993ad67abcf7e256829ab53a6a796de4149129a34e4b97c32cb59a148b1` |
| `crates/archy-mcp/src/main.rs` | `dc36c3c2fe0b08e90ad7d0ffd48f344720eadf576240a68b09e6c6d07f26a45b` |
| `crates/archy-mcp/src/framing.rs` | `c14ce7035f913929532afb2e8c88732093b834a450b9c9753b0128f8703631ba` |
| `crates/archy-mcp/src/bounds.rs` | `55a330dd9f04561e9d870a5a9fdc9b9f550c5dd7ec724e4fba729144de34443a` |
| `crates/archy-mcp/src/server.rs` | `1f53d531d615afd3130317478a48761df6c7eb5209d0db38686252bbb9d3bb16` |
| `docs/plans/mcp-backpressure-repair-handoff.md` | `983cac7fd9d6b94b7d3696fa315a699f0e7cd1e1af3dd4ed50a688e082d4d8e0` |
| `docs/plans/mcp-backpressure-repair-round5-review-request.md` | `513890efa53f18700c4e9ac22cf38fcfbe7ead94e4235ee649101b6d6ce47915` |

I read `AGENTS.md`, the handoff, round-four review, round-five request, the complete Python test, example, shipping main, framing and bounds, and both server versions. The test-only `ObservedWriter` delegates `poll_write` with the original buffer and returns its original result; it delegates flush and shutdown. Its `P` follows a positive write and an actual inner `Poll::Pending`; `R` follows a later positive inner write. It is directly inside `FrameWriter`, so a frame-writer mutex wait cannot itself generate `P`. The separate inherited control pipe is checked against stdout/stderr pipe inodes. The parent puts only actual child stdout into its frame reader. Shipping and example mains use the same project opening, `stdio_pipes`, `FrameWriter`, `JsonLineFilter`, `DepthGuard`, `ResponseGuard`, transport and server expressions; the example adds the adapter and control setup and differs in startup diagnostics. The source-expression check is a drift tripwire, not a semantic equivalence proof.

## Coverage and timing

The separate shipping-binary burst primes the process, then a writer thread submits 16 `tools/list`, 48 `get_project`, and 16 malformed lines in one payload. The parent drains 80 complete frames and requires 64 distinct expected valid IDs, 16 exact `-32700` parse-error objects, no marker echo, clean exit, empty stderr, and no buffered or later extra output. All 16 lists must advertise `get_project` with input/output schemas. Calls must have a complete success result, including project identity and matching text/structured content, or pass `saturated_call_reply`. That helper rejects JSON-RPC errors and checks `isError`, exact structured error fields, and matching text content. A fabricated arbitrary tool error is rejected. The maintained serial stage separately requires successful results for all valid requests through the shipping binary and the observer, so the round-four loss of the 48-call mixed shape is repaired on this base. Final fixture checks require unchanged `project.yaml` and no other files.

The two observed `P`/`R` cycles and positive stdout drains establish transport-level `StdioWriter::poll_write` Pending/resume behavior in the observed runs. The control-pipe check and first stdout read are separate syscalls, leaving a narrow ordering window; the observation is not an atomic causal proof. Tokio `Pending` does not prove a kernel `EAGAIN`. Saturation is scheduler dependent: zero bounded replies is valid when all 48 calls satisfy the complete success oracle.

## Validation on frozen base

All live checks used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029`.

| Check | Observed result |
| --- | --- |
| `python3 -B crates/archy-mcp/tests/parse_errors.py` | Exit 0; two Pending/resume cycles on a 4,096-byte pipe; mixed burst 48 successes, zero saturation; final PASS. |
| 16 script invocations using eight workers | 16/16 exit 0, each with the two-cycle and final PASS markers. |
| `python3 -B scripts/check-schemas.py` | Exit 0; live MCP suites, nine schema pairs, 16 source schemas, 50 serde/domain cases, six demo tests. |
| `cargo fmt --all -- --check`; `cargo clippy --locked -p archy-mcp --example stdout_backpressure_probe -- -D warnings`; `git diff --check` | Exit 0 for each. |
| In-memory fabricated JSON-RPC error at ID 1000 in observer, shipping serial, and shipping mixed stages | Rejected with `AssertionError`; respectively 4, 5, and 6 children reaped. |
| In-memory arbitrary `isError` tool result in mixed stage | Rejected with `AssertionError`; 6 children reaped. |
| In-memory SIGSTOP before observer sender | Rejected at `P` timeout; 4 children reaped. |
| In-memory withheld observer stdout drain | Rejected at `R` timeout; 4 children reaped. |

The fault probes compiled modified Python source in memory and did not edit the frozen files. An initial SIGSTOP probe runner had a missing `signal` binding in its execution namespace, so it did not test the intended fault; the corrected rerun produced the recorded `P` timeout and reaped all four children.

## Integration blocker

The frozen base `server.rs` has four `reads` permits and emits exact structured tool error `BACKEND_UNAVAILABLE`, `field=read`, `message=Concurrent read limit reached`, `retriable=true` when `try_acquire` fails. The Git object at `15446ee` has four `slots` permits and emits `BACKEND_UNAVAILABLE`, `field=tool`, `message=Concurrent tool limit reached`, `retriable=true` from the same overload branch. The user also verified the current combined server source at `/tmp/archy-ci-repair/crates/archy-mcp/src/server.rs:53-59` and found no old `Concurrent read limit reached` text. Thus both are real bounded-overload contracts for their respective server versions; neither licenses arbitrary errors.

The reported combined-head stress at `/tmp/archy-ci-repair` failed 3/8 runs (`mcp-round5-15446-stress-{2,5,7}.log`). I read those logs: each failing assertion is `saturated_call_reply` comparing against the old `field=read`/`Concurrent read limit reached` structure; actual replies carried `field=tool`/`Concurrent tool limit reached`, with matching text content, `isError=true`, and IDs 1054, 1035, and 1054 respectively. This is a demonstrated integration failure, even though the frozen-base 16-way stress passed. Update the oracle to the exact published A15 overload response when integrating, retain strict rejection of all other tool and JSON-RPC errors, and rerun focused, stress, and schema gates on the combined head. The unmodified frozen repair is not publishable.

No commit, push, install, child-agent call, source edit, or other worktree edit was made; this report is the only review output.
