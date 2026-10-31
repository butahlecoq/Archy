# A15 generate/evaluate MCP handoff for independent Sol review

**Frozen worktree:** `/home/butahlecoq/work/archy/.worktrees/a15-generate-evaluate-20261029` on `feat/a15-generate-evaluate-20261029`, based on `f1e2477047dbafbacfba3229daddab0ca58ebe02`. No commit, push, install, approval, or child-agent action was performed.

## Scope

`generate_variants` and `evaluate_variant` are live typed MCP mutations. The CLI and MCP use the same `generate_locked` and `evaluate_locked` bodies, `LockedProject::begin`, publication, replay, and semantic run verification. The MCP binding retains the launch-time `ProjectStore` root descriptor and pinned source bytes. It checks configured root/source identity before and after taking the storage lock, validates managed history before lock creation, checks the accepted head's project ID, and checks an evaluation source run's project ID before starting the evaluation. No mutation call reopens the CLI project path. Four tool calls may be active; mutations serialize in process and storage locks serialize across processes. Output revisions are decimal strings.

Input requests require explicit idempotency key, expected revision, and project-relative destination. `LockedProject::begin` provides exact replay, changed-request and stale-revision conflicts, path validation, and orphan recovery. The six other tools remain registered; render, compare, IFC export, and proposal still report typed prerequisites in this worktree. No final approval or arbitrary process tool is exposed.

The root and source path checks are entry checks. Storage writes remain on the pinned root descriptor even if the configured path changes later; they cannot be redirected to a replacement root. External rename during publication remains a review point because path identity cannot be held by a filesystem rename lock. The live test proves replacement before a mutation is refused and no new root is written.

## Evidence

All commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-generate-evaluate-20261029` except `cargo fmt`.

- `cargo test --locked -p archy-core -p archy-cli -p archy-mcp`: exit 0. The full CLI suite included the long 64-run history test; core storage interruption, locking, and replay tests passed. This run preceded the final bound-reader preflight and source-ID guard.
- `cargo test --locked -p archy-core -p archy-mcp`: exit 0 after those changes, including core crash/recovery and MCP bounds/framing tests.
- `cargo test --locked -p archy-cli --test desktop_readers`: exit 0, seven tests after those changes.
- `python3 scripts/check-schemas.py`: exit 0 after final code changes. It checked all nine advertised schema pairs, actual serializers, A14 decision contracts, three existing MCP reads, stdio parse/error cases, the new live mutation test, and six A18 demo tests.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0 after final code changes.
- `cargo fmt --all -- --check` and `git diff --check`: exit 0.

`crates/archy-mcp/tests/mutations.py` drives actual stdio generate and evaluate requests. It validates successful outputs against advertised schemas and text parity, concurrent duplicate replay, restart replay, changed request, stale revision, u64 overflow, max-u64 parsing, wrong project ID, wrong historical source project, traversal and symlink destinations, rehashed duplicate outcome ID, tampered accepted history, partial managed history, and source/root replacement. It models the post-run-publication crash window using a real MCP-produced run whose revision record is removed, then proves exact retry recovers the same run and one revision. That simulation supplements the core's actual interruption tests; it is not a killed-server timing test.

## Reconciliation and review

A15 compare follow-on `8e36e11d1399dee37dc2b85a8e2225b473571602` overlaps this slice in `crates/archy-mcp/src/server.rs`, `tools.rs`, `tests/stdio_contract.py`, `docs/mcp/README.md`, and `scripts/check-schemas.py`. Reconciliation must retain compare's live handler, output schema, and `compare_stdio.py` alongside these two mutations and `mutations.py`. A02 v2 and A09 accepted binding are also separate worktrees; this slice has not modified them. Review should focus on descriptor identity across mutation, source project binding, semantic replay checks, and combined schema runner behavior. Independent Sol review and integration are outstanding. Render/export/proposal and final human approval remain outside this slice.
