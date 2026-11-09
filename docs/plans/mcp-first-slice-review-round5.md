# A15 MCP first read slice: independent round-five review

29 October 2026. **Verdict: ISSUES for the frozen slice.** The round-four parse-error adapter passes direct wire checks, but a depth-limit failure can discard a complete, valid request that precedes the offending frame in the same input read. This review makes no claim about full A15 acceptance or Omarchy runtime behavior.

## P2: a deep later frame can erase an earlier valid request

`DepthGuard` reads up to 8192 bytes and scans the entire chunk before forwarding any bytes to `JsonLineFilter` (`crates/archy-mcp/src/bounds.rs:32-70`). On the 65th opening bracket it returns `InvalidData` immediately, without forwarding even the valid line at the start of that chunk. `main.rs:29` places this guard before the line filter. The existing live depth test sends and receives a valid request *before* sending the deep frame, so it does not cover this batch case.

With the fresh round-five binary, I sent a complete `tools/list` request with ID `7`, followed by a 65-level JSON array, in one stdin write. In **20 of 20** fresh process runs the server exited `1`, printed `MCP transport startup failed`, and emitted **zero stdout frames**. The valid request therefore received no response. The 65-level frame may legitimately close the stream under the documented depth policy; losing an earlier complete request is the defect. A request already answered before sending the deep frame is unaffected, as the checked-in test demonstrates.

Reproduce from this worktree after the build listed below:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 - <<'PY'
import json, pathlib, shutil, subprocess, tempfile
binary = pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a15-mcp-round5-review/debug/archy-mcp')
meta = {'io.modelcontextprotocol/protocolVersion':'2026-07-28', 'io.modelcontextprotocol/clientInfo':{'name':'depth-probe','version':'0'}, 'io.modelcontextprotocol/clientCapabilities':{}}
valid = json.dumps({'jsonrpc':'2.0','id':7,'method':'tools/list','params':{'_meta':meta}}, separators=(',',':')).encode() + b'\n'
deep = b'[' * 65 + b'0' + b']' * 65 + b'\n'
with tempfile.TemporaryDirectory() as td:
    project = pathlib.Path(td) / 'project.yaml'
    shutil.copyfile('examples/courtyard-housing/project.yaml', project)
    for _ in range(20):
        result = subprocess.run([binary, '--project', project], input=valid + deep, capture_output=True, timeout=5)
        assert result.returncode == 1 and result.stdout == b''
        assert result.stderr.strip() == b'MCP transport startup failed'
