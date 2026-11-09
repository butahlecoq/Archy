# IFC alternatives and storey structure

## Goal

Make each design alternative distinct in exported IFC and expose its floor
levels to BIM tools, without pretending that Archy has modeled slabs, rooms, or
code compliance.

## Grounding

`archy/ifc.py` creates one `IfcProject`, one `IfcSite`, one `IfcBuilding`, and
one `IfcBuildingElementProxy` per alternative. Each proxy contains the full
extruded mass and carries a custom `Pset_ArchyMassing` property set. The verified
persisted run at
`/tmp/archy-mcp-projects-st8jiq60/93438658-1164-4d2c-8469-633d351390d7/runs/run-34a5e7b3-c9a9-41d9-87c2-7a00c5c9e391`
has three alternatives with five floors each, one building, no storeys, and
three proxies. `archy/ifc_verify.py` checks proxy properties and world geometry
against `evaluation.json`. `archy/workflow.py` calls that verifier before it
writes the version 2 manifest. `archy.verify` also rechecks stored IFC files,
so the new verifier must keep older verified bundles readable.

The IFC4 specification reserves the `Pset_` prefix for property sets defined
by IFC. User-defined property sets must use another prefix. The existing
`Pset_ArchyMassing` name is user-defined, so new exports will use
`ArchyMassing`; the verifier will continue to read the legacy name.

Sources:

- [IfcBuildingStorey in IFC4](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/FINAL/HTML/schema/ifcproductextension/lexical/ifcbuildingstorey.htm)
- [IfcPropertySet in IFC4](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/FINAL/HTML/schema/ifckernel/lexical/ifcpropertyset.htm)
- [IfcOpenShell entity creation](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/root/create_entity/index.html)
- [IfcOpenShell spatial aggregation](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/aggregate/assign_object/index.html)

## Caller view

An IFC consumer can list one `IfcBuilding` per design option, inspect its
`ArchyAlternative` properties, and read `IfcBuildingStorey.Elevation` for each
floor. The existing full-height mass proxy remains contained in its alternative
building. Each storey records a level, not floor geometry.

## Data shape

- `AlternativeBuilding`: one IFC building for each evaluation alternative, with
  `AlternativeId`, source brief digest, floor count, floor-to-floor height,
  total height, and a spatial-structure version in the user-defined
  `ArchyAlternative` property set.
- `StoreyLevel`: one `IfcBuildingStorey` per floor, named `Level 1` through
  `Level N`, with elevation `(level_index - 1) * floor_to_floor_height_m` in the
  building's local height datum.
- `AlternativeMassing`: one unchanged full-height rectangular proxy per
  alternative, contained by its `AlternativeBuilding`. Its `ArchyMassing`
  property set records the existing geometry inputs and measurements.
- Legacy model: one building, no storeys, and the old `Pset_ArchyMassing`
  property set. The verifier continues to validate its proxy IDs, properties,
  and geometry.

## Chosen shape and compatibility

Each alternative has its own building with one elevation-only storey per
floor. The unchanged full-height proxy is contained by that building because
it spans every storey. The export does not create floor geometry or spaces.

The verifier identifies new models when the `ArchyAlternative` property set
or storeys are present. It validates alternative IDs, storey counts and
elevations, exclusive parent relationships, storey geometry absence, proxy
containment, proxy properties, and proxy geometry. It uses the legacy path only
when no new alternative metadata or storeys are present. The artifact manifest
remains version 2 because its shape does not change.

## Throughput checkpoint

- **Blocking first steps.** Inspect the exporter, verifier, workflow manifest,
  and a real verified bundle before changing IFC semantics. Done.
- **Independent workstreams.** Implementer one owns `archy/ifc.py` and the
  exported IFC structure. Implementer two owns `archy/ifc_verify.py` and legacy
  compatibility. The root owns the shared contract, docs, plan, and integration.
- **Shared mutable state.** The implementers write disjoint modules against the
  selected data shape. Both consume the same read-only evaluation report. The
  generated workflow output goes to a new temporary directory.
- **Smallest safe decomposition.** Keep the change within the existing
  exporter and verifier. Do not add a new domain module, input field, manifest
  version, slabs, spaces, or code-compliance claims.

## Design phase checklist

- [x] Ground the exporter, verifier, persisted artifact, and IFC4 conventions.
- [x] Produce two design sketches with the configured implementers. Both
  selected the same IFC hierarchy and building-level proxy.
- [x] Score both sketches and select the structure before either agent edits
  application code. Candidate 2 is the base for its strict new/legacy
  classifier. Candidate 1's explicit stable-ID and relationship checks are
  included.
- [x] Check the selected sketch against IFC4 spatial and property-set rules.
  The cross-family judge was skipped because the configured model pool only
  contains GPT-6-Luna and the user caps delegation at two implementers and two
  reviewers.
- [x] Implement exporter and verifier in their separate files.

## Review and proof

Two independent GPT-6-Luna medium reviewers checked the implementation and
IFC boundaries. Their findings were fixed: reject proxy count mismatches,
forbid storey geometry, and require one exclusive parent of the expected IFC
class for the site, each building, and each storey. The first end-to-end run
also exposed IfcOpenShell entity-wrapper list comparisons in parent checks;
these now compare entity IDs. Follow-up review further required those parent
links to use `IfcRelAggregates` specifically. Both reviewers approved that
final relation-type check. The model pool has one family, so the optional
cross-family design judge was unavailable. A fresh workflow run succeeds, and
the new bundle and persisted pre-change bundle both pass `archy-verify` with
the final verifier. Direct IFC inspection confirms three buildings, fifteen
levels, and three proxies. Syntax compilation and `git diff --check` pass. No
automated tests were added or run. Keep this work local and uncommitted.

## Work log

- [x] Confirmed the current persisted bundle verifies and has one building, no
  storeys, and three alternative proxies.
- [x] Compared the two candidates and selected one building per alternative,
  elevation-only storeys, and one unchanged mass proxy per alternative. See
  `.audit/archy-ifc-alternative-storeys-design.md`.
- [x] Implement IFC alternative buildings and storeys in separate modules.
- [x] Review behavior, IFC semantics, boundaries, and comments with two
  reviewers; fix their concrete findings.
- [x] Verify a new real workflow bundle and an older saved bundle.
- [x] Recheck syntax and whitespace.
- [x] Preserve existing uncommitted work and leave this change uncommitted.
