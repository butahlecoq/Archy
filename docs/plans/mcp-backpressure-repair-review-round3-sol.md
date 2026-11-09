# MCP backpressure repair: independent round-three review

30 October 2026. Reviewed only `/home/butahlecoq/work/archy/.worktrees/mcp-backpressure-repair-20261029` at `8e36e11d1399dee37dc2b85a8e2225b473571602`. **Verdict: changes requested.** The new probe gives convincing evidence of two real `StdioWriter::poll_write` `Pending`/positive-write cycles in the observed Linux runs, and the SIGSTOP counterexample is rejected. The batch still accepts arbitrary JSON-RPC errors for all 64 well-formed requests. Both the probe example and the shipping binary must require successful `tools/list` and `get_project` results while retaining the 16 exact parse-error assertions.

## Source integrity and scope

I read `AGENTS.md`, the handoff, round-one and round-two reviews, round-three request, the complete example, shipping `main.rs`, `framing.rs`, `parse_errors.py`, and the local locked `rmcp`/Tokio write paths. `sha256sum -c docs/plans/mcp-backpressure-repair-round3-freeze.sha256` returned OK for all seven entries. `git rev-parse HEAD` returned the base above; `git diff --check` exited 0. I made no source edits, commits, pushes, installs, changes to another worktree, or child-agent calls. All builds used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029`.

The example and shipping `main.rs` use the same `ArchyServer`, `stdio_pipes`, `FrameWriter`, `JsonLineFilter`, `DepthGuard`, `ResponseGuard`, and `AsyncRwTransport` expressions. The intentional difference is `FrameWriter::new(ObservedWriter { inner: writer, ... })` in the example versus `FrameWriter::new(writer)` in production. The example also needs a control descriptor and uses assertions instead of production startup diagnostics. `ObservedWriter::poll_write` first calls the actual inner `StdioWriter::poll_write` and reports `P` only for its `Poll::Pending` after a prior positive write; it reports `R` only for a later `Ready(Ok(n))` with `n > 0`. It forwards the unchanged buffer and result; flush and shutdown also delegate. The control pipe is separate from stdout/stderr, is checked by pipe inode, and carries at most four event bytes. Neither adapter nor parent adds, drops, or rewrites stdout payload bytes. The event is below the `FrameWriter` mutex, so mutex waiting alone cannot produce `P`.

The local `rmcp-3.8.0` `StdioWriter` delegates on Linux FIFOs to Tokio's pipe sender. Tokio's `PollEvented::poll_write` first awaits write readiness, then attempts the write; it can return `Pending` without an `EAGAIN` syscall. A spurious or unrelated task wakeup does not itself make the stdout registration writable. This is evidence for observed async transport backpressure, **not** a claim that a kernel write returned `EAGAIN`.

`check_probe_wiring()` is an effective tripwire for its listed expressions: I changed the `stdio_pipes()` expression in an in-memory `Path.read_text` result, and it failed with `AssertionError(('transport wiring drift', 'let (reader, writer) = stdio_pipes();'))`; all spawned children were reaped. It is a substring check, so it cannot prove semantic equivalence or catch unlisted changes. Side-by-side source review is still required. `scripts/check-schemas.py` builds `archy-mcp` before invoking `parse_errors.py`; a direct invocation of `parse_errors.py` expects an existing shipping binary while building the probe example itself.

## Blocking protocol finding and exact regression reproduction

The example batch sends 16 `tools/list`, 48 `get_project` calls, and 16 malformed lines. It checks 80 complete JSON frames, each valid-request ID exactly once, 16 exact `-32700` parse errors, and marker exclusion. The shipping binary run sends the same batch plus a priming request and checks 81 frames. In both loops, a reply with an expected valid ID is accepted without checking that it has a `result` rather than an `error`.

I ran this in-memory mutation from the reviewed worktree; it changes only `Reader.frame`'s parsed return value for IDs 1000–1063, not repository files or child stdout bytes. It turns all 64 valid responses into fabricated errors in **both** batch checks:

```sh
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B - <<'PY'
import pathlib
p = pathlib.Path('crates/archy-mcp/tests/parse_errors.py')
s = p.read_text()
needle = '        return json.loads(raw)\n'
replacement = "        reply = json.loads(raw)\n        if reply.get('id') in set(range(1000, 1064)):\n            return {'jsonrpc':'2.0','id':reply['id'],'error':{'code':-32000,'message':'fabricated'}}\n        return reply\n"
assert s.count(needle) == 1
s = s.replace(needle, replacement, 1)
ns = {'__name__': 'probe', '__file__': str(p.resolve())}
exec(compile(s, str(p), 'exec'), ns)
print('IN_MEMORY_VALID_RESULT_CORRUPTION_ACCEPTED')
PY
```

Observed exit 0 with `MCP stdout transport Pending/resumed writes: 2 pipe bytes: 4096`, the suite's final `PASS`, and `IN_MEMORY_VALID_RESULT_CORRUPTION_ACCEPTED`. This is a test-oracle failure, not an observed production response failure. A direct shipping-binary sample returned `result.tools` containing `get_project` for `tools/list`, and for `get_project` returned `result.isError == false`, a nonempty text `content` array, and `structuredContent`; the malformed line returned the exact parse error. Require `result` and absence of `error` for each well-formed ID; assert `tools/list` has a tools array with the advertised `get_project` descriptor, and `get_project` has `isError == false`, expected content/structured result shape, and project identity. Keep the 16 exact parse-error objects, unique IDs, marker check, and complete-frame accounting. Apply equivalent assertions to the shipping-binary cross-check.

## Backpressure ordering, fault injection, and limits

The normal live script exited 0. An in-memory instrumentation at each parent drain measured 1,216 bytes after the first `P` and 4,096 after the second; both `R` reads came after those drains. A second in-memory run inserted 100 ms between the zero-time no-`R` check and the drain: `PREDRAIN_R_QUEUED 0 False`, `DRAINED_AFTER_P 0 1216`, `PREDRAIN_R_QUEUED 1 False`, `DRAINED_AFTER_P 1 4096`; the complete suite passed. This is real event-order evidence for those runs, not a universal timing proof.

The source checks the control pipe for queued `R` once, then calls `drain_available()`. An `R` could theoretically arrive between that zero-time check and the first `os.read`; the helper also does not assert a positive drain count. The local Linux pipe writer and no-drain result make a false resume from unrelated wakeups unlikely in this setup, but the test does not atomically prove that parent drainage caused every `R`. To make the claim exact, count bytes read in each drain, require a positive count, and check for queued `R` immediately before the first read (or otherwise record event/read order). No-drain fault injection is a useful regression gate.

For SIGSTOP, I inserted `os.kill(process.pid, 19)` immediately before the batch's `thread.start()` in memory, preserving the real child, payload, and events. It failed after the 10-second event deadline with `AssertionError((b'P', 'stdout write event timeout'))`; all four children were reaped. Skipping `drain_available()` in memory failed waiting for `R` with `AssertionError((b'R', 'stdout write event timeout'))`; again all four children were reaped. The 10-second `P`/`R` deadline is shared across both cycles; the subsequent frame loop has a 20-second aggregate deadline, with separate 10-second sender join and child wait bounds. These two forced paths exercised cleanup; they do not prove every possible I/O failure path. The control writer's synchronous four one-byte writes cannot fill its otherwise unused pipe in this run. The sender thread is joined after child kill on failure; ordinary and forced cases did not deadlock.

## Validation and portability

| Check | Observed result |
| --- | --- |
| `python3 -B crates/archy-mcp/tests/parse_errors.py` | Exit 0; two `Pending`/resumed cycles, 4,096-byte pipe, final PASS |
| 16 parallel live script invocations, eight workers | 16/16 exited 0 with both PASS markers; Cargo's normal build-lock messages went to stderr |
| `python3 -B scripts/check-schemas.py` | Exit 0; live MCP suites, nine MCP schema pairs, 16 source schemas, 50 decision cases, six demo tests |
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy --locked -p archy-mcp --example stdout_backpressure_probe -- -D warnings` | Exit 0 |
| `git diff --check`; round-three freeze | Exit 0; seven hashes OK |

The backpressure harness uses Linux `F_SETPIPE_SZ`, `/proc/self/fd`, Unix inherited descriptors and signals. That fits the stated Omarchy/Arch Linux acceptance target. It is not a Windows/macOS runtime proof; `stdio_pipes` has a blocking fallback there. A different Linux page size or pipe-size policy could reject the requested 4,096-byte pipe before the probe runs. The example instrumentation is test-only. The shipping binary is separately run with concurrent `subprocess.run(input=..., capture_output=True)` drainage; that validates full batch framing and IDs, but does not itself observe a shipping-binary `Pending` event.

The published integration head is now `160787d1f6c65a85d0e022a111d56ee7b0eb2b86`. Its `archy-mcp` changes are `src/server.rs`, `src/tools.rs`, `tests/mutations.py`, and `tests/stdio_contract.py`; there is no direct file overlap with this review's example or `parse_errors.py`. The A15 server/tool behavior still needs validation after carrying the repaired test into integration; no integration worktree was edited here.
