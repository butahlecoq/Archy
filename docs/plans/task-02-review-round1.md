# Task 02 independent round1 rereview

Scope: the three findings in `task-02-review-final.md`, its three named evidence gaps, and breakage introduced by the round1 fixes. Reviewed the new round1 handoff section, source, tests, schema/fixture outputs, and ran targeted checks on this workspace snapshot. Previous reviews remain unchanged. No implementation edits, commits, or pushes were made.

## Verdicts

| Item | Verdict | Evidence |
| --- | --- | --- |
| Reserved generated check IDs in original brief and public normalized-spec paths | **ADDRESSED** | Shared `constraint_identifier` rejects `site_containment` and `target_gfa` ([`crates/archy-core/src/validation.rs:120-128`](../../crates/archy-core/src/validation.rs)); both normalization ([`validation.rs:275-283`](../../crates/archy-core/src/validation.rs)) and public `site_bounds` ([`variant.rs:400-418`](../../crates/archy-core/src/variant.rs)) call it. The parsed-brief and public-spec regression passed ([`analytic.rs:240-254`](../../crates/archy-eval/tests/analytic.rs)). Reserving `target_gfa` even for soft targets avoids a later hard-target collision. |
| Length-to-area/GFA parity budget, rigid transform policy, and separate absolute hard-target tolerance | **ADDRESSED** | The handoff states the ε-per-axis coordinate model, area bound `εP₁ + nε²`, storey multiplier, transformed-axis policy and ordered rigid transform ([`task-02-handoff.md:48-55`](task-02-handoff.md)). The evaluator computes these values, recomputes the budget after rotation, and checks coordinate, area, GFA and transform-area deltas ([`archy-eval/src/lib.rs:27-137`](../../crates/archy-eval/src/lib.rs)). The hard GFA target still uses the separate absolute `TOLERANCE_M2` ([`archy-eval/src/lib.rs:236-258`](../../crates/archy-eval/src/lib.rs)). Size/storey, translated, rotated, and hard-target coexistence regressions passed ([`analytic.rs:283-384`](../../crates/archy-eval/tests/analytic.rs)). This is an analytic comparison policy; backend geometry parity remains unrun. |
| Negative zero in stored footprint | **ADDRESSED** | `Variant::validate` now compares every stored and derived coordinate by `to_bits()` ([`crates/archy-core/src/variant.rs:353-369`](../../crates/archy-core/src/variant.rs)). JSON deserialization and in-memory negative-zero mutations both fail in the passing regression ([`analytic.rs:256-281`](../../crates/archy-eval/tests/analytic.rs)). |
| Named analytic rectangle/L fixture | **ADDRESSED** | [`tests/fixtures/analytic/courtyard-shapes.json`](../../tests/fixtures/analytic/courtyard-shapes.json) records both polygons and expected area, GFA, height, and four clearances; the passing fixture test asserts those values ([`analytic.rs:386-453`](../../crates/archy-eval/tests/analytic.rs)). |
| Changed geometry changes canonical ID | **ADDRESSED** | The passing test constructs otherwise identical rectangles with widths 10 and 11 m and asserts different IDs and footprints ([`crates/archy-generate/tests/generation.rs:77-108`](../../crates/archy-generate/tests/generation.rs)). |
| Translated generation bounds | **ADDRESSED** | The passing test translates the site by `[10,-5]`, checks three variants, changed IDs, translated footprints, bounds, and evaluation results ([`generation.rs:110-136`](../../crates/archy-generate/tests/generation.rs)). |

**New breakage introduced by these fixes:** none found in the scoped code and tests. The parity comparator rejects malformed polygons and nonfinite transforms, uses the transformed expected polygon for its rotated budget, and recomputes its own budget rather than trusting a serialized report field. This is a source and synthetic-test conclusion, not proof for Blender/IFC artifacts or extreme coordinate ranges.

## Independent validation

Run in `repository` on this snapshot, all exit 0:

- `cargo test --locked -p archy-eval --test analytic`: 12 passed, 0 failed.
- `cargo test --locked -p archy-generate --test generation`: 4 passed, 0 failed.
- `python3 scripts/check-schemas.py`: six schema artifacts valid and source matching; actual Rust serializer output conforms; golden normalized output and 26 negative CLI fixtures conform.

The implementer reports `cargo fmt --all -- --check`, workspace Clippy with `-D warnings`, and full workspace tests passing (CLI 5, core 12, evaluation 12, generation 4). I did **not** rerun those broad commands in this round; their outcomes are reported evidence, not independent validation here. Actual Omarchy and backend parity checks remain outside this round1 rereview.
