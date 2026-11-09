# MCP backpressure repair: independent round-four review

30 October 2026. Reviewed only `/home/butahlecoq/work/archy/.worktrees/mcp-backpressure-repair-20261029` at `8e36e11d1399dee37dc2b85a8e2225b473571602`. **Scoped verdict: the backpressure repair and successful-result oracle pass on this frozen base; changes requested against the stated no-weakened-test requirement because the old mixed-burst coverage was removed.** The observed runs establish two real `StdioWriter::poll_write` `Pending`/positive-write cycles and successful protocol replies in both children. They do not establish a kernel `EAGAIN`, an atomic drain-to-resume causal proof, or validation on the published integration head.

## Integrity and source review

I read `repository/AGENTS.md`, the handoff, round-three Sol review, round-four request, `parse_errors.py`, the complete test-only example, shipping `main.rs`, `framing.rs`, and `bounds.rs`. `sha256sum -c docs/plans/mcp-backpressure-repair-round4-freeze.sha256` returned OK for all seven entries, before and after validation. The five frozen source hashes are:

| File | SHA-256 |
| --- | --- |
| `crates/archy-mcp/tests/parse_errors.py` | `6aaf90230572266ed88cb6d3c2b176c5bc1da25d81f1181cb5d85dea27193469` |
| `crates/archy-mcp/examples/stdout_backpressure_probe.rs` | `5bdf8993ad67abcf7e256829ab53a6a796de4149129a34e4b97c32cb59a148b1` |
| `crates/archy-mcp/src/main.rs` | `dc36c3c2fe0b08e90ad7d0ffd48f344720eadf576240a68b09e6c6d07f26a45b` |
| `crates/archy-mcp/src/framing.rs` | `c14ce7035f913929532afb2e8c88732093b834a450b9c9753b0128f8703631ba` |
| `crates/archy-mcp/src/bounds.rs` | `55a330dd9f04561e9d870a5a9fdc9b9f550c5dd7ec724e4fba729144de34443a` |

`ObservedWriter` calls the inner writer's `poll_write(cx, buf)` with the original buffer and returns its unchanged result. It reports `P` only when that call returns `Poll::Pending` after a prior positive write; it reports `R` only on a later `Ready(Ok(n))` with `n > 0`. Flush and shutdown delegate. Because it sits directly inside `FrameWriter`, waiting for that writer's mutex cannot itself produce `P`. Its four one-byte control events use a distinct inherited pipe; inode checks reject stdout/stderr aliasing. The parent feeds only actual child stdout bytes into `Reader`. The example and shipping main use the same server, `stdio_pipes`, framing, depth and response guards, and `AsyncRwTransport` expression. The example inserts this adapter and control setup; startup diagnostics also differ. `check_probe_wiring()` catches listed source-expression drift, but substring equality cannot prove semantic equivalence or catch an unlisted change. Side-by-side source inspection supports the scoped comparison.

The parent receives `P`, checks for a queued `R`, checks again immediately before its first stdout read, requires a positive drain, and then receives `R`; it repeats this twice. The second check runs only until the first read, so the fix avoids rejecting `R` after drainage has begun. The check and first `os.read` are separate syscalls: an `R` arriving between them is a narrow unresolved ordering window. The result is observed transport backpressure and resumed positive writes, not an atomic proof that drainage caused each resume. Tokio can return `Pending` on readiness without issuing a failing `write` syscall, so no `EAGAIN` claim follows.

## Protocol assertions and pacing

The probe sends 16 `tools/list` requests and 16 malformed lines together, observes the two transport cycles, then sends 48 `get_project` calls one at a time through that same child. The shipping child receives the same 80 requests plus one priming request, with the same staging. Each loop requires complete frames, unique expected IDs, no marker echo, exactly 16 replies equal to the complete `-32700` parse-error object, and success for every valid ID. For `tools/list`, `get_project` must be advertised with input and output schema objects. For `get_project`, `isError` must be false, `structuredContent.ok` true, `data.project_id` equal `courtyard-demo`, and the text content must decode to that same structured value. JSON-RPC `error` is forbidden. The probe accounts for 80 batch frames; shipping accounts for 81 including priming. Child cleanup kills/reaps on failure and the exit handler reaps all registered children.

The staging respects the real `ArchyServer` four-permit `try_acquire` limit on this base. The published A15 integration retains four slots. A simultaneous 48-call burst can legitimately return `BACKEND_UNAVAILABLE`; serial calls let the new oracle require successful results. **Coverage finding:** the previous test sent all 48 `get_project` requests in the mixed burst and checked their framing/IDs even when they returned tool errors. This test no longer exercises that exact 48-call saturation shape. It has stronger success assertions and still tests concurrent production of 16 large list replies and 16 parse errors. Therefore a literal claim that *no* test dimension was weakened would be false. To satisfy that requirement, retain the old mixed-burst shape as a separate saturation case with explicit expected error semantics, while keeping the serial successful-result stage for the backpressure oracle.

## Independent validation

All commands below ran in this worktree with `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029` where Cargo or the live Python suites used a target. No source files were changed.

| Command or fault | Observed result |
| --- | --- |
| `sha256sum -c docs/plans/mcp-backpressure-repair-round4-freeze.sha256` | Exit 0; seven OK, repeated after tests |
| `python3 -B crates/archy-mcp/tests/parse_errors.py` | Exit 0; `Pending/resumed writes: 2`, 4,096-byte pipe, final PASS |
| 16 invocations of that command via `ThreadPoolExecutor(max_workers=8)`, checking both output markers and exit status | 16/16 exit 0 with both markers |
| `python3 -B scripts/check-schemas.py` | Exit 0; live MCP suites, nine schema pairs, 16 source schemas, 50 serde/domain cases, six demo tests |
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy --locked -p archy-mcp --example stdout_backpressure_probe -- -D warnings` | Exit 0 |
| `git diff --check` | Exit 0 |
| In-memory `Reader.frame` substitution of a fabricated JSON-RPC error at ID 1000, first encounter (probe) | Rejected with `AssertionError` showing fabricated error; four children reaped |
| Same substitution at ID 1000, second encounter (shipping) | Rejected with `AssertionError` showing fabricated error; five children reaped |
| In-memory `SIGSTOP` immediately before batch sender startup | Rejected at `P` timeout; four children reaped |
| In-memory replacement of the batch's `drain_available(...)` call with no operation | Rejected at `R` timeout; four children reaped |

The fault injections compiled modified `parse_errors.py` text in memory and executed it with `__file__` set to the frozen path; none wrote repository files or fabricated child stdout. An extra exit-handler print checked that every registered process had been reaped. The first combined injection runner itself ended nonzero because I incorrectly asserted four children for the shipping case; its captured shipping result showed the correct five reaped, and the subsequent two fault injections completed with their expected assertions. This runner mistake did not change the tested source or the observed rejection.

## Integration boundary

Published integration is `15446eeaf316846936a093377041b8c1d2874092`, with reviewed A15 MCP mutation work and A16 ingress/journal. A15 changes `crates/archy-mcp/src/server.rs`, `src/tools.rs`, `tests/mutations.py`, and `tests/stdio_contract.py`; A16 also changes workspace manifests and `Cargo.lock`. This repair changes only `parse_errors.py` and adds the example, so there is no direct file overlap with A15/A16. The integration's four-slot server limit still explains the serial successful-result stage. The frozen base validations do not validate the merged tree: carry this repair while retaining both A15 and A16, then rerun the focused and schema gates on that integrated result. I made no commit, push, install, child-agent call, or edit to another worktree.
