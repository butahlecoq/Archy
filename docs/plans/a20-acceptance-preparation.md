# A20 acceptance matrix preparation

Base: published `6536a586c5486367e9e2d5e958862a1402e2a31a`. Scope: an initial acceptance matrix and this review handoff in an isolated worktree. Issue #20 remains open. No release asset, tag, tracker change, or external publication is authorized by this slice.

## Plan recorded before the matrix edit

1. Read the complete release specification, issue #20, backlog, research, current source, CI and GitHub state. Check official platform, packaging, release and IFC references. Keep the specification's required tests intact.
2. Define measurable row predicates and distinguish verified evidence from incomplete, failed, and untested gates. Anchor source references to the published commit and give each external result a run or issue identity.
3. Check every cited local path and remote result, inspect the diff, freeze a patch digest, and hand the matrix to an independent reviewer. Do not claim reviewer approval.

Risks: mutable branch links can drift; old handoffs can overstate the current published source; WSL or Arch container work can be mistaken for Omarchy; passing unit tests can be mistaken for a production workflow. Use commit-pinned source links, name the environment and command for evidence, and keep the complete release gate blocked until target evidence exists. No dependency is added.

## Handoff for independent review

Changed files are this plan/handoff and `docs/validation/acceptance-matrix.md`. The matrix names the source revision and CI runs, maps all ten §7 tests plus the §7.2 aggregate gate, and traces the other mandatory release contracts. It preserves open target gates and the current integrated demo failure. The issue and live tracker were read, not changed.

Direct evidence checked on 2026-10-30: `gh issue view 20 --json number,title,body,state,labels,url,comments`; `gh api repos/butahlecoq/Archy/issues?state=all\&per_page=100` (A01–A23 all open); `gh api repos/butahlecoq/Archy/branches/feat/release-0.1` (full SHA above); `gh api repos/butahlecoq/Archy/branches/main/protection` (strict `rust`, one review, stale dismissal, admin enforcement); `gh pr view 27 --json headRefOid,baseRefName,headRefName,state,isDraft,statusCheckRollup,reviewDecision,url` (draft, review required, two successful `rust` jobs); commit check-runs and Actions runs API (IDs in matrix). Source inspection included the CLI command enum, MCP server's unavailable tool branches, workflow, golden evidence, packaging and license handoffs. Official GitHub, Arch, Blender and IfcOpenShell references are linked in the matrix.

Validation for this documentation slice: all 57 Markdown link targets were enumerated; all local targets existed in the worktree. `git diff --cached --check` exited 0 with the two new files staged. No Rust, Python, backend or Omarchy acceptance was run as part of this documentation slice. Existing hosted CI is bound only to published `6536a586`, not to this local docs commit.

Reviewer: verify each row against the pinned source and actual evidence, with special attention to the unavailable MCP tools, required versus optional scope, CI coverage, packaging notices, and the distinction between WSL, an Arch container and a real Omarchy host. Request corrections in this isolated branch; no review verdict is asserted. Next owner action is to finish dependent integration and target gates, update issue dispositions, then rerun the full matrix on a new frozen candidate. Do not close A20 on this preparatory document.
