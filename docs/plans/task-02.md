# Task 02: deterministic generation and evaluation (A03-A04)

**Spec:** ../specs/release-0.1.txt, sections 3–4 and acceptance tests. Research: ../research/omarchy-and-reuse.md, especially geometry. Depends on reviewed task01 public normalized types. Implement with Sol medium, independently review with Sol medium. No CLI persistence/worker/MCP features in this slice.

## Interfaces and files

Create `archy-core/src/variant.rs`, `metric.rs`, `check.rs`, with serde/schemars public types exported through lib.rs. `Variant` contains schema_version, project_id, deterministic id, seed, generation_algorithm, shape and editable shape parameters, closed footprint in metres, storeys and floor_to_floor_m. The serialized immutable snapshot contains all geometry-producing inputs; no timestamps or run ID in deterministic content. Metadata for physical runs is added by A05.

Create `archy-generate` and `archy-eval` workspace crates, with focused source modules and integration tests. Public entry points: `generate_variants(spec: &NormalizedSpec, count: u32, seed: u64) -> Result<GenerationResult, ArchyError>` and `evaluate_variant(spec: &NormalizedSpec, variant: &Variant) -> Result<EvaluationReport, ArchyError>`. `GenerationResult` includes variants, requested_count, attempts, algorithm_version and explicit exhaustion explanations if fewer than requested. `EvaluationReport` includes variant ID, versioned metrics, checks, dimensional tolerance metadata, a required-checks-passed boolean and professional-limits statement. Follow established error envelope.

Document exact final structs in task-02-handoff.md. Update Cargo workspace/lock, schema artifacts and dependency rationale. Own tests/fixtures/generation and tests/fixtures/analytic. Existing project parsing must remain shared, not duplicated.

## Implementation decisions

- Reuse GeoRust `geo` for polygon validity, area and bounds/predicates rather than inventing a CAD kernel. Official inspected version0.33.1 exports `geo::algorithm::Validation` (`check_validation`) and `geo::Area` (`unsigned_area`). Do not repair invalid polygons silently.
- Use a specified portable seeded generator, such as pinned rand_chacha0.10.0 (official docs promise deterministic portable streams); record RNG/sampling algorithm version and dependency lock. Do not use platform-default hashes/RNGs or wall-clock randomness for geometry. SHA-256 via sha2 (inspected0.11.1) is appropriate for stable parameter identity. Resolve compatible dependencies and justify changes.
- Rectangle is four corners plus closing point. L shape is an outer rectangle with a positive upper-right rectangular notch, six corners plus closure; dimensions and origins are explicit. All outputs are CCW, axis-aligned, non-self-intersecting, positive and finite. Use the actual translated site envelope; north angle remains orientation metadata, not a reason to silently rotate local coordinates.
- Generate bounded dimensions/positions within the buildable rectangle. Target GFA influences size without being promoted to law. Honour allowed shapes and sample distinct alternatives. Count range1..1000. Bound attempts and explain exhaustion, including dimensions below supported numeric resolution. A stable geometric signature suppresses duplicate footprints, independent of sample index/vertex-start ordering.
- Invalid constraint outcomes (for example height exceeding cap) remain candidates with evaluation failures; do not filter them out or call them successful renders. Malformed geometry is INVALID_GEOMETRY, not a zero-area PASS. Never claim feasibility just because the requested count was reached.
- Area is footprint area; gross floor area is footprint area times storeys, an approximation; height is floor_to_floor_m times storeys. Report xmin/ymin/xmax/ymax and each site's edge clearance, including negative clearances outside site. Signed polygon area cannot yield negative GFA.
- Each metric has name/value/unit/method_version. Each check has constraint_id/result/measured/limit/explanation and required flag. Results are PASS/FAIL/WARN/UNKNOWN. Required UNKNOWN or FAIL prevents required-checks-passed. Assumed sources do not prevent measuring geometry, but no authority string creates legal certification. State only that declared geometric bounds passed.
- Record length tolerance1e-6m and separate area tolerance1e-6m², with algorithm versions. Compare boundary touching and differences within tolerance consistently. Hard target area means minimum target GFA within area tolerance; soft target contributes a metric/objective only. This interpretation must be documented, not silently inferred.

## Tests and evidence

Write tests before implementation and observe red/green where feasible. Golden spec:20x35m,3 storeys,3.2m floor height,12m cap,3m setback,900m² soft target,count3,seed42. Two independent generations serialize identically and evaluate identically. At least3 geometrically distinct feasible options; shape restriction tests cover rectangle and L. Changing seed yields a changed set; changing geometry changes ID. No run metadata contaminates deterministic identity. Tight/narrow bounds terminate and explain fewer options.

Analytic rectangle fixture: [(3,3),(13,3),(13,23),(3,23),(3,3)] gives area200m²,GFA600m²,height9.6m,edge clearances3/7/3/12m on golden site. L fixture [(3,3),(13,3),(13,8),(8,8),(8,13),(3,13),(3,3)] gives area75m² and GFA225m². Translate both site and footprint by [10,-5] and preserve measurements. Test clockwise representation, duplicate/consecutive points, bow-tie, zero area, missing closure, non-finite coordinates, outside-site footprint, excessive height, arithmetic overflow and project/variant mismatch.

Test exact setback/height limits, ±0.5e-6m and ±2e-6m; area thresholds use area tolerance. Assert no arbitrary source/authority text grants legal compliance. Construct required UNKNOWN checks and prove aggregation cannot pass. Verify all serialized metric/check fields and generated schemas using actual output, not just handwritten fixtures.

Run WSL Ubuntu pinned Rust fmt/clippy/test with existing target directory. Commit and push the bounded source/tests/evidence; report only tests actually run. No Omarchy machine testing is performed here. Independent Sol reviewer verifies both requirements and code quality. Do not mark A03/A04 closed until evidence and review pass.

## Canonical geometry consistency requirement

Shape parameters are the editable source; footprint is derived and stored for inspection. Provide one shared deterministic derivation/validation path so generator, evaluator and future workers cannot disagree. When reading a serialized Variant, reject a footprint that differs from its declared shape parameters or an ID that differs from its canonical parameter hash; never quietly evaluate one geometry while rendering another. Test tampered params, footprint, project ID and identifier, alongside valid round-trip serialization. Exact derived coordinate ordering/tolerance and identity serialization must be documented.
