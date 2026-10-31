# Current handoff — 29 October 2026

## Current verified state

- Branch feat/release-0.1, draft PR https://github.com/butahlecoq/Archy/pull/27. All authored deliverables are committed/pushed in bounded commits; work in flight is committed by its owner before handoff.
- Full supplied Word specification and extracted text are in docs/specs. Omarchy/reuse research is in docs/research/omarchy-and-reuse.md; backend detail in docs/plans/backend-research.md.
- All26 issues are published and independently verified: A01-A23 (#1-23) release scope; R01-R03 (#24-26) later roadmap. See issue-map.md and tracker-review.md. main requires rust CI plus one approval, stale dismissal and admin enforcement; no merge performed.
- Task01 init/validate baseline is implemented and independently reviewed PASS after two fix rounds. Code commits59d3c90 and c11157b; final review13f8f4d. Reports: task-01-report.md, task-01-handoff.md and task-01-review-final.md.
- Pinned Rust1.99.1 runs in Ubuntu WSL. Final scope covers12 core tests and5 CLI tests plus26 negative fixture cases through core/real CLI schema checks. fmt/clippy/build/schema checks passed. Both GitHub CI runs on c11157b succeeded (40877097234,40877108347). Later documentation-only CI can be inspected separately.
- Task02 A03/A04 generation/evaluation implementation is starting with Sol medium from base13f8f4d. Requirements and analytic fixtures: task-02.md. No generation, worker, IFC, MCP or Omarchy desktop completion is claimed yet.

## Current next actions

Finish and independently review A03/A04, then A05 persistence/reports and remaining typed Blender/IFC/MCP/native Omarchy slices. Keep original release requirements intact. Original issue acceptance spans integration requirements (run snapshots, complete entity schemas, downstream CI), so do not equate a completed first slice with closure of all A01/A02 or release gates.

## Decisions and environment

Omarchy4.0.6 stable is the researched target;4.5.0-rc2 is not assumed deployed. Reuse Quickshell, mise, agent skills/sessions, Linux isolation and existing modeling/BIM systems. No new viewport, scheduler or cloud service. Original core is MIT; workers/dependencies retain separate obligations.

Explicit _m/_m2 always means SI, generic dimensions obey declared input units, duplicate representations fail. Scalar-looking valid IDs are safely quoted by init. Shape parameters will be authoritative; stored footprint/identifier consistency is required before evaluation/rendering.

GitHub authentication and WSL work. Older sandbox/auth/toolchain failures in historical reports are not current blockers. Actual Omarchy execution, desktop/visual/GPU and clean-target acceptance are deferred by the founder until a machine is available; Ubuntu/CI evidence is not target acceptance. Never modify Windows security settings.

Downloaded tooling, raw upstream reading caches, build outputs and temporary command payloads are not repository deliverables. All authored source/research/plans/reviews are committed and pushed. Task implementation and review remain Sol medium; root coordinates/researches.
