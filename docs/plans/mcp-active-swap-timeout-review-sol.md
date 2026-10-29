# Independent MCP timeout patch review — 2026-10-30

**Verdict: PASS for the narrow timeout adjustment.** Full schema verification remains with the parent.

## Identity and scope

- Patch: `/var/tmp/archy-mcp-timeout-hMPxHy/mcp-timeout-fix.patch`; SHA256 **`59c183010f38cd97431c5d8095c848170b31db93bc3af2edfc261eb0f649e332`**. The unstaged diff of the two affected files in the frozen source has the same SHA256 and 3,099 bytes. Hash rechecked after testing.
- Inspected the actual `crates/archy-mcp/tests/mutations.py` and `scripts/check-schemas.py`. The only longer MCP waits are the response after the 100-variant active swap and that same request's retry and idempotent replay, for both `replace_root` branches. Each uses 60 seconds. The staging probe remains 15 seconds; `replies()` and `call()` retain 15-second defaults; all other calls use those defaults. Schema validation, response checks, error-code checks, revision-count checks, and replay equality assertions remain intact.
- `check-schemas.py` assigns 300 seconds only to `mutations.py`; the other MCP scripts remain at 120 seconds. No service code or ordinary response deadline changes are in this patch.

## Fresh verification

- Ran the complete live `python3 -B crates/archy-mcp/tests/mutations.py` from the isolated source, with `CARGO_TARGET_DIR=/var/tmp/archy-mcp-timeout-hMPxHy/borrowed-target`, a new private ext4 `TMPDIR`, and an external `timeout 300` guard. Exit **0**, `ELAPSED_SECONDS=131.16`; output: `MCP live generate/evaluate/compare, historical read, replay, conflict, scope, tamper and replacement: PASS`. The reused binaries were read only: CLI SHA256 `df55ae3ea27c544080f6e696448c0d3b37b445de2e7c9de2791e876f7e56fafd`; MCP SHA256 `0baf3bf766b3de97f39555b64b5df5730932a64d8d4f02cf442d411a1cb008f7`.
- Negative check executed the actual parsed `frame`/`replies` functions against a live process that supplied no stdout. `timeout=0.05` raised `AssertionError: MCP response timeout` after **0.053 seconds**; inspected signatures confirmed both helper defaults are **15 seconds**. This verifies that a silent process still fails instead of waiting indefinitely.
- The 300-second outer limit exceeded this full run by **168.84 seconds** (about 2.29 times its duration). It is adequate for the observed busy-host run, but it is not a mathematical upper bound for every permitted inner wait: two branches can each reach a 15-second staging probe and three 60-second response waits, before the rest of the test. If all these waits approach their limits, the outer limit can fire first.

## Diagnosis and limits

- The prior evidence reports four completed 100-variant responses at 18.17, 14.63, 23.00, and 22.94 seconds, with service CPU time increasing during two slow waits. That establishes progress for those live calls and supports a load-sensitive timeout diagnosis for the reproduced failures. It does **not** rule out deadlocks in other executions or prove that every future call completes within 60 seconds.
- The host remained loaded: initial load average **14.96** on **8 CPUs**, CPU `some avg10=59.09%`, swap **2.0/2.0 GiB**; near completion load average **16.26**, CPU `some avg10=41.11%`, swap still full. Another MCP mutation process was active concurrently. This run therefore exercises the fix under contention, but its 131.16-second duration is environment-dependent.
- No full `check-schemas.py` rerun was performed here, per parent ownership. No candidate, original, or source files were modified by this review; the review's private temp directory was removed.
