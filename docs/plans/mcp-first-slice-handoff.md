# A15 first read-only slice handoff

29 October 2026. Scope follows the MCP plan and the earlier SDK probe. Product server implemented for independent review; this is not A15/A16 workflow closure.

## Short plan and result

1. Pin `rmcp =3.8.0` (`server`, `macros`, `transport-io`) and Tokio `=1.54.1` minimal runtime/I/O features in the workspace, preserving prior dependency pins. Cargo resolved the official SDK and macro packages offline from the verified probe cache.
2. Reuse the CLI's `BoundProjectRead` for descriptor-pinned, noncreating source/history reads. The server never calls `lock`, bootstrap, recovery, or a CLI process. A new pinned-source check in core and a desktop status refactor put both readers on that path.
3. Advertise exactly nine source-derived schema pairs. Three read handlers return verified data; six handlers return typed prerequisites. Wire bounds and the SDK's dual-version transport were tested through the actual binary.

## Review files

- `crates/archy-mcp/{Cargo.toml,src,examples/contracts.rs,tests}`: stdio server, bounded syntax adapter, nine contracts, size/depth guards, and real protocol tests.
- `crates/archy-cli/src/project_read.rs`, `crates/archy-cli/src/{lib.rs,desktop_status.rs}`, `crates/archy-cli/tests/desktop_readers.rs`: shared pinned read interface and replacement regression.
- `crates/archy-core/src/storage.rs`: pinned source handle and root/source identity checks needed by a long-lived reader.
- `Cargo.toml`, `Cargo.lock`, `schemas/mcp/*.schema.json`, `scripts/check-schemas.py`, `docs/mcp/README.md`.

## Evidence and limits

Focused live transcript `python3 crates/archy-mcp/tests/stdio_contract.py` passes modern discovery, legacy initialize, explicit unsupported version error, exact nine SDK descriptors matching checked-in schemas, three reads with output-schema validation, six prerequisite errors, malformed args, wrong project, oversized frame recovery, multi-variant paging, stale cursor, corrupt/symlink history, live source replacement, and filesystem inventories. `cargo test --locked -p archy-mcp --test bounds` passes four guard tests. Existing desktop reader tests pass seven cases, including locked reads, semantic tamper refusal and root/source swap. The six unfinished operations do not claim success. No Omarchy or backend execution was attempted.

Response content and JSON-RPC frame bounds are separate: results above 400 kB become a typed bounded error before SDK text duplication; the transport writer rejects any encoded response over 1 MiB before writing it. The input adapter discards an oversized line and handles the next request. Depth violations terminate the affected stream. Tool calls run on at most four blocking read tasks; no writer authority is present. Cursors carry a digest, offset, and last variant ID. The digest binds the accepted head as well as root, project, source digest, revision, and query. Old revision cursors and altered last IDs conflict. `get_project` and `validate_project` verify the accepted head ancestry, and `list_variants` verifies accepted runs; the shared verifier recomputes full evaluations before returning data. The rehashed false-evaluation regression now asserts this shared head check refuses the forgery.

The live stdio tests are Python executables invoked by the schema checker used in CI. SDK protocol interoperability beyond this wire transcript and real Omarchy isolation remain separate acceptance work. Independent review should scrutinize the transport guard contract, historical revision semantics, source/root swap races, and output-schema strictness.

## Round-one review fixes

The round-one review at `docs/plans/mcp-first-slice-review-round1.md` and the independent parallel review at `/home/butahlecoq/work/archy/.tools/reviews/a15-parallel.md` reproduced three defects. All three failed against the previous freeze before changes.

1. Explicit `get_project` revision `0` used the pinned current manifest after a run existed. Storage has no immutable genesis source snapshot, and the first run's source cannot prove what revision `0` contained. The selector now returns a typed `REVISION_CONFLICT` once any revision is accepted. On a genesis project it still returns the current source. An omitted revision still returns the pinned current source. The live source-edit test checks old revision `1`, changed current source, and revision-zero refusal.
2. The nine output schemas allowed `ok` with missing data or missing error. The generated schema now has two closed `oneOf` branches. Success fixes `ok` to `true`, requires nonnull typed `data`, and requires null `error`. Failure fixes `ok` to `false`, requires null `data`, and requires a typed Archy error. The checker rejects impossible envelopes for all nine tools, and the stdio test validates actual success and error results against the advertised schemas.
3. `compare_variants.run_ids` had a 100-item array limit but no item length limit. Its generated `items` schema now requires 1 to 128 characters. The schema checker accepts a 128-character ID and rejects a 129-character ID. The live handler still rejects the latter with JSON-RPC `-32602`.

