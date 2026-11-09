# Oriented rectangular footprints: design rationale

## Problem

Archy's evaluator, preview, comparison plan, IFC proxy, and archived-bundle
verifiers all assume footprints align with the site axes. Adding rotation in
only one adapter would produce contradictory design and constraint results.
The feature needs one pose convention shared by every output while preserving
existing axis-aligned briefs and saved artifacts.

## Usage

Existing briefs remain unchanged. A brief can now rotate a local rectangular
footprint around its lower-left local corner:

```yaml
footprint:
  width_m: 20
  depth_m: 15
  origin_x_m: 5
  origin_y_m: 4
  rotation_degrees: 30
```

Angles are finite values in `[0, 360)`, measured counter-clockwise from east.
The origin is the world coordinate of the rectangle's local `(0, 0)` corner.
An omitted angle is zero and preserves current placement exactly.

## Shape

`archy.geometry` owns an immutable `Footprint` and `BuildableArea`. `Footprint`
provides `from_mapping`, `corners`, `center`, and `bounds`. The four ordered
corners are defined by `(x + u cosθ - v sinθ, y + u sinθ + v cosθ)` for local
rectangle corners `(u, v)`. `BuildableArea.contains` checks every corner
against the closed setback rectangle with a `1e-9 m` numeric tolerance. Exact
cardinal input angles use exact sine/cosine pairs. This is sufficient because
both shapes are convex. Bounds are derived from those corners and are used for
framing and verification, never for the setback predicate.

`massing.py` parses and validates at the brief boundary, defaults an omitted
angle to zero, and emits a normalized report pose. The report preserves the
version 1 contract, version 2 manifest, existing `measured` bounds violation,
and adds ordered `measured_polygon` points for rotated setback failures.
Historical reports without an angle are interpreted as zero.

Blender builds and inspects the rotated prism from the report pose. Camera
framing and inspection use world-space corners, not the object's unrotated
dimensions. The HTML comparison draws the same four points as an SVG polygon
and includes a visible orientation caption. IFC keeps its centered local
rectangle profile, with product placement rotated around the local-corner
anchor. The IFC verifier checks the property angle and actual represented
vertices. The archive verifier recomputes corner containment, bounds, and
violation details from the report; for legacy rows it defaults to zero and
retains their existing payload shape. The MCP prompt and operation contract
describe the same anchor and angle convention.

The geometry module provides a single typed seam for the transform and
containment rules. The brief schema and external JSON remain boundary formats;
adapters parse once and pass the immutable footprint internally. This keeps
trigonometry and setback policy out of renderers and exporters.

## Synthesis decision

Candidate 2 is the base. It scored higher on polygon correctness and coherent
adapter behavior because it centralizes pose and containment in a pure geometry
module. Candidate 1 put helpers in `massing.py` and rotated about the center
while keeping the unrotated lower-left point as its origin. We rejected that
pivot because the origin stops being a physical footprint corner at nonzero
angles. Both sketches' local-origin convention, omitted-angle compatibility,
and cross-artifact verification were retained. The arena cross-judge was
skipped: the configured pool contains only GPT-6-Luna, and the user limited
delegation to two implementers and two reviewers.

The selected candidate proposed accepting any finite angle and rotating the
IFC proxy by translating to the world center while retaining a centered local
profile. We narrowed input to `[0, 360)` for canonical serialization and
corrected the IFC transform: retain the centered profile, but rotate/translate
the product placement at the local-corner origin so the center offset is
applied exactly once. Candidate 1's explicit camera-framing concern is included
by deriving the view bounds from transformed world corners.

| Criterion | Candidate 1 | Candidate 2 |
|---|---:|---:|
| Brief and archive compatibility | 4 | 4 |
| Polygon setback semantics | 4 | 5 |
| Cross-adapter geometric consistency | 3 | 5 |
| Interface depth and scope | 4 | 4 |
| Geometry verification coverage | 3 | 4 |

## Tradeoffs accepted

- Keep the strict version 1 brief/report and version 2 artifact manifest in
  exchange for additive optional rotation and direct compatibility with saved
  briefs and evaluations.
- Use the local-corner anchor in exchange for a simple pose transform and a
  stable physical meaning for `origin_x_m`/`origin_y_m`.
- Use a tiny site-coordinate tolerance in exchange for stable closed-boundary
  checks with floating-point rotations.
- Share a small geometry module in exchange for one authoritative polygon and
  containment implementation across evaluation and adapters.

## Alternatives considered

- Center-pivot rotation was rejected because it makes the existing origin a
  virtual reference rather than a footprint vertex, obscuring where the
  building is placed.
- Independent rotation logic in the evaluator, Blender, comparison, and IFC
  adapters was rejected because changing one transform could desynchronize the
  report from the model.
- User-supplied polygon vertices were rejected because they duplicate
  width/depth and allow inconsistent geometry for a rectangular mass.

## Open questions and risks

- The user-facing angle convention has no clockwise-negative shorthand; use
  `[0, 360)` with `270°` for a 90° clockwise turn.
- IFC placement was checked against represented world vertices from the
  installed IfcOpenShell version, including non-cardinal angles.
- The installed IFC geometry backend cannot verify every positive extrusion
  depth. Archy therefore requires a conservative minimum total IFC extrusion
  of 0.1 mm; the current installed workflow verifies that depth, while a 1 µm
  extrusion fails geometry processing. Re-probe this boundary if the geometry
  backend changes before lowering it.
- Archived HTML must remain byte-for-byte canonical when old comparison rows
  omit the angle field.

## Implementation outcome

The pure footprint type and evaluator were connected to Blender, HTML, IFC, and
archive verification against the frozen pose API. The workflow validates the
report-level IFC constraints and imports both IFC API and geometry modules
before it starts Blender; geometry-kernel errors still surface at IFC export or
verification.