print('20/20: earlier valid request had no response')
PY
```

The fix belongs at the framing boundary: preserve and deliver complete lines before acting on a depth error in a later line, while keeping the 64-level bound. Add a single-write regression with a valid request followed by a 65-level frame. I made no source change.

## Round-four behavior verified

The actual stdio binary returned exactly `{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null}` for complete malformed JSON, invalid UTF-8, an empty line, lone surrogate escapes, and a syntactically invalid line of exactly 1,048,576 bytes. A following valid frame was answered. A 1,048,577-byte line was discarded and the next frame answered. An incomplete final line at EOF produced no additional response. A 65-level frame closed the stream; a 64-level array reached the SDK and received `-32600`. Response order among concurrent requests and a parse error varied, but each observed frame was complete. The checked-in backpressure test filled the stdout pipe and accounted for 64 SDK responses plus 16 parse errors without interleaving or echoing the untrusted marker. The Rust writer tests also passed partial-write, cancellation, and backpressure cases.

The live `stdio_contract.py` check passed modern discovery, legacy initialize, exact equality of all **nine** advertised input/output schema pairs with `schemas/mcp`, three successful verified reads, six typed `BACKEND_UNAVAILABLE` tools, wrong-project refusal, source/root replacement, history corruption/symlink refusal, paging and stale cursors, revision-zero behavior, Unicode/schema bounds, and no-write inventories. The separate desktop reader suite exercised pinned-root behavior but had the lock failure below. These checks support the fixed-root, read-only authority claim for the exercised paths; they do not prove behavior under simultaneous in-place edits of a single source read or against external MCP clients.

## JSON-RPC and SDK limit

[JSON-RPC 2.0](https://www.jsonrpc.org/specification) requires `id:null` when a parse or invalid-request error cannot identify an ID. [MCP 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/basic) and [MCP 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic) say messages follow JSON-RPC 2.0, yet type an error response ID as an optional string or number. The adapter deliberately chooses JSON-RPC's explicit null for `-32700`; a client enforcing the MCP type literally may reject it. This is a specification/interop tension, not a newly observed dropped parse error.

For a parseable invalid request shape (`{"jsonrpc":"2.0","method":1}`), the live SDK returned `-32600` **without an `id` member**. That is not JSON-RPC's required null. The pinned rmcp 3.8.0 `JsonRpcError.id` is `Option<RequestId>` with `skip_serializing_if = Option::is_none` (`src/model.rs:597`), and its transport routes invalid shapes through that type (`src/transport/async_rw.rs:308`). This pre-existing SDK limit remains outside the syntax adapter's correction. The probe did not show this behavior for a valid request ID that the SDK could read.

## Separate old-base lock observation

`cargo test --locked -p archy-cli --test desktop_readers` produced six passes and one failure: `accepted_pages_are_verified_and_stale_cursor_conflicts_without_writes` hit `REVISION_CONFLICT`, `project lock held by another process`, at `desktop_readers.rs:136`. This worktree is based on `d4b6ca8`, which predates the published lock fix `f4aed22`; the user reports that the flake was independently reproduced, fixed, and reviewed on integration. I did not treat it as an MCP framing finding and did not rerun it.

## Exact source identity and commands

Worktree `/home/butahlecoq/work/archy/.worktrees/a15-mcp`, branch `codex/a15-mcp`, base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`, with uncommitted source changes predating this review. `sha256sum -c docs/plans/mcp-first-slice-freeze.sha256` passed all **31** entries before testing. Manifest SHA-256: `d1bcd5bc72fcc80902e758b5cf69d7ce31fa163b7a5a96cc185efa8f1a775ef4`; `bounds.rs`: `728f5f3a07306f4dfb1db48dad6134c01652e889a1bb963dec56373f3c133e92`; `framing.rs`: `c14ce7035f913929532afb2e8c88732093b834a450b9c9753b0128f8703631ba`; `main.rs`: `dc36c3c2fe0b08e90ad7d0ffd48f344720eadf576240a68b09e6c6d07f26a45b`; `server.rs`: `78e5116a9eeb5fab66c06573607c33909dc56292757077abc7ec17408538df6c`; `tools.rs`: `d29fbff5009741f60d9678f0a4007fab92051c94f891e6a9990287f109aec71c`.

Cargo commands used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-mcp-round5-review PYTHONDONTWRITEBYTECODE=1` from the worktree:

- `cargo test --locked -p archy-mcp --test bounds --test framing`: **7 passed**.
- `cargo build --locked -p archy-mcp -p archy-cli`: **passed**.
- `python3 crates/archy-mcp/tests/parse_errors.py`: **passed**, including modern/legacy framing, fragment/UTF-8 splits, EOF, oversized input, backpressure and no-write inventory.
- `python3 crates/archy-mcp/tests/stdio_contract.py`: **passed**.
- `python3 scripts/check-schemas.py`: **passed**; it reran both live MCP stdio suites, compared all nine schema pairs to SDK descriptors, and checked existing schemas, fixtures, and CLI output.
- `cargo test --locked -p archy-cli --test desktop_readers`: **6 passed, 1 old-base lock failure** as described above.
- `node crates/archy-mcp/tests/unicode_regex.js`: **passed** for eight Unicode field occurrences.
- Independent Python subprocess probes: exact 1 MiB boundary, over-limit recovery, `-32700`/null, `-32600`/missing ID, EOF, depth shutdown, and the 20-run valid-prefix loss: **observations above**. The first probe assumed response order and failed its assertion; I corrected it to compare IDs without ordering and it passed. No wire or source change was made between probes.

The only worktree file written for this review is this report. No source edits, commit, push, or host installation occurred.
