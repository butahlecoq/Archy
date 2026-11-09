# Task02 implementation handoff

Scope: A03/A04 deterministic planar massing and declared geometric evaluation. This is an uncommitted direct implementation for independent review. No CLI generation, persistence, rendering, IFC, MCP, or Omarchy execution is included.

## Public contract

`generate_variants(&NormalizedSpec, count: u32, seed: u64) -> Result<GenerationResult, ArchyError>` uses `GenerationResult { variants: Vec<Variant>, requested_count: u32, attempts: u32, algorithm_version: String, exhaustion_explanations: Vec<String> }`. Count is 1..1000. Attempt limit is count × 100. The result can contain fewer than requested; exhaustion explanations are explicit.

`Variant { schema_version: String, project_id: String, id: String, seed: u64, generation_algorithm: String, shape: Shape, parameters: ShapeParameters, footprint_xy_m: Vec<[f64; 2]>, storeys: u32, floor_to_floor_m: f64 }`. `ShapeParameters` has `Rectangle { origin_xy_m, width_m, depth_m }` and `LShape { origin_xy_m, width_m, depth_m, notch_width_m, notch_depth_m }`, with all dimensions `f64` metres. Shape parameters, not the stored footprint, are authoritative. There is no timestamp or run ID. On read, Variant rejects mismatched shape, footprint ordering/coordinates, hash, or malformed polygon. Signed zero is normalized in newly constructed parameters and rejected in noncanonical parameter or stored-footprint snapshots by bitwise coordinate comparison.

`evaluate_variant(&NormalizedSpec, &Variant) -> Result<EvaluationReport, ArchyError>` uses `EvaluationReport { variant_id: String, evaluation_version: String, metrics: Vec<Metric>, checks: Vec<Check>, tolerance: DimensionalTolerance, geometry_parity_budget: GeometryParityBudget, required_checks_passed: bool, professional_limits: String }`. `Metric { name: String, value: f64, unit: String, method_version: String }`; `Check { constraint_id: String, result: CheckResult, measured: Option<f64>, limit: Option<f64>, explanation: String, required: bool }`, with PASS/FAIL/WARN/UNKNOWN. `DimensionalTolerance { length_m: f64, area_m2: f64, method_version: String }`. `GeometryParityBudget { policy_version: String, coordinate_length_m: f64, footprint_area_m2: f64, gross_floor_area_m2: f64, rigid_transform_policy_version: String }`. Deserialization rejects a report whose required-pass flag contradicts its checks. Required WARN, UNKNOWN, and FAIL all prevent aggregate pass. Errors use the shared `ArchyError` envelope.

## Geometry and identity

Rectangle point order is `[origin, lower-right, upper-right, upper-left, origin]`. L shape removes a positive upper-right notch; order is `[origin, lower-right, right-before-notch, inner-notch, top-after-notch, upper-left, origin]`. Both are CCW. Derived coordinates must match exactly on validation; measurement tolerances do not relax the serialized snapshot contract. GeoRust validates topology and measures unsigned planar area/bounds. Dimensions and notch legs must exceed 1e-6 m and remain representable after translation. Invalid polygons and arithmetic overflow return INVALID_GEOMETRY.

Variant ID is `variant-` plus lowercase SHA-256 over the `archy-variant-v1` domain, then length-prefixed UTF-8 schema version, project ID and algorithm version, big-endian seed and storeys, big-endian IEEE-754 floor height, one shape byte (rectangle 0, L 1), then big-endian IEEE-754 origin x/y, width, depth and optional notch width/depth. Zero is canonical positive zero. Geometric duplicate signature is SHA-256 over `archy-footprint-v1` and canonical point coordinate bytes. Neither identity includes a sample index or execution metadata. Any intentional identity encoding change requires a new domain/version.

