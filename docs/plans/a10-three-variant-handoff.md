# A10 three variant WSL parity handoff

Date: 2026-10-29. Base: `8e76c4af76b5c4cc834bbfe16e1140f4a91695db`, worktree `codex/a10-three-variant-proof`. This is development fixture evidence, not an accepted production render run or Omarchy result.

## Plan and gap assessment

The existing Blender integration tests already render analytic rectangle/L fixtures, decode PNG pixels, parse transformed GLB instances, verify storey volumes and reopen `.blend`. A08 additionally checks camera framing. A05 has a saved deterministic three variant CLI workflow. They do not bind **fresh CLI generated and evaluated variants** to three **independently parsed** exports in one retained run, or retain a partial render failure beside independent design checks. A09's production render supervisor and publication gate are absent.

Plan executed: (1) build the existing CLI offline; (2) copy the courtyard source to a private disposable project and run real `generate` and `evaluate --all`; (3) pass each saved variant through the pinned worker as a test fixture; (4) read each GLB in fresh external CPython with pinned trimesh and compare its transformed vertices, roof caps, volumes and bounds to the saved core evaluation; (5) run a second project with one injected worker output collision; (6) retain snapshots, logs and SHA-256 manifests. No product CLI, Rust schema, Cargo, worker or shared repository files changed. No new dependency was installed. Pillow/NumPy/trimesh are the already pinned optional external reader from `docs/research/artifact-readers.md`; Blender's bundled Python remains the worker runtime.

## Exact execution and evidence

```sh
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo build --locked -p archy-cli
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python tests/backend/blender/three_variant_proof.py --slot 3
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python tests/backend/blender/three_variant_proof.py --slot 4 --failure-index 1
```

All three commands exited 0. The CLI build finished in 49.24 s. Slot 3 has three `fixture_verified` variants. Slot 4 has two `fixture_verified` variants and one `failed_collision_preserved` variant, with all three separate design evaluation records readable. The failing worker exited nonzero on an existing `model.glb` sentinel before response creation; the sentinel bytes remained unchanged. Its stderr, exit code, request and independent evaluation are retained. Neither slot is publication by a supervisor.

Evidence root: `/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof/`. `slot-3/project/` and `slot-4/project/` contain real CLI run snapshots, reports, immutable variant/evaluation files and logs. Each slot has `proof.json`, a SHA-256/length `artifact-manifest.tsv`, `source-manifest.tsv`, a `source-snapshot/`, worker requests/responses, bounded stdout/stderr, GLBs and PNGs. The harness refuses an occupied slot and accepts only slot 0–9 and failure index 0–2. The slot 3 generation run is `99051459-1d62-4237-a8f3-64ec56ac07bc`; evaluate-all run is `0e6e42f9-deb9-48a5-8b41-aa540bff873f`.

| Slot 3 inventory | SHA-256 |
| --- | --- |
| Source manifest, six frozen source files | `7a05db81a8c8c8a82d443aa4a0b27711386044f273886a90819fae27d7c2b759` |
| Artifact manifest, 69 retained files | `2d88e9566f0e8fad3feed37e6bfe8e88586070481769262f279df32a7f1217e4` |

Pinned runtime observed: Blender 4.5.15 LTS, external trimesh 5.2.0, Pillow 13.0.0, NumPy 2.5.4, CPython 3.12.13. CLI executable SHA-256 `a84886871501ef034aa34ebe78c92739cf6a1ca6909a8ea2025fc6eed3b298f4`. The retained source manifest binds the actual worker, protocol, scene code, input brief, harness and Cargo lock. The prior published Blender archive pin is in `docs/plans/blender-worker-handoff.md`.

## Portability correction and scoped rereview

The original `ROOT.parents[1]` calculation was reproduced without writes for both actual roots. It selected `/home/butahlecoq/work/.tools/blender-4.5.15-linux-x64/blender` for the ordinary `/home/butahlecoq/work/archy/repository` checkout, where that file does not exist. The linked worktree selected the existing `/home/butahlecoq/work/archy/.tools/blender-4.5.15-linux-x64/blender`. Both share the Git common directory `/home/butahlecoq/work/archy/repository/.git`.

