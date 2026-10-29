# A15 reviewed mutation integration on published compare

## Scope and plan

This uncommitted worktree is detached at exactly `8e36e11d1399dee37dc2b85a8e2225b473571602`. The plan was to verify the 17-path reviewed mutation freeze; bring the locked core and CLI transaction source over unchanged; reconcile MCP server, tool, schema runner, wire test, and documentation with published live compare; then run Rust, schema, and real stdio gates. `docs/plans/a15-mutation-on-8e.patch` is the review patch relative to that exact commit. Its SHA-256 and each changed source hash are in `docs/plans/a15-mutation-on-8e-freeze.sha256`.

The original `a15-mutation-review-fix-freeze.sha256` passed **17/17** in the source worktree before integration. Core `storage.rs` and CLI `project_read.rs`, `generate.rs`, and `evaluate.rs` remain byte-identical to that reviewed source. The frozen mutation delta in CLI `lib.rs` was applied onto published compare's version of that file. The two mutation schemas were applied from the frozen source. The live compare handler, `CompareVariantsData`, compare schema, `compare_stdio.py`, and CLI comparison code remain on the published base. The shared MCP server/tools/test/docs/schema runner were reconciled so generation and evaluation are live without reverting compare. The new integrated stdio test extends the frozen mutation probe with MCP generate → evaluate → compare and a historical revision read. This combined patch is awaiting separate Sol integration review; the round-two Sol verdict covered the source repair only.

No dependency was added. Existing `archy-cli`, `archy-core`, `archy-report`, and `rmcp` types are reused.

## Transaction and limit

The MCP mutation calls use the same locked CLI bodies and `PendingRun::commit_checked` as the reviewed repair. They require a fixed launch project, an idempotency key, expected revision, and project-relative destination. The frozen tests exercise duplicate concurrent calls, changed-key and stale-revision conflicts, source binding, invalid selectors before staging, and same-key orphan recovery. A detected binding change before staged-run publication refuses; one detected after run publication returns a retriable uncertain `REVISION_CONFLICT`. Descriptor-relative storage does not write into a replacement configured root.

An uncooperative root/source rename between identity check and publication or response can escape detection. A success receipt means the pinned project passed the observed checks. It does **not** establish atomic or continuous identity of the configured pathname. Retrying the same key after restoring the original binding resolves an uncertain outcome. The public MCP README states this limit and does not promise an atomic path lease.

## Integration boundaries

The integration head advanced to `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1` while this task ran. That reviewed process fixture fix changes only `crates/archy-adapters/src/process.rs` and its review documents, with no file overlap with this patch. It is absent from this exact-8e worktree and must be retained when applying this patch upstream. The independently active MCP pipe test fix was not copied or judged by this worktree; its gate must be run against the eventual combined head, and a failure must be reported rather than waived.

A02 v2 and A09 accepted-run integration are separate f1-based work and overlap CLI/core ownership. This patch is anchored to 8e and does not absorb either branch. Their merge must preserve this transaction API and verify their combined behavior, especially source/run binding, accepted history, replay, and CLI workflow. No arbitrary project path/destination, generic execution, final approval, backend install, commit, push, child agent, or other worktree edit is part of this task.

## Validation

- `cargo test --locked -p archy-core -p archy-mcp`: exit 0, including four synchronized publication swap cases, crash/replay, MCP bounds and framing.
- `cargo test --locked -p archy-cli`: exit 0, including 20/20 core workflow tests (the 64-run history test took 107.98 s), seven desktop reader tests, golden artifacts and doctor tests.
- `python3 crates/archy-mcp/tests/mutations.py`: exit 0 with real stdio generate/evaluate/compare, historical read, concurrent duplicate replay, stale/changed conflict, root/source swaps, recovery, and invalid-selector no-staging.
- `python3 crates/archy-mcp/tests/compare_stdio.py`: exit 0 with published compare parity, pagination, tamper refusal, and no writes.
- `python3 scripts/check-schemas.py`: **exit 1** at the unchanged `crates/archy-mcp/tests/parse_errors.py:208` pipe backpressure assertion: `available=60482`, `capacity=65536`, threshold `61440`. Before that failure it printed nine SDK schema pairs matching, 50 A14 decision-contract cases passed, all 16 schema files and CLI fixtures conforming, and the live `stdio_contract.py` read/framing checks passed. The script did not reach its compare, mutation, or A18 subprocesses on this invocation; those were run directly as listed here. The existing test assumes stdout reaches capacity minus `PIPE_BUF`; its measured plateau fell below that threshold. The independently active pipe repair changes that test and was not copied into this patch. This is a recorded failed gate, not a waived or green schema run.
- `python3 crates/archy-mcp/tests/parse_errors.py` then passed directly. One full `python3 scripts/check-schemas.py` retry also exited 0: 50 A14 decision cases, all nine MCP schema pairs, stdio parse/compare/mutation suites, and six A18 demo cases completed. The earlier failure remains evidence of a flaky gate on this base and needs the separately reviewed pipe repair in the eventual integration.
- `ARCHY_BIN=<unique-target>/debug/archy python3 -m unittest discover -s tests/demo -v`: exit 0, six A18 demo tests.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0.
- `cargo fmt --all -- --check` and `git diff --check`: exit 0.

All Cargo and MCP Python commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-reviewed-integration-20261029`; the A18 test used that target's CLI binary through `ARCHY_BIN`. `df` reports this target and workspace on `/dev/sdd` ext4; `lsblk` reports `ROTA=1` for that device. It is a unique target directory, but the host does not verify it as SSD. `/tmp` is tmpfs.
