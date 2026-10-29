# A08 independent camera and preview review, round 1

**Verdict: scoped PASS.** No verified A08 defect in the frozen WSL/CPU source and evidence. This review does not establish Omarchy operation or cross-GPU pixel tolerance.

## Scope and identity

- Worktree `/home/butahlecoq/work/archy/.worktrees/a08-render`, branch `codex/a08-render`, HEAD/base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`. The three author additions were uncommitted: `tests/backend/blender/test_render_presets.py`, `docs/render-reproducibility.md`, and `docs/plans/a08-render-handoff.md`. Worker source had no code change.
- Read `AGENTS.md`, A08 in `docs/plans/backlog.md`, the release specification, handoff, reproducibility note, `workers/blender/{scene,worker}.py`, and the A08 and established Blender tests. This review used the pstack interrogate standard of retaining only run-proven defects, plus direct artifact and observable-behavior checks. The requested no-subagent constraint superseded its fan-out step.
- Frozen source manifest: `/home/butahlecoq/work/archy/.tools/targets/a08-render-evidence/source-manifest.tsv`, SHA-256 `d6af2e03d3254db9b50206b907394d39fc8e25844ba7a8e42d24c14fa8ada3a9`; all 21 listed files matched their byte lengths and SHA-256 in this worktree. It includes worker source, fixtures, A08 test, and reproducibility note, but is a development inventory rather than a release identity.
- Frozen artifact manifest: `/home/butahlecoq/work/archy/.tools/targets/a08-render-evidence/artifact-manifest.tsv`, SHA-256 `bad7cc29344adb14b3708dfdbc3ddc8f8fe05220a7a4146b01a8485c6afa8a61`; all 27 listed files matched their byte lengths and SHA-256. The three `response.json` files had `status: ok`, matching request/project/variant/run IDs, and five artifacts apiece whose recorded hashes and sizes matched disk. Response hashes: `rectangle-wide` `95bb6824e00c9279a9d79b1e99a62ecbdc837458bbece04b2944cccd5dfe60ef`, `l-deep` `0cf1e4c9532422175e98839e59931bb0595f4caab7682694cfd5fa01868343b7`, `l-source` `14c8e4c3b69db23774cc9d70b318bffa0c4aebe90de792a354f9dfda723ccf82`.

## Verification

From the worktree, using the pinned `/home/butahlecoq/work/archy/.tools/blender-4.5.15-linux-x64/blender` and existing reader environment:

```sh
ARCHY_BLENDER_BIN=/home/butahlecoq/work/archy/.tools/blender-4.5.15-linux-x64/blender /home/butahlecoq/work/archy/.tools/reader-venv/bin/python -m unittest discover -s tests/backend/blender -p test_render_presets.py -v
```

Actual: exit 0, `Ran 1 test in 50.947s`, `OK`; its six rectangle/L × wide/deep/tall subcases each launched the real Blender worker with `--offline-mode`. The test's temporary job directories were removed on completion. `blender --version` reported 4.5.15 LTS. The author's earlier full-suite run (16 tests) was documented in the handoff; I did not rerun the full suite.

The A08 assertions cover all footprint vertices at ground and roof height, plus all site polygon corners at ground height, in world coordinates for each of `top`, `front`, and `iso`. Transforming by the recorded inverse camera matrix and checking horizontal/vertical half-frame extents plus near/far clipping is sufficient for these extruded prisms and flat polygonal sites: linear projection reaches its extrema at vertices. Cases are 200 × 2 × 3.2 m, 2 × 200 × 3.2 m, and 2 × 2 × 200 m for both shapes, each with a site extending 2 m beyond the footprint. The worker constructs storeys directly from that ring; its existing world-coordinate parity guard is 1e-6 m. These checks prove framing of the specified geometry, not that the site is drawn.

The test validates three labels, fixed engine/device/samples/seed/resolution and several color/material/light settings through the response protocol. The established `test_repeat_semantic_scene_and_preset` runs source rectangle/L cases twice and compares complete geometry and preset summaries, including camera transforms; it does not claim PNG byte identity. The focused test opens and fully loads each 512 × 512 PNG with Pillow, checks whole-image variation, and counts foreground pixels inside the projected massing rectangle. Thus its smoke check is stronger than nonzero file size, although foreground counts alone cannot prove every face is visually distinguishable. Existing backend tests additionally verify PNG format/frame and artifact hashes.

I independently viewed retained `l-source/view-top.png`, `l-source/view-iso.png`, `l-deep/view-top.png`, and `rectangle-wide/view-iso.png`. The source L notch is visibly resolved in top and iso; the 100:1 L and rectangle are visible as thin complete silhouettes. At that aspect and 512 pixels, the L notch is not resolved in the overview. This is a presentation limit distinct from framing failure. The site affects camera bounds but has no rendered outline. The retained images and source are identified by the two manifests above; the focused rerun used disposable test outputs.

## Findings and limits

No run-proven defect; no failing command/actual/expected pair to report. The source/fixture variant ID reuse in synthetic aspect cases is disclosed in the handoff and confines this evidence to the bounded Blender worker; canonical Rust identity remains outside A08. GPU and target Omarchy runs remain untested. The documented `>8/255` and `>12/255` thresholds are CPU smoke visibility heuristics, not cross-GPU comparison tolerances. No pixel-equivalence tolerance is asserted here.