The fixture now anchors defaults to that common checkout: it chooses its `.tools` directory when present, otherwise a sibling `.tools` when present, otherwise its own `.tools` as the default. Each checkout keeps its own `target/debug/archy` CLI default. For unusual development layouts only, absolute nonempty `ARCHY_A10_TOOLS_DIR`, `ARCHY_A10_EVIDENCE_DIR`, and `ARCHY_A10_CLI` environment variables override those test fixture paths. `ARCHY_A10_TOOLS_DIR` names the directory containing `blender-4.5.15-linux-x64/` and, unless independently overridden, `targets/a10-three-variant-proof/`. **The selected CLI and Blender executables must be trusted: they run with the invoking developer's permissions, and this test fixture does not sandbox them.** These are test harness settings and do not add product runtime paths. Git's common-directory lookup and pure path resolution perform no writes. No actual workflow was run from the ordinary checkout.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/backend/blender -p test_a10_paths.py -v
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python tests/backend/blender/three_variant_proof.py --slot 7 --failure-index 1
```

The four focused tests passed with system Python and no optional reader imports. Direct read-only resolution for both actual checkouts selected the pinned Blender and shared evidence root, with checkout-local CLI paths. Slot 7 exited 0, with two verified variants, the injected collision preserved, and all three evaluations readable. A second slot 7 invocation exited 1 at `mkdir` with `FileExistsError`; the occupied evidence was unchanged. Slot 6 is an earlier successful collision run before the final import adjustment. Slots 3, 4 and 5 retain their original manifests and source snapshots, and their contents were rechecked. Their old source hashes identify the earlier harness, not the corrected source.

| Final source evidence in slot 7 | SHA-256 |
| --- | --- |
| Source manifest, seven frozen files including path tests | `10b8ae37c4c327041c87f35ec02653d1da049bda04ee6ca29b671145e99c6112` |
| Artifact manifest, 64 retained files | `5175d28ffc719e04835780dafa9d8c86f3e6e4e2f3df9fdb74a1d8322b8786c3` |
| `proof.json` | `da1b2ca850e67e64ba1a411881b36e3d38abbec506bbb6ccc097be7d2ceeab70` |

Slot 7 generation/evaluation run IDs are `d1c4b5e1-752c-44b2-bb94-6a78ae53ec75` and `eb971add-6f8d-426c-8b5a-3fa1043559b0`. The source snapshot matches the current seven listed files; the artifact manifest covers the project and job files, while the source manifest covers the source snapshots separately.

**Scoped rereview request for Lovelace:** inspect the common-directory resolution for ordinary and linked checkouts, the test-only overrides and occupied-slot refusal, then verify slot 7's source/artifact manifests and collision result. Compare slots 3–5 only against their retained snapshots. The prior [independent review](a10-three-variant-review.md) passed the earlier WSL fixture; this correction needs its own review. This session has no independent reviewer dispatch tool and the task prohibits child agents, so the request is recorded here for routing and no rereview verdict is claimed.

## Evidence-root symlink correction

The appended disposition in [round 2 review](a10-three-variant-review-round2.md) accepts the trusted-executable boundary and identifies the evidence-root symlink case. The former harness was reproduced with a disposable root symlink: it created `slot-0` in the symlink target before the selected `/bin/false` CLI failed. The fixture now refuses a final-component evidence-root symlink before creating a slot, including a dangling symlink. It still accepts an existing real evidence directory and refuses an occupied slot or a symlink at the slot itself. The check does not establish a race-free sandbox for parent components or concurrent path replacement.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/backend/blender -p test_a10_paths.py -v
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python tests/backend/blender/three_variant_proof.py --slot 9
```

The six focused tests passed, including a disposable symlink target with an unchanged sentinel, a dangling link, a real directory and occupied slot. A separate harness invocation with the evidence-root override set to a disposable symlink exited 1 with `evidence root must not be a symlink`; its target still contained only the unchanged sentinel. Slot 9 exited 0 with three `fixture_verified` variants and three readable design evaluations. Slots 3–8 were checked against their own manifests and source snapshots; their recorded manifest hashes remain unchanged.

| Slot 9 final source evidence | SHA-256 |
| --- | --- |
| Source manifest, seven frozen files | `3826dd2b93dea2bd20a5eb09bc63451c126bfcff25945fbe30b2f063eec1068c` |
| Artifact manifest, 69 retained files | `378f8d59c0ba4de8be05b0d78bc36d3d07b7c78e5edbe6e3aa9dbb69df581f8a` |
| `proof.json` | `41c07f1d747007825dddc0332d48fc22ecb0aac60765f58cbe35e667184ba488` |

