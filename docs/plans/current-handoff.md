# Current handoff — 29 October 2026

## Verified state

- Branch: feat/release-0.1; original upstream base 155df35.
- Full supplied Word specification and extracted text are under docs/specs.
- Research for Omarchy and reusable projects is in docs/research/omarchy-and-reuse.md; backend API/version research in docs/plans/backend-research.md.
- A01-A20 ticket descriptions exist in docs/plans/backlog.md. Remote issues were still absent at the last check; publication is the next orchestration task.
- GitHub authentication works with ADMIN repository permission. Earlier sandbox authentication failure is historical, not a current blocker.
- Rust 1.99.1 runs in Ubuntu WSL; Cargo.lock was resolved. Build and clippy passed on the current initial implementation; tests had 8 passes and 1 failure, and formatting failed. Exact commands/results: docs/plans/toolchain-revalidation.md.
- The failing translated-site test depends on LF-only replacement against CRLF input. Independent review also found unexercised negative fixtures, incomplete error paths/schema checks and missing baseline documentation. See docs/plans/task-01-review.md.
- No implementation slice is claimed complete. No Omarchy desktop, Blender or IFC runtime acceptance has been performed. Actual Omarchy execution is deferred by the founder until a target machine exists.

## Next work

1. Publish/reconcile the specification issues and add Omarchy integration tasks from the research.
2. Complete D1-D2 with Sol medium; resolve reviewed findings, run WSL checks, obtain independent Sol medium review.
3. Implement deterministic generator/evaluator and immutable run/report pipeline, then typed Blender/IFC/MCP and native Omarchy integration in dependency order.

## Decisions

Omarchy 4.0.6 stable is the researched baseline; 4.5.0-rc2 is not assumed deployed. Reuse Quickshell plugin/theming, mise, agent skills/session tools, Linux isolation and existing modeling/BIM systems. Do not build a custom viewport, scheduler or cloud service. Original core license selection: MIT; worker/dependency obligations remain separate.

SI input ruling: explicit _m/_m2 values always mean SI, generic dimensions obey declared input units, and simultaneous generic/SI representations are rejected. This replaces the earlier overly restrictive meters-only alias rule and resolves the review/plan conflict. Update schemas/fixtures accordingly.

The founder asked that all authored work/findings be committed and pushed. WIP commits document failures; a commit is not a release or test-pass claim. External dependency downloads, raw upstream reading caches, build outputs and helper scratch files are not repository deliverables. Windows security settings must not be modified.
