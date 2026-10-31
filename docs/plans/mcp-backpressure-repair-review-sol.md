# Independent MCP backpressure repair review — Sol

29 October 2026. Reviewed only `/home/butahlecoq/work/archy/.worktrees/mcp-backpressure-repair-20261029`. Base `8e36e11d1399dee37dc2b85a8e2225b473571602`. No source changes, commits, pushes, installs, other worktree edits, or child agents.

## Verdict

**Blocked for the intended actual-backpressure assertion.** The repair reliably exercises the live protocol and validates all 80 complete responses, but `!POLLOUT` on a 4 KiB Linux pipe does not establish that the server's next write is blocked. In a live run the first `!POLLOUT` occurred at 836 unread bytes; without draining, unread bytes rose to 1,216 over the next 100 ms. The test could therefore start draining before the server encounters backpressure. The prior byte-threshold failure is real, and the rest of the protocol checks pass. Route a narrower readiness repair before accepting this as a backpressure test.

## Evidence and commands

`git rev-parse HEAD` returned `8e36e11d1399dee37dc2b85a8e2225b473571602`. `sha256sum -c docs/plans/mcp-backpressure-repair-freeze.sha256` returned OK for all six listed files, including `parse_errors.py` at `f94eef362445c9a09f0dcab1f8ee3ed56298e07178ad635a923e6e7c226ab9c8`. I read `AGENTS.md`, `docs/plans/mcp-backpressure-repair-handoff.md`, the freeze file, `git show HEAD:crates/archy-mcp/tests/parse_errors.py`, the worktree diff, and the MCP bounds/framing sources.

The old assertion was `FIONREAD >= F_GETPIPE_SZ - PIPE_BUF`. An independent live probe used the existing binary, a 65,536 byte parent-owned stdout pipe, and 64 small `tools/call` requests. It waited for `poll(write_fd, POLLOUT)` to become empty before draining. Output: `capacity 65536 old_threshold 61440 POLLOUT [] first_no_POLLOUT 33904 unread_after_200ms 33904 sender_alive False`; after draining: `replies 64 unique_ids 64 all_result True sender_errors [] exit 0 stderr`. Thus the old threshold rejects genuine pipe write unavailability with intact replies. The probe was an inline `python3 -B - <<'PY'` command; it made no file edits.

The decisive live probe ran `runpy.run_path('crates/archy-mcp/tests/parse_errors.py', run_name='__main__')` under an inline `select.poll` wrapper. The wrapper delegated `register`, `unregister`, `modify`, and `poll` to the real poller, but on its first empty result measured `FIONREAD`, slept 100 ms without reading stdout, and measured it again. Command environment: `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B - <<'PY'`. Output: `FIRST_NO_POLLOUT 836 AFTER_100MS_NO_DRAIN 1216 CAPACITY 4096`, followed by the script's PASS line. A standalone Linux pipe probe independently showed `capacity 4096 unread 1024 poll [] write_1 1`. Linux `POLLOUT` promises room for an atomic `PIPE_BUF` write; an empty result does not rule out a smaller successful write. This directly disproves the claimed observation of actual blocked server output at the test's decision point.

The same live wrapper replaced only the `F_SETPIPE_SZ` requested size, then ran the complete unchanged script once per size. Results:

| Requested / actual bytes | First `!POLLOUT` unread | After 30 ms without drain | Script |
| --- | ---: | ---: | --- |
| 4,096 / 4,096 | 760 | 1,216 | PASS |
| 8,192 / 8,192 | 5,312 | 5,312 | PASS |
| 16,384 / 16,384 | 13,504 | 13,504 | PASS |
| 65,536 / 65,536 | 61,781 | 61,781 | PASS |

This host supports unprivileged `F_SETPIPE_SZ` shrinking to one 4 KiB page; the test's API and Python `fcntl` constant worked here. This is evidence for this Linux runner, not proof of every target CI kernel/page size. The 4 KiB case is sufficient to reject the present backpressure claim.

`CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B crates/archy-mcp/tests/parse_errors.py` passed. Sixteen concurrent runs of that exact script, launched with an eight-worker Python `ThreadPoolExecutor`, all exited 0 with the PASS line and empty stderr. The existing batch still checks unique IDs 1000–1063, 16 exact parse-error objects, no marker echo, full JSON frames, and sender completion. Its earlier sequential checks retain frame size, depth, malformed-frame recovery, legacy, EOF, and source nonmutation coverage. It does not assert order for the concurrent batch, and I make no ordering claim for it.

`CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 cargo test --locked -j 2 -p archy-mcp --test bounds --test framing` passed 6 bounds and 3 framing tests. `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B scripts/check-schemas.py` exited 0, including live MCP stdio suites, nine MCP schema pairs, 16 source schema files, 50 decision cases, and six demo tests. `git diff --check` exited 0.

The parent write descriptor remains open until the batch `finally` block. A direct pipe/child probe showed `child_exit 0 read b'ok' parent_writer_open True eof_ready False`, then after closing that descriptor `parent_writer_closed_eof_ready True eof_read b''`. This is the expected EOF behavior; the batch uses counted frames rather than EOF while the parent writer is open. A forced assertion at the first batch poll produced `caught FORCED_AFTER_SENDER_START`, `all_reaped True`, and no remaining sender thread immediately or after 100 ms. A forced poller that always reported writable reached the 10-second fill deadline: `caught (1216, 4096, 'stdout remained writable')`, `elapsed_seconds 10.52 children 4 all_reaped True`. Code inspection confirms each drain read receives the remaining portion of a 20-second aggregate deadline; sender join and process wait have separate 10-second limits. Those limits were not separately fault-injected.

## Required repair direction

Keep the full response accounting and cleanup. Replace the `!POLLOUT` assertion with a probe that proves the server's output write is actually stalled while input is pending, or otherwise establish full pipe occupancy under the exact frame/write pattern being tested. Re-run the live probe at the decision point; unread bytes must stop increasing without a drain, and the test must distinguish the early 836-byte observation from a genuine stall.
