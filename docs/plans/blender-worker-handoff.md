# Blender worker development handoff

Date: 2026-10-29. Scope: `workers/blender/**`, `tests/backend/blender/**` and
ADR 0003. This is a bounded WSL development slice, not A06/A07 closure.

## Delivered

- Official Blender 4.5.15 Linux x64 archive downloaded outside the repository
  to `.tools/blender-4.5.15-linux-x64.tar.xz`. The actual SHA-256 matched the
  official checksum: `0d2b8bb2efe97e6c8eb57de6b7322f8800e1984e3125b60decca5f6771e4773e`.
  The archive was extracted only under `.tools`. Bundled `copyright.txt` and
  `license/license.md` were inspected. No Blender binary is tracked.
- `--version` reports Blender 4.5.15 LTS, build hash `5afeabba6097`, built
  2026-10-22. Pinned runtime probe reports bundled Python 3.11.15, glibc 2.43
  on WSL2 Linux 6.6.87.2 x86_64 and an available bundled glTF exporter
  `io_scene_gltf2` version 4.5.51 (archive `__init__.py` `bl_info`). All
  proposed fixed launch flags appeared in the pinned binary's `--help`.
- Stdlib protocol rejects duplicate/unknown JSON fields, nonfinite and
  overlarge requests, malformed rectangle/L footprint snapshots, bad
  associations, unsupported capacity, invalid IDs and logical path traversal.
  Response validation requires the five fixed relative output names, hashes,
  sizes and media types. Worker errors never list partial artifacts as accepted.
- The trusted worker constructs one closed prism per storey, checks cap
  coverage, manifold edges, mesh validation and positive signed volume, exports
  selected massing to GLB, saves `.blend` and renders fixed 512×512 Cycles CPU
  top/front/iso views. Site north angle is metadata; geometry is not rotated.
  Scene framing includes site bounds. No production render CLI is included.
- WSL tests use Task02's read-only analytic rectangle 200 m² / 600 m² GFA and
  L 75 m² / 225 m² GFA, both with three 3.2 m storeys. They decode all PNG
  pixels, parse GLB through trimesh in separate CPython, check transformed
  watertight volumes/bounds to the 1e-6 m coordinate budget, and reopen `.blend`
  in a fresh Blender process. A source-generated translated L variant and
  nonzero north metadata also run. Exact-position welding on a test copy joins
  glTF normal seams before topology checks; exported bytes are not repaired.
- Reader pins came from `docs/research/artifact-readers.md`. A separate
  CPython 3.12.13 environment uses the hash-locked Pillow 13.0.0,
  trimesh 5.2.0 and NumPy 2.5.4 in
  `tests/backend/blender/requirements-cp312-linux-x64.lock`. The lock is
  development-test only; the worker uses Blender's bundled Python. Reader
  licenses observed: Pillow MIT-CMU, trimesh MIT, NumPy expression
  `BSD-3-Clause AND 0BSD AND MIT AND Zlib AND CC0-1.0`.

## Commands and observed evidence

```sh
curl -fL --retry 3 -C - -o /home/butahlecoq/work/archy/.tools/blender-4.5.15-linux-x64.tar.xz https://download.blender.org/release/Blender4.5/blender-4.5.15-linux-x64.tar.xz
sha256sum /home/butahlecoq/work/archy/.tools/blender-4.5.15-linux-x64.tar.xz
# 0d2b8bb2efe97e6c8eb57de6b7322f8800e1984e3125b60decca5f6771e4773e
uv pip sync tests/backend/blender/requirements-cp312-linux-x64.lock --python /home/butahlecoq/work/archy/.tools/reader-verify/bin/python --require-hashes --reinstall
# exit 0; numpy 2.5.4, pillow 13.0.0, trimesh 5.2.0 installed
ARCHY_BLENDER_BIN=/home/butahlecoq/work/archy/.tools/blender-4.5.15-linux-x64/blender /home/butahlecoq/work/archy/.tools/reader-verify/bin/python -m unittest discover -s tests/backend/blender -v
```

