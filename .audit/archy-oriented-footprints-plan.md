# Oriented rectangular footprints

## Goal

Extend Archy's site studies to represent rectangular building footprints with a
plan rotation and evaluate their actual polygon against setbacks. Keep existing
briefs and saved version 1 runs usable.

## Architect phases

- [x] Ground the current brief, evaluation, rendering, comparison, and IFC flow.
- [x] Sketch two independent full-shape designs with the two implementers.
- [x] Compare the designs against the rubric, select a base, and record synthesis.
- [x] Implement in disjoint modules and keep the two workstreams verifiable.
- [x] Recheck the design when implementation exposes backend limits; extend the
  export boundary explicitly rather than allowing a late opaque geometry error.

## Grounding

The YAML brief is checked against `docs/massing-brief.schema.json`, then parsed
in `archy/massing.py` into `Site` and `Alternative` records. `evaluate_brief`
currently treats width and depth as global X/Y axes, computes an
axis-aligned bounding rectangle from `origin_x_m`, `origin_y_m`, `width_m`, and
`depth_m`, and checks that rectangle against the setback envelope. The report
embeds both the exact brief and derived inputs, measurements, and violations.

`archy.workflow` sends that report to `archy/blender_scene.py` in a separate
Blender process, verifies the saved mesh bounds, exports IFC, then builds a
JSON/HTML comparison. The Blender script creates an unrotated cube and checks
axis-aligned world bounds. `archy/comparison.py` draws an axis-aligned SVG plan.
`archy/ifc.py` exports a rectangular profile placed by translation only;
`archy/ifc_verify.py` checks the profile and world extents. `archy.verify`
repeats evaluation consistency checks when a bundle is reopened. The operation
contract, schema description, and MCP drafting prompt all currently say
axis-aligned. All these consumers must agree on the same orientation semantics.

Existing briefs have no orientation field, and saved reports/manifests have
contract version 1/manifest version 2. Treat an omitted angle as zero. Do not
change the project/revision store or the IFC hierarchy from the previous
feature. Preserve existing axis-aligned behavior exactly.

## Arena rubric

Score each candidate from 1 to 5 on:

1. Existing brief and saved-bundle compatibility, with clear angle semantics.
2. Correct geometric evaluation of rotated corners against the setback
   rectangle, including boundary-coincident and off-site footprints.
3. A single coherent footprint model and consistent behavior across Blender,
   HTML, IFC export, and all verifiers.
4. A small public contract with domain validation at the boundary and no
   duplicated unsynchronized geometry rules.
5. Realistic verification on the installed workflow, including zero rotation,
   cardinal rotation, and a non-cardinal orientation.

## Selected geometry contract

Use a pure `archy.geometry` module with an immutable `Footprint` value and a
`BuildableArea`. The footprint's origin is its actual local `(0, 0)` corner in
site coordinates. Its local X axis points east at zero rotation; positive
`rotation_degrees` rotates counter-clockwise about that origin. Missing angles
mean zero; explicit values must be finite and lie in `[0, 360)`. Preserve the
version 1 brief/report contracts, version 2 manifest, and all existing field
meanings.

The transformed corners are the canonical polygon. Its axis-aligned bounds are
for display and geometry checks only. A footprint satisfies setbacks exactly
when all four corners lie inside the closed buildable rectangle; use a shared
`1e-9 m` numeric tolerance for computed coordinates and snap exact cardinal
angles to exact sine/cosine pairs. The evaluator keeps its existing `measured`
bounds violation field and adds ordered `measured_polygon` points.

Keep the rectangle profile positioned at its local center `(width/2,
depth/2)`. IFC product placement uses rotation about Z and translation to the
local-corner anchor `(origin_x_m, origin_y_m, 0)`. Do not translate to the
rotated center while retaining the centered profile; that would apply the
half-width/depth offset twice.

## Implementation workstreams

- Implementer 1 owns `archy/geometry.py`, `archy/massing.py`,
  `docs/massing-brief.schema.json`, `docs/massing-operations.md`, and
  `archy/mcp_prompts.py`. It implements the validated pose, corners, bounds,
  site containment, report serialization, and input guidance.
- Implementer 2 owns `archy/blender_scene.py`, `archy/comparison.py`,
  `archy/ifc.py`, `archy/ifc_verify.py`, `archy/verify.py`, `README.md`,
  `docs/proof-of-concept.md`, and `docs/roadmap.md`. It implements each output
  adapter and verifier against the frozen `Footprint` API above.
- The report shape and helper signatures are shared read-only design inputs;
  the two modules lists are disjoint.

## Verification plan

Do not add or run automated tests. Use the installed workflow on the existing
brief and a purpose-built oriented brief. Reopen both bundles with
`archy-verify`. Inspect site-plan polygons, Blender world bounds, IFC rotation,
property values, and verifier output for 0°, 90°, and one non-cardinal angle.
Run the older persisted bundle through `archy-verify` to prove compatibility.
Keep the checkout local and uncommitted.

## Work log

- [x] Read the current source, schema, operation contract, MCP prompt, and
  previous feature audits.
- [x] Produce two independent design sketches and choose the final shape.
- [x] Implement the chosen shape with two disjoint implementer workstreams.
- [x] Review the implementation with two reviewers and resolve findings.
- [x] Verify standard, rotated, boundary, and legacy artifacts.
- [x] Recheck syntax, whitespace, and existing uncommitted work.

## Final implementation and evidence

- The two implementation workstreams added the immutable footprint geometry
  contract and connected it to evaluation, Blender, HTML, IFC, archive
  verification, documentation, and the MCP drafting prompt. The two reviewers
  checked compatibility, geometric matching, and the report/export boundary;
  their concrete findings were resolved.
- An IFC geometry-kernel probe showed that 0.1 mm (`1e-4 m`) total building
  height exports and verifies, while 1 µm (`1e-6 m`) exports but cannot be
  processed by the verifier. A separate 0.1 µm attempt had failed during IFC
  shape creation. Archy now names a 0.1 mm minimum total extrusion depth,
  validates it before Blender, and checks that both required IfcOpenShell API
  and geometry modules import. Standalone massing evaluation still accepts
  smaller positive heights.
- The full rotated brief completed at
  `/tmp/archy-oriented-footprints-final-v3-20261031`; its four alternatives
  cover 0°, 90°, a valid 30° edge touch, and an invalid 30° setback crossing.
  The one millimeter and 0.1 mm rotated briefs completed after preflight was
  added. All three bundles pass `archy-verify`.
- The 1 µm brief exits with the explicit IFC minimum error before Blender
  starts. The previous massing bundle and the persisted legacy IFC/storey
  bundle at
  `/tmp/archy-mcp-projects-st8jiq60/93438658-1164-4d2c-8469-633d351390d7/runs/run-34a5e7b3-c9a9-41d9-87c2-7a00c5c9e391`
  both pass `archy-verify`.
- Reviewers caught and resolved two preflight wording/capability gaps: docs now
  distinguish report/dependency preflight from later geometry-kernel failures,
  and the preflight imports both `ifcopenshell.api` and `ifcopenshell.geom`.
- Python compilation and `git diff --check` pass. No automated tests were added
  or run. Work remains uncommitted in the local checkout.
