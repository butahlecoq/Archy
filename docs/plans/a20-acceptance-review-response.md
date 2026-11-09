# A20 acceptance matrix review response

This response addresses the independent findings in `.tools/github-updates/a20-acceptance-review-sol.md` against reviewed commit `67b386c41ee3fec790f7d412acfef3979fff1801`. It requests a new independent review of the [acceptance matrix](../validation/acceptance-matrix.md). It does not approve release 0.1.

| Review finding | Correction and rereview check |
|---|---|
| Broken GitHub Actions retention link | Replaced the 404 with GitHub's repository Actions settings page, whose retention section documents finite run, artifact and log retention. Check the live URL and its retention text. |
| Source evidence could drift | Replaced the matrix's relative source citations with `blob/6536a586c5486367e9e2d5e958862a1402e2a31a` or `tree/6536a586c5486367e9e2d5e958862a1402e2a31a` permalinks. The introduction now distinguishes pinned source from live tracker and external documentation. Check every cited path in that tree. |
| G01 omitted the observed pixel result | G01 now requires paired preview measurements, a defined tolerance, render environment/settings and an actual pass/fail verdict for each view before passing. It keeps parametric and metric repeat results separate from GPU render comparison. |
| Issue #20 final evidence was aggregated | Added a final candidate checklist for exact candidate-bound Rust format/Clippy/tests, Python lint/tests, schemas, security/reliability, golden demo, artifact validation, install/README, changelog/release note, tag proposal, known limitations, licenses, individual blocker/exclusion issue dispositions and independent review. Every item is pending. |
| A22/published-head boundary | Added an explicit checkpoint: A22 read-only status is in published `6536`; the separate desktop open/install worktree at `15446` has uncommitted changes and is unpublished. Its scoped development review cannot pass installed Omarchy, plugin launch/approval or full demo gates. |

The §7 Blender minimum is one preview and a GLB per variant. The separate §2 golden journey requires three camera views per variant, or nine previews for the three-variant demo. The matrix records both requirements. WSL worker fixtures and Ubuntu CI remain scoped development evidence. G06, C09 and clean target installation remain untested on Omarchy. The production CLI/MCP render and export/approval workflow remains failing at published `6536`; release 0.1 remains blocked.

Rereview should inspect the matrix and this response at the new documentation commit, then verify source permalinks, external link reachability, issue #20 traceability and the pending/failing/untested classifications. No target acceptance, release tag, asset, GitHub issue edit, push or A22 worktree edit is part of this correction.

Validation on 2026-10-30: the matrix contains 58 Markdown links (42 distinct). A source-tree check found zero relative factual citations, zero non-`6536` repository source citations and zero missing cited paths at `6536`. All 13 distinct non-source external and live URLs returned HTTP 200 to HEAD requests; the repaired GitHub page also displayed its retention section. HTTP 200 does not prove a release result. The documentation diff passed `git diff --check`. No product acceptance suite or target-machine test ran for this correction.