The tested Blender command uses `--background --factory-startup
--disable-autoexec --offline-mode --python-exit-code 20 --threads 1 --python
workers/blender/worker.py -- --request ... --response ...`. Final full suite:
**12 tests, 55.143 s, exit 0, OK**. Executed negatives include duplicate or
unknown JSON fields, malformed/oversized/nonfinite input, bad association and
world bounds, footprint/notch/storey/preset/path values, missing/traversing
response artifacts, real output collision, response symlink and traversal with
unchanged sentinels. The repeat test ran each analytic shape twice and compared
scene geometry, preset and GLB vertex/face counts; it did not assert image byte
identity across hardware.

The final-source retained WSL smoke is at
`/home/butahlecoq/work/archy/.tools/blender-final/` (untracked). The worker
command exited 0, response status was `ok` with five entries, and fresh-process
`.blend` reopen exited 0 with `ARCHY_REOPEN_OK 3`. Output `sha256sum` values:

| Artifact | SHA-256 |
| --- | --- |
| `scene.blend` | `da281f76d814301b0f93a650be7b88b5ee69f6f57bc873922a766f5f0908b348` |
| `model.glb` | `8711ea166746fceda41dfb4016bf1586bd2a6104df06262c39789aef82b96cde` |
| `view-top.png` | `de4d338e318d152f5274f91d7a3ab909dc6523808622867eb30cf7eea66e6afa` |
| `view-front.png` | `59e021a0aec00d4db05c67c134dd19e6b2ea0408f6838128fb335514c5d9b93d` |
| `view-iso.png` | `6fd33b8057165e0493b371161f9433cc94028a9fceaa2e8870738d4ae5ea5e2d` |

Observed development failures retained for diagnosis: the pinned
`tessellate_polygon` returned integer triangle indices (the first attempt
expected Vector objects); an empty factory scene had no world; the initial
Cycles material/world produced nearly white objects on a white background;
independent GLB indexing reported open seams until exact-position welding on
the test copy. The Task02 generated L cap area also exposed float32 local
coordinate rounding; cap coverage now uses the perimeter-propagated 1e-6 m
coordinate budget. The original strict 1% foreground heuristic failed on the
washed-out images, then passed after explicit shader/world values. These were
real WSL failures, not silent threshold widening. A fresh-process reopen also
found an absolute private render path embedded in the saved scene; the worker
now sets a relative `//view-iso.png` path before saving.

Visual inspection of `.tools/blender-visual-2` found the L notch visible from
top and iso, the full mass inside frame and a distinguishable background; the
front view shows a centered dark mass silhouette. A coordinator independently
observed the top/iso images and noted bright/noisy roof lighting consistent
with the low-sample smoke preset. These are WSL preview observations only.
Those files are untracked development artifacts, not accepted project runs.

## Integration and acceptance still outstanding

- Rust protocol schema/serializer, canonical variant ID and project binding,
  evaluation recomputation and A09 response authority have not been implemented
  in this slice. A structurally valid 64-hex variant ID is not Python proof of
  canonical identity. The analytic backend test changes parameters while
  retaining a fixture ID solely to probe geometry; Rust must supply valid IDs
  in actual integration.
- A09 must enforce private mounted staging, no-follow descriptor-based output
  verification, process-tree timeout/cancellation, memory/output/log limits and
  immutable publication. Python path checks and Blender startup flags are not
  filesystem or network isolation. Exit zero, worker hashes and previews do
  not grant approval or design feasibility.
- Exact `[10,-5]` translated canonical variants, full malformed fixture
  inventory, render differences across repeated jobs, target visual thresholds,
  and full independent GLB conformance checks remain for integration/review.
  The reader tests cover only this fixed GLB profile. No tolerance was widened
  beyond Task02's 1e-6 m coordinate budget; volume comparisons use the analytic
  3-storey fixture and a separately dimensioned 1e-4 m³ test assertion.
- Actual Omarchy install/execution, hardware/loader/GPU evidence and clean
  target release smoke are deferred. A06 target acceptance, A07 Rust integration,
  A08 visual quality and A09 artifact authority remain open. Independent review
  and distribution/license obligations remain with the coordinator.

