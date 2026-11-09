# A02/A04 authority migration on published head: final review handoff

30 October 2026. This isolated, detached worktree starts at published head `1e2313fe0bf20f69f246836334adb761276d2390`. The source patch is `a02-a04-ff7-integration-20261030-authority-migration.patch`, SHA-256 `48630f44c324ab1dff1575efbed38cd6ff4f3b218fe2e04ad8a4147e32b4a52d`, based on `ff7f6bc`. The independent original-candidate rereview in `.tools/patches/a02-a04-authority-rereview-sol.md` passed its 101 hashes and authority probes, but did not review this combined tree. The 64-plus-run stress test timed out in that rereview and is not counted as passed here.

## Integration decisions

- `git apply --3way --index` reported three conflicts: `crates/archy-cli/src/generate.rs`, `crates/archy-cli/src/evaluate.rs`, and `crates/archy-mcp/src/tools.rs`. Generation keeps A15's locked mutation and pinned source bytes while dispatching fresh work through canonical v2 and preserving historical keyed replay. Evaluation keeps A15's locked entry point and callback while migrating new claimed-v1 descendants before `begin()`. MCP tools keeps the A15 mutation receipt shape and adds the v2 normalized representation.
- A09's `BoundProjectRead` now allows a read-only historical snapshot when the current source fails v2 parsing. Its accepted-worker path originally accepted a malformed current source. The worker boundary now requires either a valid current source or a valid historical v1 source; the mutation boundary requires current parsing. This preserves worker capture for authentic v1 sources with preferences that v2 import rejects, while malformed current source still fails. The initial failing `accepted_worker` assertion at line 172 and the final five-test worker suite are recorded in the target logs.
- The MCP mutation test's 100-variant retry/replay exceeded its 15-second response deadline under concurrent host load in two full schema runs, at different requests. A later 30-second attempt also timed out. The unchanged script passed separately; disposable timing probes measured the large replies first at 7–13 seconds and later at 17.6–28.5 seconds, all completing with the expected responses. Its response budget is now 60 seconds; the schema runner allows 360 seconds for that script alone. These changes affect test deadlines, not MCP transport or mutation behavior.
- A15 storage, scoped mutation/recovery, and MCP write handlers remain on the published line. A16 `authority/` and its ingress/journal boundary, A09 accepted-worker capture and service gate, and the published MCP stdout backpressure implementation were not replaced. No commit, push, or other worktree edit was made.

## Verification

All final checks use `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2`, the unique ext4 target `/home/butahlecoq/work/archy/.tools/targets/a02-a04-published-20261030`, and short workspace `TMPDIR=/home/butahlecoq/work/archy/.t`. Logs and `.exit` files are in the target.

- `cargo test --locked --offline -p archy-cli -p archy-core -p archy-eval -p archy-mcp -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable`: exit 0, 166 passed across 28 result groups; `core-cli-eval-mcp-final2.log` and `.exit`. The excluded stress case is not claimed.
- `cargo test --locked --offline -p archy-authority`: exit 0, 24 passed across seven result groups, including socket policy and journal tests; `authority-final-short.log` and `.exit`.
- `python3 -B scripts/check-schemas.py`: exit 0; `schema-final4.log` and `.exit` for the final source and test bytes. It includes source-derived schema contracts, CLI negative fixtures, MCP descriptor pairs, current/historical reads, comparison, framed transport/backpressure, live mutation, and demo checks.
- Format, whitespace, manifest and patch identity checks are recorded with the final freeze.

The first focused Rust run failed in A09's malformed-current-source worker case before the boundary fix. Three full schema attempts reached the 100-variant MCP mutation retry/replay and timed out at 15 or 30 seconds. The unmodified mutation script passed separately; a disposable timing probe passed with all large requests at 17.6–28.5 seconds during the latest high-load run. A16 authority tests initially failed because longer workspace `TMPDIR` values exceeded Unix socket path limits; all passed under `.t`. Failed attempts are not counted as passes.

## Freeze and next review

`docs/plans/a02-a04-published-integration-freeze.sha256` hashes the integrated changed paths excluding itself. The exact base-relative binary patch and its SHA-256 sidecar are under `.tools/patches/a02-a04-published-1e2313-20261030.*`. Review those bytes against `1e2313f`, particularly the A09 source-validity boundary, A15 replay and pinned source, the claimed-v1 migration, MCP mutations and backpressure, and A16 preservation. This is a prepared candidate, not final approval.

The later published head `c8c8641c796e82554573f4b63cc7c41a070314b4` descends from this base through `6536a58`. It adds reviewed A18 contributor documentation and A20 acceptance matrix documents. A18's `README.md` overlaps this patch; A20 adds files only. Final integration must preserve both documentation layers and reconcile the README without discarding either change. This worktree and patch remain based on `1e2313f` for exact review.