The fixes do not add a genesis file, storage write, root selector, mutation handler, or approval action. The fixed-root, no-repair read path and transport bounds remain in place.

## Round-two review fix

The independent review at `docs/plans/mcp-first-slice-review-round2.md` reproduced a mismatch between JSON Schema character counts and the handler's UTF-8 byte limits. The previous schema accepted 65 `é` characters in `compare_variants.run_ids`; the handler rejected the 130-byte string. Accepted run IDs are UUIDs generated by storage. Project IDs and idempotency keys are ASCII in core, and variant IDs are ASCII hashes. The schema and handler now require ASCII for project, run, and variant IDs, idempotency keys, and cursors.

Presets, objectives, destinations, and reasons retain Unicode and their byte ceilings of 128, 128, 512, and 4096 bytes. Standard JSON Schema `maxLength` counts characters, so these fields use conservative width tiers. If `N` is the field's byte ceiling and `W` is the widest UTF-8 character in the string, the number of Unicode characters must be at most `floor(N / W)`. The schema expresses the same tiers with standard patterns and `maxLength`; the handler also checks actual bytes. This can reject a mixed string whose actual UTF-8 length is below `N`. It does not increase any byte or frame limit.

The live test compares each bounded input schema with the handler for ASCII limits, multibyte identifiers, two-, three-, and four-byte Unicode text, mixed text, and values on both sides of each tier. Existing tests cover all nine closed output schemas, actual success and error results, revision-zero refusal, current-source behavior, paging, corruption, and no-write inventories.

The direct probe at `/tmp/archy-a15-round2-probe.log` records that `é` repeated 64 and 65 times is rejected by both the run-ID schema and wire. As an objective, 64 copies are schema-valid and reach `BACKEND_UNAVAILABLE`; 65 are schema-invalid and return `-32602`. The same boundary holds for 32 versus 33 four-byte characters. The probe also records the conservative mixed-string cutoff and confirms that reads create no files.

## Round-three review fix and SDK limitation

The independent review at `docs/plans/mcp-first-slice-review-round3.md` reproduced an unpaired-surrogate mismatch. Python's JSON Schema validator accepted a lone surrogate in Unicode text, while the pinned server discarded the corresponding line. The Unicode schema's final width tier now accepts scalar values and well-formed UTF-16 surrogate pairs and rejects lone high or low surrogates. Earlier ASCII identifier rules, UTF-8 byte ceilings, and the 1 MiB frame limit are unchanged. The existing conservative width-tier restriction for mixed Unicode text still applies.