Slot 9 generation/evaluation IDs are `41b113a5-6caf-4293-a399-a3425601357a` and `2b6e46e3-6bf6-4b04-83b3-8c74ce2a52dc`. Its source snapshot matches the current seven source files; its artifact inventory is separate from the source inventory. The portability correction and slot 7 evidence passed the scoped geometry/path portion of round 2 review. The current symlink correction, trust sentence, tests and slot 9 evidence are frozen for a further scoped independent rereview; no verdict on those changes is claimed.

## Independently measured geometry

The GLB reader preflights binary size/JSON chunks, rejects external buffers/images and unsupported required extensions, uses trimesh with `process=False`, and applies each scene node's world transform before the inverse Blender Z-up conversion. It derives bounds from **all transformed vertices**. An exact-position welded topology copy accounts for glTF normal seams; the original export remains untouched. Every solid must be closed, consistently wound and outward. Every original footprint corner must appear at each storey elevation, every transformed vertex must lie within the source polygon, and roof triangle centroids must remain inside the L footprint so a filled notch fails. Roof cap area, summed GFA and per-storey signed volume are compared to saved core metrics. All nine PNGs decoded at 512×512 with visible foreground; the exact per-view counts are in `proof.json`.

| Variant ID suffix | Shape | Parsed world bounds `[xmin,ymin,zmin,xmax,ymax,zmax]` m | Roof area m² | Total volume m³ | Max XY bound error m |
| --- | --- | --- | ---: | ---: | ---: |
| `3902dc3b` | L | `[5.052881876,6.268478967,0,13.472765446,31.309940815,9.600000143]` | 191.900561446 | 1842.245417331 | 5.112e-7 |
| `8594779d` | L | `[5.519766686,4.592456390,0,15.282759666,31.666032314,9.600000143]` | 230.898864140 | 2216.629128770 | 7.362e-7 |
| `efd505ff` | L | `[4.281063549,5.620833773,0,11.657241344,30.072162151,9.600000143]` | 136.239068772 | 1307.895079699 | 1.431e-7 |

The saved evaluation's coordinate budget is exactly **1e-6 m** and was not widened. The saved per-variant area budgets are about 6.37e-5–7.37e-5 m², and GFA budgets about 1.91e-4–2.21e-4 m²; the harness uses each actual report's value. The volume comparison derives a dimensioned per-storey limit from that area budget × 3.2 m plus `2 × area × 1e-6 m`. The largest observed absolute roof area error was 9.912e-6 m²; largest total volume error was 8.499e-5 m³. Float32 export is visible at 9.6 m becoming 9.600000143 m and sub-micrometre XY rounding. This policy works only where the exported world coordinates can meet 1e-6 m: the worker's existing far-origin capacity test returns `BACKEND_UNAVAILABLE` for unrepresentable coordinates; it does not silently relax the budget. This fixture's coordinates fall in the tested representable range.

The seed 42 run happened to produce three L variants. Existing `test_blender_integration.py` covers rectangle and translated L with three storeys and independent GLB reading; those tests were inspected and not rerun for this slice. The new harness demonstrates three actual generated alternatives, not a rectangle in the same seed 42 set.

## Failed checks and acceptance boundary

Slot 0 is retained as an initial harness failure: NumPy 2.5 rejected 2D `np.cross` in the roof calculation. After replacing it with an explicit 2D determinant, slots 1 and 3 passed; slot 2 and final slot 4 passed the partial failure scenario. These failed findings were retained rather than folded into render success. The proof keeps `design_checks` and `render_status` as independent fields for each variant; the synthetic collision never changes evaluation results.

A10 full acceptance remains **partial**: there is no production `archy render --all` supervisor, accepted render manifest, timeout/cancellation and atomic publication gate from A09; this fixture launches trusted worker requests directly. The target Omarchy run and visual release gate are deferred. This proof is a WSL CPU geometry and preview fixture only. The prior independent review passed the earlier fixture; a scoped independent rereview of the portability correction is pending. Do not mark A10 accepted or infer professional compliance from these checks.
