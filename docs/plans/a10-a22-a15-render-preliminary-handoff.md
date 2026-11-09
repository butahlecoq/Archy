# Preliminary A10/A22 desktop and A15 MCP render integration

Date: 2026-10-30. This is a frozen candidate for independent combined review, not publication or installed Omarchy acceptance.

For the later integration of A17 and the orphan recovery binding fix, see [the follow-on handoff](a10-a22-a15-orphan-a17-integration-handoff.md). The evidence and outstanding items below describe the earlier `59650fc` candidate.

## Inputs and resolution

- Base: `103072bfa5d7f3a961341e0df5b4af4c7858acb2` (descended from published `c72f530`). The isolated worktree is `.worktrees/a10-a22-a15-render-preliminary-20261030`.
- Cherry-picked A15 scoped render `bbbc3c6908e55355cb4e96ec361298905c28d5c3` and reviewed publication callback fix `6382d9be27e3a35f01ae770db22152dd9cfc1aa0`, retaining source trailers in separate commits.
- Grafted independently reviewed desktop schema/doctor patch from `4e10b353476643394c8d5eee44ffa0055c3cc29f` as a separate commit. Grafted reviewed camera warning patch from `b7a8a3ce87150b0e70dd5faad923bf045318aa78` as a separate commit; its applied binary diff SHA-256 is `adfefe4f1d4468ded2ab831b702fc4eddf950767f4fbc2d2dcb5ffb9acfe7d5e`.
- The only cherry-pick content conflict was `crates/archy-cli/src/render.rs`. Kept A10/A22 current v2 and historical v1 source handling, preview symbol receipt/desktop/replay behavior, and fixed opener tests. Added the A15 locked render entry point and binding callback at entry, before commit, and through `commit_checked` around publication. `run_integrity.rs`, A23 files, and QML remain at the base. The desktop 32 MiB response bound remains in place.
- Restored `JsonSchema` derives required by A15's typed render output and regenerated only `schemas/mcp/render_variant.schema.json` to include A10 preview symbols. A live MCP probe found that invalid variant selection left a staging directory; moved `begin()` after source and selection validation, with the same-key digest conflict check still preceding validation. The probe then passed.

## Validation on this integration

Commands used `TMPDIR=/home/butahlecoq/work/archy`, `CARGO_BUILD_JOBS=2`, and isolated `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a10-a22-a15-render-preliminary-20261030` on ext4 (`/dev/sdd`). Cargo used `--locked --offline` or `CARGO_NET_OFFLINE=true`.

- `cargo test -p archy-adapters --lib --test protocol`: 26 library and 9 protocol passed; 1 unrelated IFC test ignored by its existing gate.
- `cargo test -p archy-cli --lib --test accepted_worker --test desktop_readers --test render_publication`: 14 + 5 + 8 + 4 passed after camera graft. After the staging fix, the focused binding test and all 4 render publication tests passed again.
- `cargo test -p archy-mcp`: 9 transport tests passed. `node --test tests/omarchy/integration/*.test.js`: 24 passed. `node crates/archy-mcp/tests/unicode_regex.js`: 8 fields passed. `python3 -m unittest tests.omarchy.integration.test_desktop_status_schema tests.omarchy.integration.test_plugin_contract`: 8 passed.
- Final `python3 -u scripts/check-schemas.py`: exit 0. It matched all 9 generated MCP schema pairs, validated 50 source/domain cases, actual desktop CLI output, all 26 negative CLI fixtures, five live MCP stdio probes including render and historical v1 replay, and 6 demo tests. The render probe checks invalid-request nonmutation, exact replay, stale/conflicting requests, and corrupt replay refusal. The CLI doctor report conforms with exit 1 because target acceptance is deferred.

## Remaining grafts and review limits

- A separate investigation is addressing the inherited `LockedProject::begin` orphan-recovery path, which can publish a revision without the new `commit_checked` callback. Graft its reviewed fix when available, then rerun publication and live MCP replay checks. The existing callback tests prove the new-run boundaries, not that every possible publication is race-proof.
- Request-derived camera warning metadata and rehashed replay refusal were independently reviewed for the camera patch. This integration has not independently proven pixel-to-model correspondence or an installed Blender/service-gate success. On this host the installed gate is absent, so live MCP renders retain `failed/BACKEND_UNAVAILABLE` with `all_verified:false`.
- The combined branch itself still needs independent review and target acceptance. No push, merge, or service installation occurred.
