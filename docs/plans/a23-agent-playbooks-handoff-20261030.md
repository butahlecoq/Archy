# A23 playbook candidate rereview record

This records the playbook-only candidate before integration. The combined checkout includes the canonical 27-gate Omarchy harness; see the integrated A23 handoff for current commands and hashes.

Base: `e61c977d3e8b504dfafd2e81689e97f62c934e76`. Previous A23 candidate: `95b28fe8c9091a8540419f63e1ba5ef3502f2cc8`. This revision resolves both findings in `.tools/github-updates/a23-agent-playbooks-review-sol.md`. Review this frozen worktree independently. Do not push, merge or close #23 on this handoff.

The original `--target-probe` ran an unqualified `archy` from inherited PATH. A local fake executed and printed `{"ok":true}`; the probe reported `observed` with exit 0 and all nine gates deferred. The mini harness also duplicated acceptance inventory. This revision deletes `tests/acceptance/omarchy/a23_harness.py`, routes the playbook to the separate canonical `scripts/validate-omarchy.py`, and leaves the A23 tool/authority tests independent. It adds an optional integration test against the canonical harness for hostile PATH nonexecution, rejected agent approval, preserved failures and deferred target status. The test skips only when that harness is absent; run it against the separate read-only checkout for review and again after integration.

In the playbook-only candidate, `scripts/validate-omarchy.py` and `docs/validation/omarchy-gates.md` were absent. The combined checkout now includes both. The playbook points to G08 agent autonomy, G09 human control, and OM-A23-01 clean golden/IFC/visual review without maintaining another gate table. The A19 owner still supplies named native evidence; A20 independently reviews it. No target gate passed here.

The nine MCP descriptor names and schemas still match. Live reads are get/validate/list/compare; live mutations are generate/evaluate. Render, IFC and proposal are advertised but return `BACKEND_UNAVAILABLE` at this base. The skill rejects agent self-approval and uncertain-outcome double writes. Herdr/tmux remains optional; no socket API, Archy API key or hosted service is required.

Changed in this revision: `docs/agent-playbooks/omarchy.md`, `docs/plans/a23-agent-playbooks-{plan,handoff}-20261030.md`, `tests/acceptance/omarchy/test_a23_playbooks.py`; deleted `tests/acceptance/omarchy/a23_harness.py`. No core, MCP, packaging, shared harness or other worktree file was edited.

Validation evidence on this WSL development host:

- A temporary PATH with fake `archy` and `omarchy` reproduced the original bug: fake `archy` executed, `{"ok":true}` yielded `probe.status: observed`, process exit 0, and all nine old gates stayed deferred.
- `python3 /home/butahlecoq/.codex/skills/.system/skill-creator/scripts/quick_validate.py integrations/agents/archy` → `Skill is valid!`, exit 0.
- `python3 -m unittest discover -s tests/acceptance/omarchy -p 'test_a23_*.py' -v` → 4 tests run, 3 passed, 1 integration test skipped because the canonical harness is absent in this checkout; exit 0.
- With `ARCHY_OMARCHY_HARNESS_ROOT=/home/butahlecoq/work/archy/.worktrees/a23-omarchy-harness-e61`, the same A23 test command → 4 passed; exit 0. The second worktree was read only.
- In that separate worktree, `python3 -m unittest discover -s tests/acceptance/omarchy -p 'test_harness.py' -v` → 7 passed; exit 0.
- `python3 scripts/check-schemas.py` → exit 0; nine MCP schema pairs matched descriptors, live stdio and mutation checks passed, all 26 negative CLI fixtures conformed, and actual doctor JSON conformed with target acceptance deferred on WSL.
- `git diff --check` → exit 0.

Reviewers should inspect the diff and rerun the A23 and canonical suites after integration. Source/WSL checks do not establish native Omarchy, worker, GPU, IFC or visual acceptance. A19 must supply target install, worker, containment, CLI/MCP, plugin, preview, IFC and visual evidence; A20 must independently review the acceptance matrix. No GPU or human visual result exists here.
