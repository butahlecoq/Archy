# Massing operation contract

This document defines the first deterministic operation boundary for Archy 0.1. The version 1 input structure is described by [massing-brief.schema.json](massing-brief.schema.json). The Python implementation is available as `python -m archy <brief.yaml>`. It reads the YAML brief and emits JSON measurements and constraint outcomes without calling an agent or IFC service.

## Shared conventions

- Lengths are metres and areas are square metres.
- The site coordinate system is local Cartesian: X east, Y north, Z up. The site origin is the southwest corner of the rectangular site.
- Dimensions are finite positive decimal values. Setbacks may be zero. Floor count is a positive integer.
- Calculations use decimal input values without unit conversion. Reports round display values only; constraint comparisons use unrounded values.
- A failed precondition is an error. Operations never silently clamp, repair, or reinterpret inputs.

## Input shapes

```yaml
schema_version: 1
contract_version: 1
project: {id: sample, name: Sample}
units: {length: m, area: m2}
coordinate_system: {origin: southwest_site_corner, x_axis: east, y_axis: north, z_axis: up}
site:
  width_m: 30
  depth_m: 24
  setbacks_m: {north: 3, east: 2, south: 3, west: 2}
  max_footprint_area_m2: 500
  target_gross_floor_area_m2: 1800
  max_building_height_m: 20
alternatives:
  - id: compact
    footprint: {width_m: 20, depth_m: 15, origin_x_m: 5, origin_y_m: 4, rotation_degrees: 30}
    floor_count: 6
    floor_to_floor_height_m: 3.2
```

`site` describes a rectangular parcel and its limits. `max_building_height_m` is an optional positive height limit in metres; omit it when the brief supplies no such limit. Each entry in `alternatives` describes a constant rectangular footprint extruded vertically into equal-height floors. The footprint origin is its local `(0, 0)` corner in site coordinates. `rotation_degrees` is optional, finite, and in `[0, 360)`; it measures counter-clockwise rotation from east about that corner. If omitted, the angle is zero. Gross floor area is an explicit rectangular-massing estimate equal to footprint area times floor count; it does not subtract voids, openings, or other deductions. Openings, internal floor plates, overhangs, sloped walls, and non-rectangular parcels are outside this contract.

## Operations

### `validate_alternative(site, alternative)`

Checks required fields and domain constraints, then returns either a list of named violations or success. The footprint must fit inside the setback buildable rectangle:

```text
setback_min_x = west setback
setback_max_x = site width - east setback
setback_min_y = south setback
setback_max_y = site depth - north setback
```

The footprint must have positive dimensions. Its four corners are transformed from the local rectangle by `x = origin_x + u cos(θ) - v sin(θ)` and `y = origin_y + u sin(θ) + v cos(θ)`, where `(u, v)` are the local corner coordinates and `θ` is the angle in radians. Setbacks that consume the entire site are invalid. Inputs with missing, non-finite, or non-positive dimensions, an invalid rotation, or an invalid floor count, are rejected as malformed. A footprint is inside only when every transformed corner lies in the closed setback rectangle, allowing a `1e-9 m` coordinate tolerance; because both regions are convex, this also means the full footprint lies inside. A footprint outside the buildable rectangle remains in the evaluation with a named violation, so other alternatives can still be compared. `measured` preserves its axis-aligned envelope and `measured_polygon` lists the four ordered world-coordinate corners. Area limit failures are returned the same way.

The result shape is:

```yaml
valid: false
violations:
  - code: footprint_outside_buildable_area
    measured: { west_m: 5, east_m: 25, south_m: 4, north_m: 19 }
    allowed: { west_m: 2, east_m: 28, south_m: 3, north_m: 21 }
    measured_polygon: [{x_m: 5, y_m: 4}, {x_m: 25, y_m: 4}, {x_m: 25, y_m: 19}, {x_m: 5, y_m: 19}]
```

Violations are sorted by stable code so repeated evaluation yields the same result.

When the optional height limit is exceeded, the report records the failed
constraint and its violation in metres:

```yaml
constraints:
  - code: building_height_maximum
    measured_m: 19.2
    operator: <=
    limit_m: 18
    passed: false
violations:
  - code: building_height_exceeds_maximum
    measured_m: 19.2
    allowed_m: 18
```

