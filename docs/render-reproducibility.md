# A08 Blender preview reproducibility

This note describes the bounded `smoke-cpu-v1` preview produced by the existing GPL-3.0-or-later worker. It is WSL development evidence with pinned Blender 4.5.15 LTS. It is not an Omarchy acceptance or a photorealistic presentation preset. The worker code and request/response schema were already published before A08; this slice verified them without changing their behavior.

## Views and framing

Each variant emits exactly three labeled 512 × 512 PNGs: `view-top.png`, `view-front.png`, and `view-iso.png`. The recorded `preset.cameras` contains their world matrices, orthographic scales, and clipping distances. `top` looks down from +Z; `front` looks along +Y from the negative Y side; `iso` looks from +X, −Y, +Z. All cameras aim at the center of the axis-aligned union of the massing bounds and the site polygon bounds. The worker projects the eight corners of that union into camera space and sets the orthographic scale to 1.1 times the larger horizontal or vertical projected extent, with a minimum extent of 1 m. Thus the current square image gives a nominal 10% framing allowance. The near and far planes are recorded per view. Site north angle is retained as metadata; it does not rotate the source geometry or the view presets. The site polygon is included in framing but is not drawn as a site boundary, terrain, or survey model.

The six real Blender test cases use both rectangle and concave L footprints at each of three shapes: wide 200 × 2 m and deep 2 × 200 m, each 3.2 m high, plus tall 2 × 2 m and 200 m high. Each site extends 2 m beyond the footprint on every side. The test projects every footprint vertex at ground and roof height and every site corner at ground height through each recorded camera matrix. It requires all points inside the 512 × 512 orthographic frame and between recorded clipping planes. This is camera-space coverage, independent of PNG color. The source L fixture and existing repeat test also exercise translated geometry, three storeys, and a nonzero north-angle metadata value. The 1e-6 m world-coordinate budget is the pre-existing Blender geometry parity check; it is not a render-pixel tolerance.

## Recorded render preset

| Setting | `smoke-cpu-v1` value |
| --- | --- |
| Runtime | Official Blender 4.5.15 LTS Linux x64, bundled Python 3.11.15, worker protocol `blender-worker-v1` |
| Renderer | Cycles CPU, one fixed thread, 16 samples, adaptive sampling off, denoising off, seed 42, frame 1 |
| Image | 512 × 512, 100% resolution, PNG RGBA 8-bit, compression 15, opaque film |
| Color | sRGB display, Standard view transform, Medium High Contrast look, exposure 0, gamma 1 |
| World | Node background RGBA (0.12, 0.15, 0.2, 1), strength 0.6 |
| Massing | Node Principled BSDF base/diffuse RGBA (0.3, 0.55, 0.8, 1), roughness 0.8 |
| Light | One white disk area light, energy 3000, size 10 m; actual transform recorded per scene |
| Export | Only the storey mass objects in GLB, Blender Z-up to glTF Y-up; cameras and lights excluded |

`response.json` records the actual values, not only preset labels. It includes all three camera matrices/scales/clips, object and light transforms, unit scale, material/world node values, render color settings, and source variant digest. The existing two-run rectangle/L regression compares the full geometry and preset summaries. The new A08 test checks the fixed settings and camera-space coverage across all six aspect cases.

## Render evidence and limits

Real pinned Blender output was decoded by Pillow 13.0.0 with NumPy 2.5.4 in the established CPython 3.12.13 development reader environment. Every tested view is a 512 × 512 PNG with visible color variation, and the new test also requires pixels distinct from the background inside the projected massing footprint. Its `>8/255` whole-image and `>12/255` projected-region differences with more than 32 pixels are **CPU smoke visibility checks**, not perceptual quality scores or image-equivalence tolerances. Each worker invocation has a 120-second timeout; each five-artifact job is checked below 64 MiB. The six-case focused test passed in 25.778 seconds in this WSL environment. The full Blender backend suite, including the existing independent GLB/Pillow checks and this A08 case, passed 16 tests in 104.323 seconds before the final projected-region check was added; the final focused A08 check passed after that addition.

Retained development runs are under `/home/butahlecoq/work/archy/.tools/targets/a08-render-evidence/`. `rectangle-wide` and `l-deep` use test-only variants that preserve a fixture ID while changing parameters; they prove bounded worker rendering, not canonical Rust identity. `l-source` uses the unchanged generated source L fixture. All three jobs exited 0 with `status: ok`, five listed artifacts, Blender 4.5.15 LTS, Cycles CPU, 16 samples, and the three labeled views. Their artifact hashes and source-file hashes are in the A08 handoff and its manifests.

Visual inspection of actual PNGs found the source L notch clearly visible from top and iso; front shows a centered mass silhouette. The 100:1 rectangle and L remain inside the frame and visible as thin top/iso silhouettes, but the extreme L notch is not individually resolved at 512 pixels. This is an expected presentation limit of the fixed overview view, not evidence of changed source geometry; inspect GLB or a closer view for that detail. No site outline is drawn.

**Pixel comparison across GPUs is unvalidated.** No cross-GPU pixel tolerance, byte-identity claim, or quality threshold is set. Establish one only after matched fixtures are rendered on at least the pinned CPU baseline and target GPUs/drivers and their measured pixel differences are reviewed. Exact parametric manifests and analytic geometry have separate reproducibility criteria.

## Dependency and license rationale

The [backend research](plans/backend-research.md) selected the official Blender 4.5.15 LTS archive for its supported 4.5 API line and bundled Python, avoiding a separate `bpy` install. [ADR 0003](adr/0003-blender-runtime.md) and [the worker handoff](plans/blender-worker-handoff.md) record archive SHA-256 `0d2b8bb2efe97e6c8eb57de6b7322f8800e1984e3125b60decca5f6771e4773e`, Blender build hash `5afeabba6097`, and the GPL-3.0-or-later worker boundary. The Blender executable remains outside this repository. [Artifact reader research](research/artifact-readers.md) chose Pillow for full PNG decoding, NumPy for pixel/camera-space arrays, and trimesh for independent GLB world-transform inspection. The existing hash-locked reader environment uses Pillow 13.0.0, NumPy 2.5.4, and trimesh 5.2.0; these are development-test dependencies and are not shipped with the worker. No software was installed in this A08 slice. Preserve the worker's GPL license and Blender's separate distribution notices when packaging later.