[JSON-RPC 2.0](https://www.jsonrpc.org/specification#error_object) specifies `-32700` with a null ID for invalid JSON when the ID cannot be detected. [MCP stdio](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports) requires UTF-8 JSON-RPC. [RFC 8259 section 8.2](https://www.rfc-editor.org/rfc/rfc8259#section-8.2) notes that the JSON grammar can represent an unpaired surrogate escape, but that it is not a Unicode character and is not interoperable. The pinned `serde_json` string parser rejects the escape as a syntax error. The [rmcp 3.8.0 async stdio transport](https://github.com/modelcontextprotocol/rust-sdk/blob/rmcp-v3.8.0/crates/rmcp/src/transport/async_rw.rs#L245-L327) explicitly drops syntax and EOF parse failures before they reach the service handler. It sends an invalid-request response for parseable JSON with an invalid message shape.

At the round-three freeze, malformed JSON, malformed raw UTF-8, and lone-surrogate frames received no parse-error response. Round four adds a bounded syntax adapter before the SDK; its behavior is described below. No input is rewritten or converted to replacement characters.

`stdio_contract.py` validates all eight occurrences of the Unicode fields across the nine tool schemas. It checks lone high and low surrogate endpoints, malformed pairs, escaped valid pairs, raw supplementary UTF-8, a following valid response, malformed UTF-8, malformed JSON, EOF behavior, and an unchanged project inventory. `unicode_regex.js` checks the patterns in an ECMAScript UTF-16 regex engine without external packages. The existing nine output-schema, historical revision, paging, and no-write regressions also pass.

## Round-four parse-error fix

The review at `docs/plans/mcp-first-slice-review-round4.md` reproduced the SDK dropping complete malformed JSON, invalid UTF-8, and lone-surrogate frames. The pinned [rmcp 3.8.0 transport](https://github.com/modelcontextprotocol/rust-sdk/blob/rmcp-v3.8.0/crates/rmcp/src/transport/async_rw.rs#L245-L327) discards serde syntax and EOF parse failures. Its [error type](https://github.com/modelcontextprotocol/rust-sdk/blob/rmcp-v3.8.0/crates/rmcp/src/model.rs#L592-L606) omits an absent ID during serialization, so it cannot emit `id: null` through that type.

`crates/archy-mcp/src/framing.rs` adds a bounded line adapter before the existing SDK transport. It uses the existing serde JSON parser to screen complete frames; it never extracts an ID or routes an RPC. The line buffer is capped at 1 MiB, the depth guard remains 64, and the temporary parsed value is bounded by that input. Valid frames pass unchanged to rmcp. An unparseable frame receives the fixed response `{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null}`. No input bytes appear in that response. A single asynchronous writer lock serializes this frame with all SDK output. Partial writes retain the lock until flush, and input state survives cancelled reads. The 1 MiB line cap, 64-level depth guard, output guard, and discard of an incomplete trailing line at EOF remain in force. Oversized lines are discarded and the next line is processed.

[JSON-RPC 2.0](https://www.jsonrpc.org/specification#error_object) requires a null ID for an unreadable ID on a parse error. The error-response descriptions for both [MCP 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/basic#error-responses) and [MCP 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic#error-responses) describe an optional string or number ID. For parse errors this facade deliberately sends JSON-RPC's explicit null, as requested; a client enforcing the MCP type literally may reject it. Well-formed messages with an invalid RPC shape still use the SDK's `-32600` handling, including its omission of an unreadable ID. That separate SDK behavior is not changed here.

`parse_errors.py` verifies modern and legacy stdio, fragmented lines and UTF-8, BOM and CRLF compatibility, multiple frames in one write, both lone surrogate halves, invalid UTF-8, malformed JSON, unknown IDs, a near-limit untrusted input without echo, oversized unterminated input, 64/65 depth, concurrent SDK responses while stdout reaches pipe backpressure, incomplete EOF, and a nonmutating inventory. `framing.rs` tests cancellation of partial reads and parse-error output, and frame serialization over a backpressured stream. The CI schema checker runs both live stdio scripts. Existing nine-schema, Unicode, revision, and fixed-root regressions remain active.

## Dedicated worktree verification and freeze

Worktree: `/home/butahlecoq/work/archy/.worktrees/a15-mcp`, branch `codex/a15-mcp`, reviewed base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`. The main worktree remained untouched. No commit, push, agent, or Omarchy execution occurred.

Set these variables for every Cargo command:

```sh
export RUSTUP_AUTO_INSTALL=0
export CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-mcp
export CARGO_BUILD_JOBS=2
export CARGO_NET_OFFLINE=true
```

The final worktree checks passed:

- `cargo fmt --all -- --check`: `/tmp/archy-a15-round4-final-fmt.log`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: `/tmp/archy-a15-round4-final-clippy.log`.
- `cargo test --locked --workspace`: 148 tests across 32 suites, `/tmp/archy-a15-round4-final-tests.log`.
- `python3 scripts/check-schemas.py`: nine MCP schema pairs and existing fixtures, `/tmp/archy-a15-round4-final-schemas.log`.
- `python3 scripts/check-schemas.py` also builds `archy-mcp` and runs `stdio_contract.py` and `parse_errors.py` on actual modern and legacy stdio. Their results are in `/tmp/archy-a15-round4-final-schemas.log` and the earlier focused logs `/tmp/archy-a15-round4-stdio-focused.log` and `/tmp/archy-a15-round4-parse-focused-final.log`.
- `node crates/archy-mcp/tests/unicode_regex.js`: ECMAScript UTF-16 pattern checks for all eight field occurrences, `/tmp/archy-a15-round4-final-ecma.log`. Node.js is optional for the other checks.

The round-four freeze had 31 source and schema entries. `/tmp/archy-a15-worktree-review.patch` and its reverse-apply check recorded the round-four state; that patch predates the round-five fix below.

## Round-five depth framing fix for independent Sol review

The independent finding at `docs/plans/mcp-first-slice-review-round5.md` reproduced a valid `tools/list` line followed by a depth-65 line in one stdin write: the old `DepthGuard` scanned both lines as one read and returned `InvalidData` before forwarding either. I reproduced exit 1 with zero stdout frames. The new live regression failed on that original binary, and the new Rust guard regression failed because its first read returned the depth error instead of the complete valid line.

`DepthGuard` now retains at most one 8192-byte input chunk and returns at a newline. It scans the next line only on a later read, so complete preceding lines reach the SDK before a later depth violation. Its quote/escape state still crosses chunks and resets at line boundaries; depth 64 remains accepted and depth 65 still ends input. `JsonLineFilter` continues to own the 1 MiB line cap, syntax screening, parse-error/null-ID response and incomplete-EOF behavior. `FrameWriter` and `ResponseGuard` are unchanged. No JSON-RPC router or new authority was added.

The live regression sends three valid `tools/list` requests and a depth-65 frame in one write, then repeats with the 65th opening bracket in a second write. It asserts three complete result frames. It drains stdout while the process runs to exercise the writer without blocking on a full pipe. The first schema-check run exposed that test-harness pipe deadlock and a truncated response; after correcting the harness, the schema check passed. Existing live tests cover depth 64, parse-error null IDs, max 1 MiB and oversize recovery, Unicode, EOF, concurrent output under backpressure, and fixed-root no-write reads. The new Rust tests cover a complete line before a later depth error and a depth-65 transition across reads.

A separate direct subprocess probe sent the same three valid requests plus depth 65 in one write to 20 fresh processes in the dedicated target. All 20 returned three complete result frames with IDs 7, 8 and 9.

Validation from this worktree with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-mcp-round5-fix PYTHONDONTWRITEBYTECODE=1`:

- `cargo fmt --all -- --check`: passed.
- `cargo test --locked --workspace`: passed, including the new guard tests and all seven desktop reader cases. Cargo reported only the pre-existing same-name `contracts` example output collision warning.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `python3 scripts/check-schemas.py`: passed; built the CLI/MCP binaries, compared nine schema pairs, and ran both live stdio suites.
- `node crates/archy-mcp/tests/unicode_regex.js`: passed all eight Unicode field occurrences.

The 31-entry freeze manifest is `eb22a4240529c03a7d37121d39ec23eabb984ee7ab898c7ba76329f8c7e55106`. Changed file SHA-256 values: `crates/archy-mcp/src/bounds.rs` `55a330dd9f04561e9d870a5a9fdc9b9f550c5dd7ec724e4fba729144de34443a`; `crates/archy-mcp/tests/bounds.rs` `080140dfec70ee9e3313e2a1082d4de2c3c3f7b2654751da69dd705440ef43fa`; `crates/archy-mcp/tests/parse_errors.py` `ba5dbf7e1b68f305d2e60fc12ab89d0c09d03193df5c86ad5b5b285fd792343c`. All other manifest entries remain as in round four. Run `sha256sum -c docs/plans/mcp-first-slice-freeze.sha256` before independent review.

The SDK's pre-existing parseable-invalid-request omission of a null ID remains outside this syntax/depth fix. This handoff requests independent Sol review of the new frame ordering and live wire evidence; it does not self-approve the slice. No commit, push, installation, or other worktree change was made.
