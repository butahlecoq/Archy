# A10/A22 desktop, A15 render and orphan recovery integration

Date: 2026-10-30. This candidate is frozen for independent combined review. It is not published or accepted on Omarchy.

## Inputs and integration

- Resumed the combined `59650fcabc175cb6a62ae96dff13a2f8714a597e`. Its scoped desktop compatibility and new-run security reviews passed; those reviews apply to that exact commit, not this later combination. Four pinned Blender preset tests were independently run on `59650fc` and passed; see `.tools/github-updates/a10-preset-candidate-596-local.md` outside this worktree.
- Merged published `15108bad8a187ad838eca5b0c524836a2ebd4175`, retaining its A17 edit-interruption test, handoff and recovery documentation byte-for-byte. Merged its published child `d303cbcad4ed269df02d9d440c95ef4aa088290d`; `.github/workflows/ci.yml` matches that commit. GitHub Actions for `d303cbca` were reported pending; this worktree does not establish their result.
- Cherry-picked independently reviewed `f5ac61fd64d57ef5ecc88205e1c94235cc6ba516`, retaining the source trailer. Four non-render files applied cleanly. The sole conflict was in `render.rs`: the candidate had moved `begin` after source and variant validation to avoid staging on invalid requests. The resolution calls `begin_checked` at that location and retains the orphan recovery retry test and the existing `commit_checked` callback.
- The newly published A23 CI step exposed an outdated A23 assertion and guidance that described render as unavailable. Updated the agent skill and Omarchy playbook for the fixed `draft` render and its `all_verified`/failed receipt, and updated the corresponding contract test. The other A23 paths, human-consent boundaries and deferred target gates remain intact; IFC export and approval proposal remain unavailable.

## Verification

Cargo and live tests used `TMPDIR=/home/butahlecoq/work/archy`, `CARGO_BUILD_JOBS=2`, and isolated `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a10-a22-a15-render-preliminary-20261030` on ext4 (`/dev/sdd`), with locked offline dependencies.

- `cargo test -p archy-core --lib --test recovery_staging`: 9 + 11 passed, including real source/root replacement before orphan revision publication.
- `cargo test -p archy-cli --lib --test edit_interruption --test accepted_worker --test desktop_readers --test render_publication`: 14 + 2 + 5 + 8 + 4 passed. The A17 test exercises a killed staged edit, request fingerprint, same-key retry and restore. The render binding test rejects an orphan retry before revision publication and then recovers the original run without repeating worker work.
- `cargo test -p archy-cli --test golden_artifacts new_children_of_authentic_claimed_v1_history_migrate_authority_profile`: 1 passed. `cargo test -p archy-mcp`: 9 transport tests passed.
- `cargo clippy -p archy-core -p archy-cli -p archy-mcp --all-targets -- -D warnings`: passed.
- `node --test tests/omarchy/integration/*.test.js`: 24 passed. MCP Unicode regex probe passed for 8 fields. Desktop schema and plugin Python suites passed 8 tests.
- `python3 -u scripts/check-schemas.py`: exit 0; all nine MCP schema pairs matched, with live stdio mutation/render probes, canonical v2 and historical v1 replay, 50 source/domain cases, 26 negative CLI fixtures and 6 demo tests. The live render probe confirmed invalid input does not create staging and exact retry returns one receipt.
- A23 playbook and harness suites passed 4 + 7 tests. The remaining 16 signed-review tests were not run locally because they start GPG agents, under the no-services constraint. The published workflow includes the full 27-test A23 step.

## Review and runtime limits

`begin_checked` rechecks the binding before publishing a recovered orphan revision, while `commit_checked` retains its new-run checks before and after run publication. These checks are sequential filesystem observations. They do not lease the configured path: a rename between a check and publication or a transient swap restored between checks can still evade detection. Neither the integrated branch nor continuous pathname identity has received a final combined review.

The installed service gate is absent on this host. Live MCP rendering therefore retains `failed/BACKEND_UNAVAILABLE` with `all_verified:false`; no installed worker success or Omarchy acceptance is claimed. The prior Blender preset checks do not establish pixel-to-model correspondence. No push or service installation occurred.
