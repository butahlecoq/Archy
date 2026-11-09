# IFC alternative buildings and storeys

## Problem

The current IFC exporter puts every option's full-volume proxy in one
`IfcBuilding`. A BIM consumer can mistake the alternatives for parts of one
building, and the IFC contains no levels for the brief's floors. Saved project
runs use the same workflow manifest version and must remain verifiable. The
current custom property set also uses IFC's reserved `Pset_` prefix.

## Usage

An architect opens the exported IFC and sees one `IfcBuilding` for each option.
Each building has an `ArchyAlternative` property set and a storey for each
floor. Its single `IfcBuildingElementProxy` still represents the full mass and
has an `ArchyMassing` property set. A prior IFC file with `Pset_ArchyMassing`
still verifies through `archy-verify`.

The public function signatures stay unchanged:

```python
export_evaluation(report: dict[str, Any], output: Path) -> Path
verify(evaluation_path: Path, model_path: Path) -> list[str]
```

## Shape

Use the validated evaluation row as the source for an `AlternativeBuilding`
and its storeys. The IFC hierarchy is:

```text
IfcProject
└── IfcSite
    ├── IfcBuilding (alternative A)
    │   ├── IfcBuildingStorey (Level 1, elevation 0)
    │   ├── ...
    │   ├── IfcBuildingStorey (Level N)
    │   └── IfcBuildingElementProxy (full-height mass)
    └── IfcBuilding (alternative B)
```

The project aggregates one site. The site aggregates one building per
alternative. Each building aggregates `floor_count` storeys. A storey's
`Elevation` is `(level_index - 1) * floor_to_floor_height_m`, relative to the
building's local height datum. IFC defines that field as the storey's base
elevation relative to the building reference height. Storeys have no geometry.
The full-height proxy is contained directly in its alternative building
because it spans every storey. Its rectangular extrusion and world bounds do
not change.

New alternative buildings carry the user-defined `ArchyAlternative` property
set with `AlternativeId`, `SourceBriefSHA256`, `FloorCount`,
`FloorToFloorHeight_m`, `BuildingHeight_m`, and `SpatialStructureVersion=1`.
New mass proxies carry the user-defined `ArchyMassing` property set with the
existing footprint, floor inputs, digest, and measurements. `Pset_` is reserved
for property sets declared by IFC, so new exports do not use
`Pset_ArchyMassing`. The verifier accepts that old name for prior bundles.

The exporter validates the existing evaluation report, then creates the site,
alternative buildings, levels, and proxies. Private helpers keep those
decisions inside `archy/ifc.py`. The public `export_evaluation` function and
the manifest version 2 contract remain unchanged.

The verifier classifies the IFC by its structure. A file with no
`ArchyAlternative` metadata and no storeys uses the legacy check. A file with
new metadata or storeys must satisfy the complete new structure. New-form
verification matches building and proxy IDs to the evaluation, checks each
building's property set and storey count/elevations, checks the spatial
relationships, and runs the existing profile, property, and world-geometry
checks for each proxy. Mixed or partial new files fail instead of falling back
to legacy validation. This keeps old workflow manifests valid without making
new verification depend on a second manifest version.

The public interface remains small. The evaluation report is still the single
source of design measurements. IFC properties and relationships expose those
values to BIM consumers, while the verifier owns both legacy and current IFC
rules behind its existing `verify` function. The exporter validates the report
at its boundary and builds the IFC directly from those evaluation rows, so no
second alternative model needs to stay in sync, per Model the Domain. Checking
the external report once at the exporter boundary follows Boundary Discipline.

## Synthesis decision

Both GPT-6-Luna candidates selected one building per alternative, elevation-
only storeys, a single full-height proxy per alternative, and legacy property
set support. Candidate 2 is the base because it specifies a strict classifier
that rejects partial new IFC structures. Candidate 1's emphasis on stable
alternative IDs, exact evaluation-derived heights, and building containment is
included. The cross-judge phase was skipped because the configured pool has
only GPT-6-Luna models and the agent cap is two implementers plus two reviewers.

The candidates and the root reject per-storey mass proxies. They multiply
objects without adding floor construction, repeat geometry, and change the
one-proxy-per-alternative contract. They remain appropriate if Archy later
models separate level mass zones.

## Tradeoffs accepted

- Storeys expose vertical levels but have no geometry because the brief models
  one whole-building mass, not floor plates.
- A single IFC file contains multiple alternative buildings at the same site.
  The `ArchyAlternative` properties identify their design role.
- One storey entity is written per floor, so IFC entity count grows with
  `floor_count`. The existing brief contract has no upper floor-count bound;
  this change adds no new bound.
- New exports use standards-conforming custom property-set names. Tools that
  hard-coded the old custom `Pset_ArchyMassing` name must use `ArchyMassing` for
  new files. Archy's verifier continues to read the old name.

## Alternatives considered

One full-height proxy per alternative is preferred over one proxy per storey.
The full-height proxy remains the same evaluated mass and does not expose
storey-specific geometry that Archy does not calculate. Per-storey proxies
would let BIM tools select level volumes, but would model new geometric
objects and multiply properties without representing slabs or spaces.

Keeping one shared building and putting alternatives in `IfcGroup` objects was
rejected. Storeys with different counts and floor heights would still share one
spatial building and no longer belong to one alternative's vertical structure.

## Open questions and risks

- Does a downstream BIM consumer need an explicit `IfcGroup` for the set of
  alternatives in addition to the `ArchyAlternative` building properties?
- Should a later brief contract cap `floor_count` to bound generated levels?

## Next implementation step

Update `archy/ifc.py` to export one tagged building and its level markers per
alternative, then update `archy/ifc_verify.py` to verify that structure and
preserve legacy model support.

## References

- [IFC4 `IfcBuildingStorey`](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/FINAL/HTML/schema/ifcproductextension/lexical/ifcbuildingstorey.htm)
- [IFC4 `IfcPropertySet`](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/FINAL/HTML/schema/ifckernel/lexical/ifcpropertyset.htm)
- [IfcOpenShell `root.create_entity`](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/root/create_entity/index.html)
- [IfcOpenShell `aggregate.assign_object`](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/aggregate/assign_object/index.html)
