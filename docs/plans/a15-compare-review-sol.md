# A15 live `compare_variants`: independent read review

29 October 2026. **Verdict: scoped PASS for the frozen read operation.** No reproducible defect was found in `compare_variants` under the exercised live stdio, historical, failure, and concurrent-read cases. This does not close [A15 issue 15](https://github.com/butahlecoq/Archy/issues/15): five mutation tools still return prerequisites, so the issue's idempotency, revision, approval, and full agent workflow acceptance remain open. The per-run comparison contract is explicit; it is not a global ranking across selected runs.

## Scope and identity

Worktree `/home/butahlecoq/work/archy/.worktrees/a15-compare-integration-20261029`, HEAD `ae6e6a6923fad5a4aae343cea67fcf40f62fd43e`. I read `AGENTS.md`, the A15 handoff and plan, release spec §5, backlog A15, issue 15, the frozen 37-file inventory, the prior MCP first-slice handoff and reviews rounds 1–6, A14 comparison reviews, relevant storage/read/verification/report/MCP source, and the A02/A18 worktree code and checker state. The earlier MCP review defects concerned revision zero, output envelope, Unicode bounds, malformed input, and depth ordering; the present live schema/stdio suites exercise their corrected paths.

`sha256sum -c docs/plans/a15-compare-freeze.sha256` returned `OK` for all 37 entries before and after review. Manifest SHA-256: `f694d40b6d125a75b25081fc9e47ddb8dd6209d26ec9884e1e23fa7f57ef8ee`. Relevant frozen values: `server.rs` `1f53d531d615afd3130317478a48761df6c7eb5209d0d708be4c12cbc65d169e`; `tools.rs` `b7fda531654eadb691f2290a35e3726fe1e101a6fac0aed95d8ba8d05996cebb`; `project_read.rs` `d6dbf17b5906b7c3f3e1ee5064c37927b8432b9dde54c2a39971e8c6867ca1f5`; `compare.rs` `108b527be2d28a04be08f3ba323ab77b619d87d29ce1456a95ac1c0607060cc7`; `compare_stdio.py` `4f9c261d5ce9484e2c5b594b5018fdbfa0b9ac0546e9c0c3205ee62a7efa8631`; comparison schema `f542b0bfbf0c8ff1cc313aba4a84a365c37c7051986422aad747c8c14121c1d7`; handoff `fde08c5561b84bb724547cd8e330b69dac05837017212b515db242c9a6099c02`. The manifest contains the remaining exact hashes.

## Findings and evidence

1. **Verified source and parity.** The live handler uses `BoundProjectRead::snapshot`, verifies the accepted head and each emitted selected run, then calls the CLI's `render_verified_comparison`. In my independent actual-stdio probe, a private copied five-revision accepted project yielded three generation/edit documents over one-run pages, in caller order. Each document equaled actual `archy compare --json --run <id>` output for the same copied project. Objective values were in ascending order within each document, with the A14 renderer's dense rank and deterministic tie policy. The documents retained `daylight` and `cost` as uncomputed. The live checked-in suite separately generated two current runs and compared a returned document to CLI JSON. These observations support exact per-run parity; they do not establish a cross-run score.
2. **Bounded and stale selection.** The independent wire probe rejected malformed and forged cursors, foreign project ID, unsupported `cost` objective, unaccepted and duplicate run IDs. A 129-byte run ID, page size 101, and 101 selected IDs returned JSON-RPC `-32602`. The checked-in live suite also exercises changed run order and a new accepted head with the old cursor; both return `REVISION_CONFLICT`. Input/output schema pairs matched SDK descriptors for all nine tools; successful and error envelopes passed their output schemas. Packed responses over 400 kB use a bounded typed error; that size path was inspected in `server.rs` but not forced in my independent historical probe.
3. **Passive reads and substitution refusal.** My inventory recorded mode, size, nanosecond mtime, and SHA-256 for every regular file, and symlink target where relevant. It was byte and metadata identical around successful and failed wire calls. Four fresh MCP stdio processes concurrently read the same historical comparison while another process held `.archy/lock`; all returned the same document and left the inventory unchanged. Replacing an accepted artifact with a symlink caused a typed refusal without repair or mutation. The checked-in live suite additionally refused a consistently rehashed metric forgery, a changed head, and replacement of the pinned source inode. Source inspection confirms descriptor-relative `openat2` managed reads, pinned root/source checks, and no lock/bootstrap/recovery call in `compare_variants`. The four-process probe establishes passive lock behavior for that case, not an exhaustive race proof against a cooperating writer committing at every boundary.
4. **Historical and merge limits.** This fixed-base worktree verifies accepted v1 bytes with `parse_project_v1` and renders each trusted historical run, independent of the current source. The in-flight A02 contract worktree adds `parse_project_current` and a v2 normalized DTO while its legacy `parse_project` remains v1. When A02 is merged, route current-source admission/output deliberately, preserve v1 historical recomputation and serialized bytes, and regenerate/check the MCP `NormalizedSpec` output schema. The in-flight A18 demo checker has six real CLI demo tests to retain at the checker tail alongside A14's decision-contract check and all three A15 live stdio scripts. Neither A02 v2 admission nor A18's merged checker was exercised here. The separate A21 locked-dependency notice gate was also outside this read review.

## Commands and outcomes

All Cargo invocations used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-compare-20261029`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`, and `CARGO_BUILD_JOBS=2`. Python checks used `PYTHONDONTWRITEBYTECODE=1`. No mocked backend was used as MCP evidence.

```text
sha256sum -c docs/plans/a15-compare-freeze.sha256
  37/37 OK, before and after probes
python3 scripts/check-schemas.py
  exit 0; nine MCP schema pairs, 50 decision cases, live modern/legacy stdio,
  parse/framing, current compare/CLI parity, tamper, bounds, and no-write suites passed
cargo test --locked --offline -p archy-mcp --test bounds --test framing -p archy-cli --test compare --test desktop_readers
  exit 0; 6 bounds, 3 framing, 4 CLI compare, 7 desktop reader tests passed
PYTHONDONTWRITEBYTECODE=1 python3 /tmp/archy-a15-sol-live-probe.py
  exit 0; copied historical five-revision project, three comparisons, CLI parity,
  cursor/selector/bounds refusals, lock-held four-process reads, symlink refusal,
  unchanged inventories
cargo fmt --all -- --check
  exit 0
node crates/archy-mcp/tests/unicode_regex.js
  exit 0; eight Unicode schema fields
git -c core.whitespace=cr-at-eol diff --check
  exit 0
```

The first independent probe attempt lacked MCP request `_meta` and received `-32602`; a second attempt copied managed fixture directories with public modes and received `PERMISSION_DENIED`. After supplying `_meta` and making the disposable directories private, one harness expectation used fewer than 101 IDs and correctly reached duplicate-ID validation. The corrected probe above passed. These were probe setup errors, not product failures. The probe script is disposable under `/tmp`, not part of the frozen source.

No product source, fixture, other worktree, commit, push, installation, or child agent was changed. This review document is the sole worktree addition.