Generation uses ChaCha8 with a 32-byte key containing little-endian seed in the first 8 bytes and zero elsewhere. Each unit sample uses the high 53 bits of a stream `next_u64()` word divided by 2^53; shape selection consumes one word modulo the allowed-shape count. Samples use the translated rectangular site reduced by the maximum declared minimum edge setback. Width is 45–98% of available width; depth is sampled around soft target GFA per storey, clamped to a broad 30–98% range. Origins are sampled inside the remaining extent. L notch dimensions are 20–60% of outer dimensions. These ranges and word consumption are part of `chacha8-u53-massing-v1`. Narrow or nonrepresentable envelopes exhaust explicitly. Constraint failures such as excess height remain generated candidates and are reported by evaluation.

Area is footprint area. Approximate GFA is area × storeys and height is floor-to-floor × storeys. Metrics include bounds, four signed site clearances, and target GFA delta. Boundary and declared setback/height checks use absolute 1e-6 m length tolerance. A hard GFA target is interpreted as a minimum approximate GFA with separate absolute 1e-6 m² area tolerance; soft target is a metric and sampling objective only. The geometry parity budget below does not change this check. No authority text grants legal certification. The report describes only declared geometric bounds and uncomputed professional concerns.

## Preliminary review adjudication

- P1 report aggregate: accepted. A tampered required UNKNOWN report was accepted before the change and is rejected on deserialize now. Regression: `serialized_report_rejects_false_required_pass`.
- P2 public normalized spec provenance: accepted. Entry points now share the Task01 identifier grammar and reject duplicate constraint IDs and blank source/authority. Regression: `public_normalized_spec_rejects_ambiguous_provenance`.
- P2 signed zero: accepted. Newly constructed snapshots normalize it and serialized noncanonical parameters are rejected. Regression: `signed_zero_has_one_canonical_serialized_snapshot`.
- Generator stub: obsolete after implementation; existing generation tests observed red on the stub and green afterward.

## Artifacts and verification

`schemas/{variant,generation,evaluation}.schema.json` derive from Rust types. `tests/fixtures/{generation,evaluation}/courtyard.seed42.json` capture actual serializer output. `scripts/check-schemas.py` compares committed schemas to fresh source output and validates every generated variant and report. Dependency rationale is in `docs/dependencies.md`; exact direct pins and transitives are in Cargo.toml/Cargo.lock. No backend dependency is required for unit tests.

## Local verification evidence (29 October 2026, Ubuntu/WSL, Rust 1.99.1)

