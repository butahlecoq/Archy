# A05 report crate handoff

**Scope:** `crates/archy-report/**`, this handoff, and the main repository `Cargo.lock` after the coordinator transferred lockfile ownership. Jason owns schema checker registration; Pasteur owns CLI persistence. No commits, pushes or agents were used in this slice.

## Frozen API for CLI integration

```rust
pub fn render_report(
    spec: &NormalizedSpec,
    variants: &[Variant],
    evaluations: &[EvaluationReport],
    context: &ReportContext,
) -> Result<ReportArtifacts, ArchyError>;

pub struct ReportArtifacts { pub json: Vec<u8>, pub markdown: Vec<u8> }
```

`ReportContext` fields are `schema_version` (`"archy-report-context-v1"`), `run_id`, `parent_run_id: Option<String>`, `parent_variant_id: Option<String>`, `operation`, `input_sha256`, `normalized_sha256`, `algorithm_version`, `build_version`, `requested_count: Option<u32>`, `attempts: Option<u32>`, and `exhaustion_explanations: Vec<String>`. All other string fields are `String`. Pasteur can write the two returned byte arrays as `report.json` and `report.md` in the staged run. The caller must supply real run IDs and hashes; fixture digests in the exporter are illustrative only.

The operation and provenance contract now follows current CLI metadata exactly:

| Operation | Algorithm | Parents | Count / attempts / results | Exhaustion |
| --- | --- | --- | --- | --- |
| `generate` | `archy_generate::ALGORITHM_VERSION` | Neither parent ID | Count 1..1000; attempts present, at most count × 100 and at least results; results at most count | A nonblank reason is required precisely when results are fewer than count. |
| `edit` | `archy_generate::ALGORITHM_VERSION` | Parent run and variant required | Count 1; attempts absent; exactly one result | Empty. |
| `evaluate` | `archy_eval::EVALUATION_VERSION` | Parent run required, parent variant absent | Count and attempts absent; at least one result | Empty. |

All reported variants use the supported `archy_generate::ALGORITHM_VERSION` as their generation algorithm, including variants selected for evaluation. Unknown and authority-implying operations such as `approved`, mismatched versions, and contradictory count fields are rejected. The public render signature and context fields did not change; this is stricter validation of their values. Pasteur's current `publish` context construction matches these cases. The report crate still cannot authenticate run ancestry or hash contents from context strings; CLI/storage must bind them to the accepted manifest and bytes.

`render_report` validates the versioned context and normalized site, checks canonical variant IDs/footprints and project association, requires one distinct evaluation per variant, and compares every supplied evaluation with fresh `archy_eval::evaluate_variant` output. A mismatched stored check, metric, aggregate boolean, method version or professional limit returns `ARTIFACT_INVALID`; no stored boolean grants authority. Variants are ordered by ID in both outputs. Empty variants with explicit nonblank exhaustion details remain reportable. The JSON document includes the normalized spec, declarations and preferences, canonical variant/evaluation pairs, uncomputed daylight/cost labels and professional limits. Markdown escapes untrusted HTML, Markdown punctuation, newlines and code fences. Check measurements show `m` for internal site containment and declared height/setback kinds, and `m2` for internal hard target GFA. Neither format has approval fields or a comparison ranking.

## Schema and fixture contract for Jason

`archy_report::report_schema() -> schemars::Schema` derives the public `ReportDocument` schema. Run `cargo run --locked --quiet -p archy-report --example report_contracts`: stdout is one JSON object with `schemas.report` and `report`. `report` is the actual Rust serializer output of `render_report` over a real `generate_variants` result and fresh evaluations, not a hand-built JSON example. The same payload is at `crates/archy-report/tests/fixtures/report-contracts.json`. Jason's shared checker registration and `schemas/report.schema.json` are unchanged by this fix.

## Validation and limits

- Review fix TDD evidence: the new provenance, count, operation, and unit tests failed against the prior renderer before implementation. The first guarded run after implementation exposed stale fixture and two test assumptions; both were corrected and the exporter regenerated.
- `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo test -p archy-report --locked`: **9 integration tests passed**, 0 failed; doc tests had 0 tests. Regression cases cover unsupported operations/algorithms, count and exhaustion contradictions, CLI edit/evaluate contexts, and mixed length/area check units, along with the existing forged-data, ordering, escaping and source-derived fixture checks.
- `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo clippy -p archy-report --all-targets --locked -- -D warnings` and `cargo fmt --manifest-path crates/archy-report/Cargo.toml -- --check`: passed. `python3 scripts/check-schemas.py` passed, including source schema, exporter and fixture agreement.
- `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo test -p archy-cli --locked --test core_workflow -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable`: 14 passed, 0 failed, one filtered. These include report publication through generate, edit, evaluate and exhausted generation. An earlier unrestricted CLI run passed the other CLI targets and these 14 cases, then was interrupted after the unrelated 65-edit history stress test ran for over six minutes at full CPU; that stress test is unverified in this slice.
- `archy-generate` is added as a local dependency solely to use its supported algorithm constant and the real generator in the fixture exporter. No templating crate, backend runtime or worker dependency was added. Following coordinator handoff, `cargo generate-lockfile --offline` refreshed main `Cargo.lock`; later `--locked` checks used that graph.
- The crate verifies typed inputs at render time; it does not open run artifacts, verify storage hashes, establish a trusted run receipt, or publish files. Pasteur/A05 storage integration owns those checks and immutable publication. A14 ranking/approval and A09 worker outputs remain outside this report slice. Target Omarchy execution and independent review remain open gates.
