# Optional maximum building height constraint

## Behavior

The v1 massing brief accepts an optional positive finite
`site.max_building_height_m` in metres. The evaluator compares it with the
already reported `building_height_m`, computed as floor-to-floor height times
floor count. Equality passes. Exceeding it adds the
`building_height_maximum` failed constraint and
`building_height_exceeds_maximum` violation, both with metre-specific fields.

The brief, schema, evaluator, verifier, comparison output, operation document,
MCP guidance, and sample brief use the same field and rule. The height limit is
not mandatory and is not treated as a building-code interpretation.

## Compatibility

The feature retains contract version 1. When the height field is absent, the
evaluation keeps its previous site and constraint shapes. The archive verifier
recomputes optional limits and violations from the preserved input brief. The
comparison renderer adds the height summary and metres-based constraint only
when a limit is present, so archived comparisons without the field keep their
prior rendering behavior.

## Verification record

- The checked-in example's three alternatives measure 16 m and pass a 16 m
  limit at equality; the generated comparison includes the limit and all three
  height constraints.
- A standalone evaluation with no height cap passes and contains only the
  existing footprint-area and gross-floor-area constraints.
- A standalone evaluation with a 15.9 m cap records the height violation and
  marks alternatives invalid.
- Full Blender and IFC workflows with the equality cap and the 15.9 m cap both
  produced bundles accepted by `archy.verify`.
- A pre-existing no-height-cap Codex MCP workflow bundle still passes
  `archy.verify`.
- Two GPT-6-Luna reviewers at medium reasoning found no remaining issues after
  resolving the documentation findings.

No automated test suite was run.
