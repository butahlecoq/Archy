# Task 02 implementation plan

Scope: A03/A04 deterministic rectangle/L generation and geometric evaluation only. Reuse normalized project types, GeoRust validity/area/bounds, ChaCha8 portable seeded streams, SHA-256 canonical parameter identity. No run persistence, CLI generation, workers, MCP or target-machine changes.

1. Write integration tests for golden generation, analytic quantities and canonical parameter/footprint/ID consistency; observe missing behavior before implementation.
2. Add core Variant/ShapeParameters, metrics/checks and the shared canonical constructor/validator. Parameter identity includes project, seed, algorithm, schema, shape and storey inputs; snapshots contain no execution metadata.
3. Implement bounded seeded sampling in translated buildable bounds and evaluation with separate length/area tolerances and explicit professional limits.
4. Extend source-derived schemas and actual serializer contract validation, analytic/generation fixtures, dependency rationale and exact handoff.
5. Run WSL Ubuntu Rust1.99.1 fmt/clippy/workspace tests/schema validation, commit and push bounded changes for independent review.

Risks: floating-point overflow and representability at translated origins; malformed public normalized structs; canonical identity changes; shape/footprint disagreement; duplicate alternatives; false regulatory claims. Reject malformed/unsupported geometry, bound attempts, hash explicit finite canonical values, and retain valid candidates whose declared constraints fail. Hard GFA means minimum declared GFA within 1e-6m²; soft GFA is an objective metric only. Canonical coordinates use exact derived ordering (no tolerance when checking stored derived geometry); measurement tolerances do not authorize tampered snapshots. Actual Omarchy and backend acceptance remain deferred.
