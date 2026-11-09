# A23 agent playbooks plan (e61c977)

This is the playbook candidate plan. The combined checkout includes the canonical 27-gate harness; the integrated A23 handoff records its current validation state.

Scope at base e61c977: add a provider-neutral skill, Omarchy acceptance guide, optional Herdr/tmux layout, and A23 contract tests. The separately developed `scripts/validate-omarchy.py` and `docs/validation/omarchy-gates.md` own the single acceptance inventory. Do not publish the target commands as runnable from this checkout until that harness is integrated. Freeze for independent review; do not push or close issue.

1. Read the issue, release baseline, live MCP descriptors/schemas, CLI, existing docs, and pinned upstream Omarchy/Herdr sources.
2. Test skill tool/field claims against the real descriptors. Exercise canonical harness dry-run, hostile PATH probe nonexecution, forged consent rejection and failure preservation when the separate harness is available. After integration, run those tests in the combined checkout.
3. Write the skill and playbooks around current capabilities, safe project paths, immutable evidence, human authority, and optional sessions.
4. Run frontmatter validation and source checks. Run A23 contract tests locally and against the separate harness read-only. Record exact results and unresolved target gates for review.

Risks: planned tools may be advertised before backend wiring; agent-client shortcuts can auto-approve; WSL/CI can masquerade as target evidence; Herdr internals may change; PATH shims may execute during a target probe. Mitigation: name the three unavailable tools, use only supported launch/session UI, require a recorded human decision, and use the canonical harness's fixed installed paths and deferred target status.

Dependency rationale: no new runtime dependency or API key. A23 target instructions depend on the separately reviewed harness being integrated first; local skill and contract tests remain usable without it. Reuse Archy MCP/CLI and installed agent/session clients.
