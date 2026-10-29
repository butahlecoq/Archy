# A05 report crate independent rereview, round 1

Date: 2026-10-29. **Scoped verdict: PASS.** The three findings in `report-review-final.md` are addressed in the current report crate and fixture. This review changed only this document; no implementation, commit, push or agent action was performed. The full A05 CLI/storage integration review remains separate.

## Original findings

1. **Operation and provenance binding — resolved for the report boundary.** `validate_context` accepts only `generate`, `edit` and `evaluate`; each has its own parent, count and attempt rules and a fixed supported algorithm version. Every variant must use `archy_generate::ALGORITHM_VERSION`. `render_report` still checks canonical variants, project association, one evaluation per variant and equality to a fresh evaluation. New tests reject `approved`, an unknown operation, a mismatched context algorithm and an unsupported variant algorithm, and accept CLI shaped edit/evaluate contexts. The crate still cannot authenticate run IDs, parent ancestry, or hash contents supplied as strings; the caller must bind these to verified storage bytes and manifest metadata.
2. **Empty/count/attempt/exhaustion consistency — resolved.** Generate requires count 1..1000, attempts present and at most count × 100, results at most count, and attempts at least results. Partial or empty results require at least one nonblank exhaustion reason; complete results reject reasons. Edit requires one result, count one, parent run and variant, and no attempts/reasons. Evaluate requires a parent run, one or more results, and no count/attempt/reason fields. Regression tests cover contradictory count, missing/too few attempts and absent/blank reasons. Legitimate empty generation with a reason remains representable.
3. **Markdown check units — resolved.** Site containment and height/setback checks render `m`; hard target GFA renders `m2`, for both measured and limit values. The mixed length/area regression passes. The mapping is explicit from internal check IDs or declared constraint kinds, consistent with the current evaluator contract.

The exporter now calls `generate_variants(&spec, 1, 42)` and evaluates its actual variant. Its report and source-derived schema equal the checked-in fixture and `schemas/report.schema.json`; the report validates against that schema. Illustrative fixture digests and run ID are structural test values, not proof of a stored run. The public `render_report` signature and `ReportContext` fields are unchanged.

## Verification on this snapshot

| Command | Result |
| --- | --- |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo test -p archy-report --locked` | Exit 0; nine integration tests passed, none ignored/failed; doc tests 0 failures. |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo clippy -p archy-report --all-targets --locked -- -D warnings` | Exit 0. |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo fmt --manifest-path crates/archy-report/Cargo.toml -- --check` | Exit 0. |
| Read-only Python check running `cargo run --locked --quiet -p archy-report --example report_contracts` and Draft 2020-12 validation | Exit 0; source schema equals checked-in report schema, full exporter equals fixture, and actual report validates. |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true python3 scripts/check-schemas.py` | Exit 1 at source equality assertion for unrelated `schemas/backend.schema.json`; it did not reach its report-specific assertions in this run. |

The shared checker failure was observed in an earlier moving main-worktree snapshot, not evidence of a report schema mismatch. **Later coordinator relay, not independently rerun in this review:** `RUSTUP_AUTO_INSTALL=0 python3 scripts/check-schemas.py` exited 0 in the current main repository, covering all 12 source-derived schemas, actual backend/report/storage serializers, 26 CLI negative fixtures and the doctor report. The earlier exit-1 observation above is retained for an accurate record; the coordinator's newer evidence supersedes it for current checker status. No broad workspace or Omarchy acceptance claim follows from this scoped rereview.