## Round 1 after independent Blender review

`docs/plans/blender-review-final.md` remains unchanged. Its three findings were
addressed in this worker slice. Blender mesh positions and object transforms
are float32: a valid input origin of `9990.12345` m had previously become
`9990.123046875` m in world space (error `0.000403125` m). The worker now
compares every actual world vertex of every storey against each canonical
rectangle/L point and expected lower/upper elevation before export. The
1e-6 m Task02 coordinate budget is unchanged. An unrepresentable design
returns `BACKEND_UNAVAILABLE` with measured error and actionable coordinate
guidance; exactly representable far coordinates remain supported.

Success response validation now requires exact nested geometry and preset
fields, rejects unknown/nonfinite values, checks positive bounds/area/volume
and the recorded world-error budget, and freezes the CPU engine/device,
samples, resolution, color management and GLB flags. Provenance includes the
chosen thread mode, image compression, transparency, unit/frame, world node
color/strength, material node color/roughness, light type/color/transform,
all camera matrices/scales/clips, object transforms and saved render path.
The repeat test compares this entire preset and geometry summary.

**Red evidence before fixes:** the new positive and negative `9990.12345` m
regressions each failed because the worker exited 0 instead of the required 20.
The nested response test failed ten subcases because changed engine/device,
nonfinite geometry/world values, unknown fields and changed export/material
settings were accepted. After strict validation was added, a real source
render exited 20 because its older response lacked the required preset fields;
recording the actual Blender settings resolved that failure.

**Final WSL validation:**

```sh
ARCHY_BLENDER_BIN=/home/butahlecoq/work/archy/.tools/blender-4.5.15-linux-x64/blender /home/butahlecoq/work/archy/.tools/reader-verify/bin/python -m unittest discover -s tests/backend/blender -v
# Ran 15 tests in 54.220s; OK; exit 0
```

The suite includes actual negative/collision tests, independent Pillow PNG
decoding and trimesh GLB vertex/world-transform checks, positive and negative
far-coordinate translations, rectangle/L caps and storey elevations, saved
scene reopen, and two-run semantic/preset comparison for both analytic shapes.
No render quality threshold or cross-hardware pixel identity was added.

Retained untracked jobs are under
`/home/butahlecoq/work/archy/.tools/blender-review-round1/`:

- `inexact/`: fixed worker argv exited 20; `response.json` reports
  `BACKEND_UNAVAILABLE`, `params.footprint_xy_m`, measured error
  `0.000403124999 m` greater than `1e-6 m`, and `artifacts: []`.
  The directory contains only request and response files.
- `exact/`: fixed worker argv exited 0; response status `ok` with five files
  and recorded maximum world-coordinate error `0.0 m`. A separate CPython
  3.12.13/trimesh process measured inverse-axis-converted GLB bounds
  `[9990.125, -5.125, 0]` to `[9992.125, -2.125, 9.75]`. Pillow opened three
  512×512 PNGs. Fresh Blender `.blend` reopen exited 0 with
  `ARCHY_REOPEN_OK 3`. The five output SHA-256 values are:

| Artifact | SHA-256 |
| --- | --- |
| `scene.blend` | `6aa8894ed15e4f74a386f56a52ab08ba54c80cbfbf0cc328323d4df70c6ea974` |
| `model.glb` | `3d03c4476f1285b5d58b239e328a3abbee0b50a08846d0a0354075e32ecab59f` |
| `view-top.png` | `aaec4a610f177a7988c491140fdbe89ec24202f15e2f64a67fb8c1a988d00171` |
| `view-front.png` | `6a25ae714c1a18367fd8f8edc38e200d65e693b32e6d05fbee37983da9a7a73a` |
| `view-iso.png` | `1b691ab80aabcbf5a519965ba6b27c7a96ce103f5ca6f73b07de90bf9cd1aea6` |

This closes the three review findings for the bounded Python/WSL slice. Rust
canonical identity/evaluation, independent A09 acceptance and actual Omarchy
execution remain separate integration gates.
