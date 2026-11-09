# A06/A07 Blender worker implementation brief

Research date: 29 October 2026. Status: implementation brief, not backend acceptance.
Basis: `AGENTS.md`, [release specification](../specs/release-0.1.txt), [A06–A09 backlog](backlog.md), [existing backend research](backend-research.md), [Task02 handoff](task-02-handoff.md), and [A05 plan](task-03.md).
Only this document is changed by this research task; no code, installation, agents, commits or pushes.

## Deliverable and ownership

Implement a trusted packaged worker accepting one validated rectangle/L variant per invocation and producing an editable `.blend`, one GLB and three labeled PNG previews in private staging.
Rust owns canonical identity, evaluation, authority and eventual artifact acceptance; Blender owns derived scene construction and export.
A06 owns runtime provenance and smoke evidence; A07 owns typed requests and scene parity.
The requested three-view smoke preset exercises a narrow A08/A09 interface in development; it does not close their broader rendering, isolation or artifact-acceptance tickets.
A09 owns production CLI render orchestration, process-tree cancellation/isolation, bounded logging and final artifact verification/publication. A05 owns immutable storage.
Do not expose a general Blender MCP bridge, arbitrary scripts, node graphs, expressions, imports, external assets or executable paths in requests.
Actual Omarchy execution remains deferred; successful WSL development checks cannot close A06's target acceptance gate.

## Runtime candidate: available, not yet tested

