# A15 live compare follow-on integration handoff

29 October 2026. Isolated worktree: `/home/butahlecoq/work/archy/.worktrees/a15-compare-followon-20261029`, detached at published A15+A21 base `f1e2477047dbafbacfba3229daddab0ca58ebe02`. The new worktree began at `0cfcf9164da20e6f22691b6ab92f305dc20fb679`; the reviewed base patch `a15-a21-on-a18-0cfcf916.patch` (SHA-256 `a2a89c2e4ae00883854399d80af090f415dff537dc2bc2fe9694447612809ac0`) applied there. Its source matched `f1e2477` exactly; that commit additionally contains 13 review documents. The worktree was reset to the clean published commit before this follow-on was assembled. No baseline review documents were edited.

## Reviewed source and reconciliation

The source is `.worktrees/a15-compare-integration-20261029`, detached at `ae6e6a6923fad5a4aae343cea67fcf40f62fd43e`. Its 37-entry `docs/plans/a15-compare-freeze.sha256` passed `sha256sum -c`; manifest SHA-256 `f694d40b6d125a75b25081fc9e47ddb8dd6209d26ec9884e1e23fa7f57ef8ee`. The independent scoped PASS review is `docs/plans/a15-compare-review-sol.md` in that source worktree (SHA-256 `3321918a9748db1b76092e43bbcb188de867bdb4c8023d90d2983c32cae5ad50`). That review does not close A15 mutation or approval work.

Only the comparison changes were carried forward: shared CLI verified-run rendering, the MCP compare handler and typed schema, the live compare stdio test, its required dependency and lock edge, existing stdio expectation updates, and the MCP README. `scripts/check-schemas.py` gained only `compare_stdio.py` in its live suite tuple. The newer A14 decision probe, A18 six demo tests, existing two A15 stdio suites, A09 controller, and A21 notice coverage remain on the published base. `Cargo.lock` adds only the local `archy-report` dependency edge for `archy-mcp`; registry versions and graph are unchanged. No other worktree was modified.

The 11 changed source, schema, test, and documentation paths have exact SHA-256 values in `docs/plans/a15-compare-followon-freeze.sha256`. That manifest excludes this handoff, so it can be checked without a self-reference. The follow-on patch is relative to `f1e2477`, not to `0cfcf916`.

## Validation

All Cargo commands used `RUSTUP_AUTO_INSTALL=0`, `CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=2`, and unique SSD `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-compare-followon-20261029`.

- `cargo test --locked --workspace`: exit 0; 199 passed, 2 ignored across 40 reported suites. Log `/tmp/archy-a15-compare-followon-tests.log`.
- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0. Log `/tmp/archy-a15-compare-followon-clippy.log`.
- `python3 scripts/check-schemas.py`: exit 0; nine MCP schema pairs, A14 50 cases, actual modern/legacy and parse/framing stdio suites, live compare CLI parity/paging/stale cursor/tamper/nonmutation suite, and A18 six demo tests. Log `/tmp/archy-a15-compare-followon-schemas.log`.
- `node crates/archy-mcp/tests/unicode_regex.js`: exit 0, eight Unicode fields.
- `PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest discover -s tests/packaging -q`: exit 0, 24 tests.
- `notices.py` admitted the existing reviewed vendor tree against the follow-on lock: 91 registry rows. Fresh inventory SHA-256 `215d1f061bfd35362cdd0ffaf7a596c434d1e3bd30355f62bac7a249efaa07bd`, bundle SHA-256 `bb7333062a0d0885d0cc4bde6370374d72760fe515f0a2513cce79fc2dd09184`; both match the reviewed base handoff.
- Exact-head forward patch check: exit 0 when applied to a clean `git archive` of `f1e2477047dbafbacfba3229daddab0ca58ebe02`; all 11 follow-on freeze entries then passed `sha256sum -c`, and the handoff matched byte for byte.

`compare_variants` is a passive fixed-project read. Each page has typed A14 documents for selected accepted generation/edit runs, ranked within each run under `absolute_target_gfa_delta`. It offers no cross-run rank, writer, backend, approval, or professional authority. A15 remains partial. No commit, push, install, package distribution, or child agent was used for this follow-on.
