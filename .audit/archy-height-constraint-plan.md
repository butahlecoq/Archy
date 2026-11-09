# Optional maximum building height constraint

## Goal

Allow a massing brief to express an optional site-wide maximum total building
height and report whether each alternative satisfies it. Existing briefs
without the field must retain their current results and report shape.

## Contract decision

- Add `site.max_building_height_m` as an optional positive finite value in the
  existing version 1 brief.
- Interpret the limit against `building_height_m`, calculated as floor-to-floor
  height multiplied by floor count.
- Equality passes. An exceeded limit makes the alternative invalid and adds a
  named constraint and violation.
- Keep area constraints and all existing outputs unchanged when the optional
  height limit is absent.
- Preserve contract version 1; this is an additive optional limit, not a change
  to the measurement or geometry model.

## Delivery

- Evaluator and JSON Schema.
- Independent archived-workflow verification.
- Comparison summary and HTML constraint display.
- MCP drafting guidance and operation contract documentation.
- Representative brief example and a recorded implementation outcome.

## Verification

- Exercise the real evaluator with an omitted limit, an exactly matching limit,
  and an exceeded limit.
- Run the real workflow on a capped brief and verify the generated bundle.
- Parse the schema and compile the changed Python modules; do not run the
  automated test suite.
