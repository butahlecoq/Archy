# A03/A04 acceptance gap handoff

**Superseded:** Independent review reproduced a historical-run verification regression and missing reasons on completed runs. See `a03-a04-gaps-review.md` and the follow-up handoff for the corrected design and evidence. The compatibility claim in this initial handoff was incorrect.

Source: isolated `codex/a03-a04-gaps` worktree at `6ff5d47d4ea3ff9dc30200ac0283c3eb2ee2f3a8`. Read `AGENTS.md`, `docs/specs/release-0.1.txt`, A03/A04 in `docs/plans/backlog.md`, and the independent `a02-a04-acceptance-review.md` from the `a02-a04-audit` worktree. The baseline audit used `cc48bc50da42b8e244a704cf493ba37d6bb955fd`; its unverified paths were hypotheses, not accepted defects.

## Plan and decision

1. Trace parser, public normalized-spec, generator, variant constructor, evaluator, and report consumers. Reproduce each suspected branch using supported inputs.
2. Add seeded tests for reachable construction failures and account for exhaustion without altering successful variant manifests or the JSON shape.
3. Test and document the A04 evidence boundary. Preserve the existing typed error for malformed or unmeasurable analytic geometry; reserve UNKNOWN for an actual check whose evidence is absent.
4. Run focused tests, schema/golden verification and scoped formatting checks. Independent Sol review and Omarchy/backend gates remain separate.

## Findings

- **A03 signature failure is reachable from parsed JSON.** A 2.1 µm wide, 35 m deep site with only L shapes and no setback passes project normalization. At seed 42, all 100 bounded attempts fail geometric construction because sampled widths/notch legs are below the supported 1 µm resolution. Before this change, the result said only that zero footprints were found. It now groups rejection counts by the actual constructor error message. These are failed *attempts to construct* a candidate: there is no canonical footprint, variant ID or immutable snapshot to retain.
- **A03 `Variant::new` failure is reachable through the public normalized-spec entry point.** A directly supplied 1e153 m square site with 10,000 storeys passes the current `site_bounds` checks but its sampled GFA overflows in `Variant::validate`. This public struct is outside the JSON parser's finite-capacity policy, so the test does not claim that the same input is a legal project file. Exhaustion now records construction-failure counts and the overflow reason. No core schema was changed.
- **A03 valid but failing designs remain variants.** The existing height-cap regression generates three variants and evaluates each as a required-check failure. Generation does not filter them. Duplicate suppression remains bounded; the exhaustion explanation now also counts duplicates if any occur.
- **A04 missing measurement cannot be produced by the supported analytic evaluator for a valid variant/spec pair.** `Variant::validate` and `site_bounds` precede measurement. All currently declared checks use computed area, height or signed clearances and declared limits; every resulting check has measured and limit values. Nonfinite arithmetic and malformed geometry return `INVALID_GEOMETRY` before report construction. The new focused test asserts complete evidence for a valid report and the typed error for a corrupted footprint. A synthetic required UNKNOWN remains covered by the existing aggregation/deserialization tests, but it is not an end-to-end evaluator case. Emitting UNKNOWN for these invalid inputs would falsely present an evaluation report for untrusted geometry.

## Compatibility and scope

Only `exhaustion_explanations` text changes on exhausted generation results. `GenerationResult` fields, successful seed-42 variant JSON, IDs, RNG consumption, `ALGORITHM_VERSION`, and evaluation report JSON are unchanged. Historical run artifacts are not rewritten. The enriched text is a diagnostic for newly generated exhausted runs; consumers that display the explanation can show it without a schema migration. No dependency was added. Production edits are limited to `crates/archy-generate`; tests are in that crate and `crates/archy-eval`.

## Evidence and limits

Environment for Rust commands: `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a03-a04-gaps`. Worktree: `/home/butahlecoq/work/archy/.worktrees/a03-a04-gaps`.

- Red: `cargo test --locked -p archy-generate --test generation legal_narrow_l_site_explains_unrepresentable_samples -- --exact` exited 101 on the missing explicit rejection explanation.
- Green after fix: `cargo test --locked -p archy-generate -p archy-eval` exited 0; generation 6/6 and analytic 13/13, plus doc tests. The narrow L case reports all 100 attempts; the public-spec overflow branch also runs for 100 bounded attempts.
- `git diff --check` exited 0. `cargo fmt -p archy-generate -p archy-eval` exited 0.
- `cargo fmt --all -- --check` exited 0.
- `cargo clippy --locked -p archy-generate -p archy-eval --all-targets -- -D warnings` with the same offline/Rust environment exited 0.
- `python3 scripts/check-schemas.py` with the same offline/Rust environment exited 0: all 14 committed schemas matched source, actual Rust generation/evaluation serializers conformed, golden normalized CLI output matched, and all 26 CLI negative fixtures conformed. Doctor returned its expected exit 1; target acceptance remains deferred.

Frozen changed Rust source/test SHA-256 identities: `crates/archy-generate/src/lib.rs` = `65d68c284f06ee8895587ac99b9dd0aae4a0743671313dd8366accfc4e070d89`; `crates/archy-generate/tests/generation.rs` = `24cffa5b2874654d9311a619567021e4daf10c8f1015cb332592ce64e2626d08`; `crates/archy-eval/tests/analytic.rs` = `1ccfb5d968004d69c1c1cd6934991cf055168d040db2c1514559387f0a8072fe`.

No real Omarchy, Blender/IFC geometry, backend parity, full workspace suite or independent Sol review is claimed here. Independent Sol review should inspect the rejection accounting, the public-spec validity distinction, and the A04 UNKNOWN policy before accepting this handoff.
