# Independent rereview: A20 release 0.1 acceptance matrix

**Verdict: PASS for the corrected documentation slice; release 0.1 remains BLOCKED.** I found no remaining issue in the matrix that would misstate release acceptance. This is a review of evidence and documentation, not execution of the full release suite or an Omarchy acceptance run.

## Exact reviewed identity

- Candidate documentation commit: `21258f7144662788f09836b3861ed7e7817b89bb`, tree `5b9dfd76ed3c1183339c34630c644f11d1e1a39e`, parent `67b386c41ee3fec790f7d412acfef3979fff1801`.
- Comparison base and published source baseline: `6536a586c5486367e9e2d5e958862a1402e2a31a`. The candidate worktree was clean at the reviewed commit. `git diff --check` passed.
- Supplied full patch: `/tmp/a20-acceptance-rereview-21258f7.patch`, SHA-256 `48b7b5156aeefe2f2dc138f2f777bca46014b5e5b5c5c1803737a0c0bfcfef1e`. A fresh `git format-patch --stdout 6536a586..21258f7` produced the same digest. The base-to-candidate diff adds only `docs/validation/acceptance-matrix.md`, `docs/plans/a20-acceptance-preparation.md`, and `docs/plans/a20-acceptance-review-response.md`.
- Matrix SHA-256: `5c5f2dac322ac56d31702d4ca9d268ec26e686b79a32dc8231ff6808b200e0d7`. Full spec SHA-256: `8e76b2b39c2fbc37eea9e5da808baaaf657884deee3b4fd961bce053b8c24235`.

## Prior findings, checked independently

| Original finding | Rereview result |
|---|---|
| Broken GitHub Actions retention link | **Resolved.** Matrix line 14 uses the live [repository Actions settings page](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository). It returned HTTP 200; its retention section states that checks, runs, artifacts and logs have finite retention. |
| Source evidence could drift | **Resolved.** All 43 repository source citations in the matrix use `blob/6536a586...` or `tree/6536a586...`; all 28 distinct cited paths exist in that Git tree. No relative factual citation or other source revision remains. Live tracker/CI and external documentation are distinguished from frozen source. |
| G01 omitted observed render-pixel result | **Resolved as a gate definition, still pending as a release test.** Matrix line 22 requires paired previews, documented tolerance, actual measured difference and per-view pass/fail, with render settings and environment. It does not mistake the WSL core byte repeat for a render comparison. |
| Issue #20 final evidence aggregated too broadly | **Resolved.** Matrix lines 52–67 explicitly list candidate/tag identity; Rust format, strict Clippy and tests; Python lint/tests; schemas; security/reliability; golden demo and artifact validation; README/install; changelog/release note and known limitations; per-issue blocker/exclusion disposition; and independent review. Every final-candidate item is labeled pending and requires candidate-bound commands, exits and retained evidence. |
| A22/published-head boundary | **Resolved.** Matrix line 16 states that A22 read-only status commit `4cfd0b82...` is within `6536`, while the separate `15446eea...` desktop open/install worktree is uncommitted and unpublished. `git merge-base --is-ancestor` confirmed the first claim; the A22 native worktree remains at `15446` with uncommitted changes. The matrix makes no installed Omarchy or plugin-authority promotion. |

## Coverage and status check

I read the full release 0.1 specification, issue #20, the previous independent review, the review response and the complete matrix. G01–G10 map all ten §7 tests; G11 covers §7.2. G06 correctly separates §7's one-preview-per-variant minimum from §2's three camera views per variant. The §7.1 scenario retains 20 × 35 m, three storeys at 3.2 m, 12 m cap, 3 m clearance, and the 900 m² **soft** GFA target. C01–C09 cover the other required product, interface, security, governance and Omarchy contracts. Required failures are blockers, not optional exclusions; no professional or regulatory acceptance is claimed.

The status labels match the cited scope. At pinned `6536`, the CLI command enum lacks `render`, `export-ifc` and `approve`; MCP `render_variant`, `export_ifc` and `propose_approval` return unavailable. The A18 source demo records exit 2 at render and zero GLB/PNG/IFC. Thus G08, the integrated G09 workflow, C03, C05 and the full demo are correctly failing. G06 and C09 remain untested on actual Omarchy. WSL worker fixtures, Ubuntu CI, development packaging and A22 status review remain partial evidence. The matrix does not infer an unauthorized approval event merely from absent commands.

The matrix has 58 Markdown links, 42 distinct. All pinned source paths resolve at `6536`; all 13 distinct non-source live/external URLs returned HTTP 200 to HEAD requests. Reachability alone does not establish product acceptance. Live GitHub API showed PR #27 still draft, `REVIEW_REQUIRED`, at `6536`; both cited Actions runs completed successfully at `6536`; A01–A23 remain open. Main protection still requires strict `rust` from app 15368 and one approval, with stale dismissal and admin enforcement. The pinned workflow covers locked Rust metadata, format, strict Clippy, workspace tests and schemas on Ubuntu; it does not cover Python lint/tests, Blender/IFC integration or Omarchy. These live facts support the matrix's snapshot, not a future candidate.

**Disposition:** The prior matrix-review issues are closed for this exact documentation commit and patch. Issue #20 and release 0.1 remain open and blocked until a frozen candidate satisfies every required row with retained, independently checked evidence on the named platforms. No candidate, issue, release asset or target worktree was changed in this rereview.