The official [4.5 download index](https://download.blender.org/release/Blender4.5/) was fetched successfully with curl and lists:

| Field | Observed value |
| --- | --- |
| Candidate | Blender 4.5.15, Linux x64 |
| Archive | `blender-4.5.15-linux-x64.tar.xz` |
| Official URL | https://download.blender.org/release/Blender4.5/blender-4.5.15-linux-x64.tar.xz |
| Index timestamp / size | 22-Oct-2026 08:09 / 378036492 bytes |
| Published SHA-256 | `0d2b8bb2efe97e6c8eb57de6b7322f8800e1984e3125b60decca5f6771e4773e` |
| Checksum source | [official blender-4.5.15.sha256](https://download.blender.org/release/Blender4.5/blender-4.5.15.sha256) |

The checksum text was retrieved directly; archive bytes were not downloaded or hashed in this task. Availability and published checksum are verified, binary integrity/runtime compatibility are not.
The official [4.5 LTS page](https://www.blender.org/download/lts/4-5/) was accessible. No newer-release, support-end-date or GPU-compatibility claim is needed for this bounded choice.
Use bundled Python only. Its exact version, Blender build hash, exporter implementation version, glibc/loader requirements and installed GPU support remain unknown until the actual archive is inspected/run.
Record those observed values in `workers/blender/versions.json`; never copy the separate IFC Python pin into this worker.
The web tool returned HTTP 402 for initial Blender URLs; curl subsequently retrieved the official index, checksum and API/manual pages successfully. No unresolved network blocker prevents this brief.

## Reuse evidence and API choices

| Official source | Implementation decision |
| --- | --- |
| [Mesh API](https://docs.blender.org/api/4.5/bpy.types.Mesh.html) | Use `bpy.data.meshes.new`, `Mesh.from_pydata` and data-block objects/collections instead of context-sensitive modeling operators. `from_pydata` accepts explicit vertices/faces; invalid mesh repair must not silently change source geometry. |
| [mathutils.geometry](https://docs.blender.org/api/4.5/mathutils.geometry.html) | Reuse `tessellate_polygon` for concave L caps, not a custom tessellator or convex fan. Omit the repeated closure point when passing the ring; degenerate consecutive points are unsupported. Confirm the exact return representation in the pinned runtime before indexing triangles. |
| [glTF operator](https://docs.blender.org/api/4.5/bpy.ops.export_scene.html), [glTF manual](https://docs.blender.org/manual/en/4.5/addons/import_export/scene_gltf2.html) | Reuse bundled glTF exporter: `bpy.ops.export_scene.gltf`, GLB format, selected massing only, `export_yup=True`, animations/cameras/lights/compression disabled. Verify operator availability and enum values via runtime RNA; do not download an extension automatically. |
| [Camera API](https://docs.blender.org/api/4.5/bpy.types.Camera.html), [render operators](https://docs.blender.org/api/4.5/bpy.ops.render.html) | Orthographic cameras with explicit `ortho_scale`; use background `bpy.ops.render.render(write_still=True)`, not viewport OpenGL capture. Set camera and output path for each image. |
| [window-manager operators](https://docs.blender.org/api/4.5/bpy.ops.wm.html) | Save the derived scene with `save_as_mainfile`; reopen only that trusted generated file in a fresh smoke process. No user-supplied `.blend` input. |
| [4.5 CLI arguments](https://docs.blender.org/manual/en/4.5/advanced/command_line/arguments.html) | Fixed argv supports background/factory startup, disabled autoexec, offline preference and nonzero Python exception exit. Flags are not an OS sandbox; verify against the pinned binary's `--help`. |

Version-family API documentation is mechanism evidence, not proof that every keyword/property works in the exact archive. The first smoke probe resolves that bounded uncertainty; fail explicitly on missing APIs rather than silently changing settings.

## Typed protocol and trust boundary

Create versioned strict request/response schemas shared by Rust fixtures and Python validation. Reject duplicate JSON keys, unknown fields, NaN/infinity, unsupported versions and invalid IDs before constructing a scene.
Request: `schema_version`, fixed operation `build_massing`, `request_id`, `project_id`, `variant_id`, `run_id`, `params` (complete canonical Task02 Variant), normalized site, input digest, and preset ID `smoke-cpu-v1`.
Include `output_dir` as a project-relative logical destination required by the backend contract, but never use it as ambient authority: A09 maps it to a fresh private mounted/staged work directory; worker outputs fixed relative basenames there.
Rust validates canonical parameters/footprint/ID/project association and recomputes evaluation before dispatch. Stored `required_checks_passed` and worker claims cannot grant approval or prove geometry integrity.
Python repeats bounded structural/finite-value checks and verifies the supplied footprint agrees with the rectangle/L parameters. It does not invent a second variant identity algorithm; Rust remains responsible for identity validation.
Required FAIL/WARN/UNKNOWN design checks remain attached to the source evidence. A successful render means only successful artifact production, never feasibility, compliance or approval.
Initial worker limits (proposed operational policy): request <=1 MiB, one variant, exactly 5/7 closed footprint points, 1..100 storeys, coordinates/heights with absolute value <=10000 m, positive dimensions, supported preset only.
These are worker capacity limits, not new Task02 schema rules: valid core designs outside them get an actionable unsupported-backend-capacity failure; never clamp or silently simplify geometry.
Response: schema/request/project/variant/run IDs, request digest, worker version, Blender/build/Python versions, status, warnings, fixed preset settings, scene geometry summary, and produced `{path, sha256, media_type, producer, size_bytes}` entries.
Success requires all five outputs: `scene.blend`, `model.glb`, `view-top.png`, `view-front.png`, `view-iso.png`. Errors carry the existing code/message/field/retriable envelope and cannot list incomplete files as accepted artifacts.
Write a single response file last; Blender stdout is a diagnostic stream, never the JSON transport. Response paths are relative; reject absolute paths, traversal, symlinks, extras and collisions.

## Geometry and deterministic smoke preset

1. Clear factory scene data; use deterministic collection/object/material names derived from validated IDs and storey indices. Metres, right-handed Z-up, unit scale 1, frame 1, no animation/modifiers or external dependencies.
2. Respect Task02's CCW rectangle and upper-right-notched L conventions. Remove only the duplicated endpoint for mesh assembly; retain original canonical snapshot unchanged.
3. Create one closed prism per storey, at elevations `i * floor_to_floor_m` and `(i+1) * floor_to_floor_m`; triangulate caps, reverse bottom winding, and join corresponding perimeter edges with consistently outward side faces.
4. Preserve site translation and north-angle metadata; do not rotate geometry just because north is nonzero. Build vertices near the variant origin with an explicit translation transform to reduce float precision loss; validate resulting world coordinates, not just local bounds.
5. Check cap coverage, normals, closed edges and positive signed volume. If mesh validation reports repairs, fail the smoke instead of accepting a changed design. Do not fill the L notch or model separate overlapping boxes as a shortcut.
6. Export massing objects only to GLB; exclude site/cameras/lights. Record axis conversion (Blender Z-up to glTF Y-up), all object transforms, export flags and metre scale. Reconstruct world geometry with the inverse axis conversion for comparison.
7. Smoke preset proposal: Cycles CPU, one thread, seed 42, 16 samples, adaptive sampling and denoising off, 512x512 at 100%, PNG RGBA 8-bit, fixed world/light/material values, Standard view transform and explicit exposure/gamma. Freeze exact values in the preset fixture; unsupported settings fail.
8. Three orthographic views: top, front and isometric. Aim at the union of site and massing bounds; project all bounding corners into camera space, fit both image dimensions with 10% margin, and set clipping distances to encompass them. Guard top-view orientation singularities explicitly.
9. Record camera matrices/scale, light transforms/energy, material values, color-management enums and render settings in response provenance. Names, topology and settings repeat exactly; PNG/GLB byte identity across hardware is not promised.
10. Use Task02's recorded coordinate/parity policy for geometric comparison: 1e-6 m coordinate budget and propagated area/GFA bounds, not a new universal area epsilon. Backend float precision may exceed it: report failure and investigate local-coordinate/export handling; do not widen tolerances silently.

## Launch and A09 handoff

Illustrative fixed argv, with trusted installation paths and private staging selected by the caller:

```sh
/trusted/blender-4.5.15/blender --background --factory-startup \
  --disable-autoexec --offline-mode --python-exit-code 20 --threads 1 \
  --python /trusted/archy/workers/blender/worker.py -- \
  --request /job/input/request.json --response /job/output/response.json
```

No shell interpolation, `--python-expr`, `--python-use-system-env`, project-supplied worker path or automatic package installation. Resolve packaged imports relative to the trusted worker directory, not the project cwd.
Development harness uses a fresh private directory with absent output names and one writer; Blender's overwrite-warning flags are not exclusive-creation guarantees. Never invoke it against an accepted run.
A09 must provide restricted filesystem/network/environment access, process-tree timeout/cancellation, output/log quotas and no-follow verification before publication through A05. Anticipated smoke limits: 120 s/job, 2 GiB memory and 128 MiB aggregate outputs; enforce real resource limits in A09, not by trusting JSON declarations.
Keep request input read-only and output staging separate. Worker cannot write project revisions, approvals or accepted manifests. An exit code of zero or worker-reported checksum alone is insufficient for acceptance.

## Implementation order and bounded evidence

- [ ] **A06 runtime:** create `workers/blender/{versions.json,README.md,smoke.py}` and ADR0003; fetch the named archive, compare actual SHA-256, inspect license files, record `--version`, `--help`, `bpy.app.build_hash`, `sys.version` and CPU platform. Missing/wrong binary returns `BACKEND_UNAVAILABLE`.
- [ ] **A07 protocol:** create `crates/archy-adapters/src/{lib,protocol,blender}.rs`, `schemas/backend.schema.json`, and `workers/blender/protocol.py`; add source-derived schema/actual-serializer fixtures and malformed request tests. Coordinate workspace/lock changes without changing Task02 geometry or A05 storage contracts.
- [ ] **Scene:** implement `workers/blender/{worker,scene}.py` and Blender-run scene tests using the APIs above. Keep Python protocol unit tests importable without `bpy`; real Blender tests are explicit opt-in integration checks.
- [ ] **Smoke exports/views:** add small `exports.py`, `cameras.py`, `render.py`/material helpers as needed; fixed preset only. Snapshot settings and inspect generated geometry before export. No production render CLI or arbitrary preset editor in this slice.
- [ ] **Positive fixtures:** Task02 analytic rectangle area 200 m²/GFA 600 m² and L area 75 m²/GFA 225 m², each three storeys at 3.2 m (height 9.6 m); repeat with translation [10,-5] and nonzero north metadata. Check per-storey bounds, total volume, L notch and world transforms.
- [ ] **Output checks:** reopen generated `.blend` in a fresh process; parse GLB with an independent format reader and decode PNGs, not just file headers. Verify three 512x512 images, visible foreground/framing, nonempty files, hashes and exact output inventory. Choose/pin the independent readers during implementation; none was verified here.
- [ ] **Negative fixtures:** tampered canonical footprint/ID, nonfinite/overflow values, malformed L notch, excessive storeys, unknown operation/preset, traversal/symlink/collision, invalid JSON and injected Python-like text. Assert no success response or changes to outside sentinels/prior artifacts.
- [ ] **Repeat:** run both shapes twice under the same binary/preset, compare semantic topology/bounds/settings, archive image differences and visual notes. Do not invent a passing cross-GPU pixel threshold; establish/document it in A08 from measured evidence.
- [ ] **Review:** run existing Rust fmt/clippy/tests/schema checks plus Python protocol tests and explicit Blender integration tests. Attach exact commands, versions, exit codes and artifacts; independent review precedes ticket closure. No reviewer/implementer agents are spawned by this research task.

## Licensing and acceptance ledger

The [official Blender license policy](https://www.blender.org/about/license/) specifies GPL-3.0-or-later for distributed Blender binaries and GPL-compatible licensing for published scripts using its Python API.
Propose `GPL-3.0-or-later` for `workers/blender` with its own license/SPDX notices, retaining MIT for original Rust core and neutral protocol schemas. Review actual distribution composition and all bundled notices/source obligations; subprocess separation is not a blanket legal exemption.
Generated assets are not automatically relicensed as Blender source; avoid third-party textures/fonts/assets and account separately for any embedded scripts or supplied assets. No legal compatibility certification is made here.

| Gate | Current status / required evidence |
| --- | --- |
| Artifact listing, published checksum, relevant API mechanisms | Verified through official sources above; no archive download or execution |
| WSL/Ubuntu backend development | Not run here; real binary/hash, scene reopen, GLB/PNG fixtures, exact commands and environment required |
| Exact bundled Python/exporter API and binary compatibility | Unverified until runtime probe; no invented interpreter/build/GPU pin |
| Actual Omarchy CPU smoke and install | Deferred; requires named hardware, OS, binary hash and real outputs; WSL is not a substitute |
| GPU, full isolation, clean target visuals and release acceptance | Deferred to A08/A09/A16/A19; no waiver or inferred pass |

Sufficient mechanism/pin evidence exists to start the bounded runtime and protocol work. Stop research here; resolve remaining runtime uncertainties with the named smoke probes, preserve failures, and report A06 target acceptance as deferred until Omarchy execution exists.