- Baseline `cargo test --locked --workspace`: exit 101; CLI 5, core 12, eval 3 passed; generation 2 failed on the explicit stub. The coordinator ran and relayed this baseline.
- Focused `cargo test --locked -p archy-generate -p archy-eval` after generation: exit 0; generation 2 and analytic 3 passed.
- Review regressions `cargo test --locked -p archy-eval --test analytic` before fixes: exit 101; `serialized_report_rejects_false_required_pass`, `public_normalized_spec_rejects_ambiguous_provenance`, and `signed_zero_has_one_canonical_serialized_snapshot` failed for the reviewed defects. After fixes: exit 0, 6/6 passed.
- Final `cargo fmt --all -- --check`: exit 0, no output.
- Final `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0.
- Final `cargo test --locked --workspace`: exit 0; CLI 5, core 12, evaluation 6, generation 2 passed; doc tests completed with 0 failures.
- Final `python3 scripts/check-schemas.py`: exit 0; all six schema artifacts valid/source matching, actual Rust serializer outputs conformed, and 26 CLI negative fixtures conformed.
- Scoped `git diff --check -- crates/archy-core/src crates/archy-eval crates/archy-generate`: exit 0. Whole-tree `git diff --check` exits 2 because extensive pre-existing CRLF-only tracked changes are reported as trailing whitespace. Those changes were preserved.
- External GitHub API lookup was reported unavailable by the coordinator because DNS failed in the current sandbox; `gh` is not installed. No network operation was needed for this slice. No commit or push was performed.

Actual Omarchy machine tests remain deferred. The independent final review is preserved in `task-02-review-final.md`; its round1 findings are addressed below, pending further independent recheck.

## Final review fix round1

The immutable final independent review is `task-02-review-final.md`. Its three findings were reproduced and addressed:

- Generated check ID collisions: `site_containment` and `target_gfa` are reserved for internal checks regardless of whether the target is hard. Both original brief parsing and public normalized-spec entry paths call the same constraint identifier validator. The regression `reserved_check_ids_fail_for_parsed_and_public_specs` failed before the fix and passed after it.
- Numeric policy: `polygon-coordinate-parity-v1` is a separate error budget, recorded as `geometry_parity_budget` in each report. Each corresponding transformed vertex coordinate may differ by at most ε = 1e-6 m per axis. For a closed simple polygon with n distinct vertices and transformed axis-wise perimeter P₁ = Σ(|Δx| + |Δy|), the signed shoelace area perturbation is bounded conservatively by εP₁ + nε² m². Repeated-storey GFA budget is exactly storeys × that area budget. The canonical report budget uses its untransformed footprint; a comparison recomputes P₁ after the supplied transform, including rotation. The 10 × 20 m rectangle has P₁ = 60 m and four vertices, so its area parity budget is 60e-6 + 4e-12 m² and three-storey GFA budget is three times that. A 100 × 200 m, five-storey case and a 45° rotation test exercise size, storey and transformed-axis scaling. These parity values are not legal or hard-target tolerances. `rigid_transform_parity_applies_coordinate_and_propagated_area_bounds` confirms a geometry parity pass can coexist with a hard-target FAIL at 600 + 2e-6 m².
- Rigid-transform policy `rigid-xy-ordered-v1`: `compare_rigid_geometry` accepts a finite 2D rotation in radians and finite XY translation, computes expected transformed vertices by the documented rotation matrix and translation in f64, then compares an equally ordered, explicitly closed observed footprint. Reflection, scale, vertex reindexing, malformed topology, vertex-count change and nonfinite transforms are not accepted as equivalent. It reports coordinate, area, GFA and transform-area deltas and passes only when each is within the named budget; transform-induced area drift is compared with the canonical footprint area under the same propagated area budget. This is an analytic policy and synthetic test, not a backend or cross-platform parity claim.
- Stored signed-zero footprint: `Variant::validate` compares every derived/stored coordinate bit, so an injected `-0.0` is rejected even though numeric equality would accept it. The deserialization and in-memory regression failed before the fix and passed after it.

The named hand-authored analytic fixture is `tests/fixtures/analytic/courtyard-shapes.json`; `named_analytic_fixture_matches_rectangle_and_l_measurements` checks its footprints, areas, GFA, height and all four clearances. `changed_geometry_changes_canonical_id` and `translated_generation_stays_within_translated_bounds` close the two generation coverage gaps. `schemas/evaluation.schema.json` and `tests/fixtures/evaluation/courtyard.seed42.json` were regenerated from actual Rust output for the report's new parity field. No new dependency was introduced.

### Round1 verification

Round1 red evidence: `cargo test --locked -p archy-eval --test analytic reserved_check_ids_fail_for_parsed_and_public_specs` exited 101 at the parsed-brief assertion; `... negative_zero_in_stored_footprint_is_rejected` exited 101 at deserialization. The parity test initially failed compilation with unresolved policy API imports (exit 101); after the first implementation, `... rotated_parity_budget_uses_transformed_axis_lengths` exited 101 on the transformed-budget assertion. Each passed after its associated fix.

Final local commands on this round1 source snapshot:

- `cargo fmt --all -- --check`: exit 0, no diff.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0.
- `cargo test --locked --workspace`: exit 0; CLI 5, core 12, evaluation 12, generation 4 integration tests passed, with zero doc-test failures.
- `python3 scripts/check-schemas.py`: exit 0; all six schemas valid and source matching, actual Rust project/normalized/error/generation/variant/evaluation serializers conform, and all 26 CLI negative fixtures conform.
- `git diff --check -- crates/archy-core/src crates/archy-eval crates/archy-generate`: exit 0. Whole-tree diff check remains confounded by preserved pre-existing CRLF-only changes.

Remote PR check 40878229206 failed its formatting checkpoint on an earlier published snapshot; these round1 edits are uncommitted and are not reflected by that run. No commit or push was performed. Actual Omarchy and backend parity tests remain deferred.