### `measure_alternative(alternative)`

Computes the same quantities from parameters without relying on a rendered or BIM model:

```text
footprint_area_m2 = footprint width × footprint depth
gross_floor_area_m2 = footprint area × floor count
building_height_m = floor-to-floor height × floor count
```

It returns the alternative ID, the three unrounded measurements, and the formulas used. It rejects malformed inputs using the same rules as validation.

### `evaluate_constraints(site, alternative, measurements)`

Evaluates each brief limit independently and returns `pass` or `fail` with measured and allowed values. The area constraints are `footprint_area_m2 <= max_footprint_area_m2` and `gross_floor_area_m2 >= target_gross_floor_area_m2`. When `site.max_building_height_m` is supplied, the evaluator also checks `building_height_m <= max_building_height_m`; its constraint record uses `measured_m` and `limit_m`, while area records use `measured_m2` and `limit_m2`. A height over the limit produces `building_height_exceeds_maximum` with measured and allowed values in metres. A setback failure appears in `violations` with measured and allowed site bounds. Any failed constraint or setback makes the alternative invalid. Equality passes. Unknown constraints are errors, not ignored.

## Determinism and reproducibility

For identical input values, each operation returns identical ordered results. The JSON report records each alternative's parameters (including an explicit zero angle when omitted in the brief), operation contract version, formulas, units, raw measurements, and constraint outcomes. The original brief bytes remain the basis of its source digest. An independent checker can reproduce every measurement using the equations above without Blender or IFC. Rendering and BIM generation may consume a validated alternative, but neither may change these authoritative input values or measurement rules.

Run the example from the repository root with `python -m archy examples/massing-brief.yaml`. The command exits zero when every alternative satisfies its stated constraints and nonzero when one or more alternatives fail a constraint. Malformed briefs produce an error and nonzero exit.

To create a Blender scene from the report, redirect the JSON output to a file and run `env -u PYTHONHOME -u PYTHONPATH PATH=/usr/bin:/bin blender --background --python-exit-code 1 --python archy/blender_scene.py -- report.json scene.blend`. The restricted `PATH` keeps environment managers such as mise from placing an incompatible Python ahead of system tools; `PYTHONHOME` and `PYTHONPATH` can also point Blender's embedded Python at an incompatible installation. Archy's full workflow isolates the Blender child process with a system `PATH` and removes both Python variables automatically. The adapter maps metres to Blender metric units at scale 1.0, X to east, Y to north, and Z to up. It creates one rectangular solid per alternative, rotates it about its local lower-left corner, and extends it from ground level to the measured building height. Render isolated PNG previews with `env -u PYTHONHOME -u PYTHONPATH PATH=/usr/bin:/bin blender --background --python-exit-code 1 --python archy/blender_scene.py -- report.json scene.blend --render-dir renders/`. Reopen and compare each saved mesh's world-space geometry and bounds to the report with `env -u PYTHONHOME -u PYTHONPATH PATH=/usr/bin:/bin blender --background --python-exit-code 1 --python archy/blender_scene.py -- report.json scene.blend --inspect --inspection-json inspection.json`. Before Blender starts, the workflow validates the IFC input and checks that IfcOpenShell is available. IFC export requires each alternative's total building extrusion height to be at least `0.0001 m` (0.1 mm); standalone massing evaluation has no such minimum. Backend-specific export or geometry errors may still occur after Blender processing. The workflow exports one `IfcBuilding` per alternative, one elevation-only `IfcBuildingStorey` per floor, and one full-height rectangular `IfcBuildingElementProxy` per alternative. The user-defined `ArchyAlternative` and `ArchyMassing` property sets record alternative IDs, source digests, inputs, and measurements. The verifier also reads the historical `Pset_ArchyMassing` name. `python -m archy.ifc_verify evaluation.json massing.ifc` checks the IFC structure, geometry, and properties against the evaluation report.

## Limits

These checks establish only the rectangular massing constraints stated here. They do not evaluate building codes, floor usability, structural feasibility, daylight, circulation, site survey accuracy, or BIM semantics beyond the massing proxy properties. The IFC proxy does not represent detailed floor plates, spaces, materials, or code classifications. A later contract version may add those only with explicit input fields, units, and independently reproducible rules.
