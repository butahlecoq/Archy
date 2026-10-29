# A03/A04 generation compatibility follow-up handoff

Base commit: `6ff5d47d4ea3ff9dc30200ac0283c3eb2ee2f3a8`. This worktree keeps the original uncommitted A03/A04 implementation and addresses both issues in `a03-a04-gaps-review.md`. No commit or push was made. Independent Sol medium review is still required.

## Contract and decisions

- `generate_variants_v1` reproduces the published v1 generation result, including the old exhaustion wording and omission of diagnostics. Run verification selects this implementation for a v1 manifest and compares the canonical JSON bytes. The captured exact-base exhausted run in `legacy-exhausted-run.json` contains its original project, revision, manifest, generation result, report, and event files; the CLI regression restores and verifies those bytes without rewriting them.
- New generation records construction rejections by phase, error code, field, message, and count; it also counts duplicate footprints. When any attempt is rejected or duplicated, the generation and manifest use `chacha8-u53-massing-diagnostics-v2` and carry `attempt_diagnostics`. Successful variants retain their v1 canonical identity and `generation_algorithm`, because the sampling and variant construction rules did not change. New runs with no rejected or duplicate attempt remain byte identical to v1 generation output.
- Completed batches keep `exhaustion_explanations` empty. Both completed and exhausted batches expose attempt diagnostics in `generation.json`, `report.json`, and the Markdown report. Report validation checks that accepted, rejected, and duplicate attempt counts sum to `attempts`; the verifier recomputes the exact generation result. Historical v1 reports deserialize with the optional field absent. Generation and report schemas plus the report contract fixture were regenerated from their Rust examples.
- A04 behavior is unchanged: valid supported analytic checks have measured values and limits; malformed or unmeasurable geometry returns `INVALID_GEOMETRY`, and the report cannot claim a fabricated pass.

## Fresh validation

Commands used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a03-a04-gaps` from this worktree.

- `cargo test --locked -p archy-generate -p archy-report -p archy-cli --test core_workflow`: 18 passed before the added exhausted-run test. This includes the captured historical run and completed-run diagnostics tests.
- `cargo test --locked -p archy-cli --test core_workflow exhausted_generation_retains_rejection_reasons_in_run_and_report -- --exact`: 1 passed after the added test.
- `cargo test --locked -p archy-generate -p archy-eval -p archy-report -p archy-cli --test generation --test analytic --test report --test golden_artifacts`: 6 generation, 13 analytic, 9 report, and 1 golden test passed.
- `python3 scripts/check-schemas.py`: exited 0; 14 schemas matched source, serializer and fixture checks passed, 26 negative CLI fixtures conformed. Doctor's expected exit 1 reported target acceptance deferred.
- `cargo clippy --locked -p archy-generate -p archy-eval -p archy-report -p archy-cli --all-targets -- -D warnings`: exited 0.
- `cargo fmt --all` and `git diff --check`: exited 0.

The initial schema check failed only because `report-contracts.json` still held the old source schema. The fixture was regenerated from `archy-report --example report_contracts`, and the full schema check then passed. No Omarchy or real backend gate is claimed.

## Frozen changed file SHA-256

```text
7bf35f12f55e99dca519764ec984640b8d855e711e580939f85a8ac0f857be87  crates/archy-generate/src/lib.rs
b6bfa9153f09695d6b6047ecbd7428083f7987e035b946d92e2bed716119583b  crates/archy-generate/tests/generation.rs
1ccfb5d968004d69c1c1cd6934991cf055168d040db2c1514559387f0a8072fe  crates/archy-eval/tests/analytic.rs
a9829c69219f68869138a4b12574667648cebd216da2ba771a01b0aa7ff22637  crates/archy-cli/src/generate.rs
33e1a14582ba822955ef546868508335a2e3c4ed2626826ee1b5edb027298068  crates/archy-cli/src/lib.rs
27756b6d3fecb92deb8a739204d261d6c2affda0fe3242fd2420bccf29a9ba8b  crates/archy-cli/src/run_integrity.rs
7bacad332df1c6d6360e3f3c582f0b49ee275130c5500d40ae0aeff2f4e61b46  crates/archy-cli/tests/core_workflow.rs
3361743ebbc4dc390fac066aa0e6eb3071efe24c8083172d6e3a935b8367dd3d  crates/archy-cli/tests/fixtures/legacy-exhausted-run.json
f3df5cede5dd6c414780daacf8af138e5f444681d312502e0a4ecb31a74e5906  crates/archy-report/src/lib.rs
0c890632b944d4c8a72826ea44d6c65ff44ecbfd521fa016e7d2d1141e1f4048  crates/archy-report/src/templates.rs
f8ca968a0fca8541ac81779eceb49b632ccaae1b1d4359a0a44a84cfc6317a47  crates/archy-report/examples/report_contracts.rs
d9944f4cdd5c30510a12372839f81148c19deadaa7a363f5563bbb1de290c998  crates/archy-report/tests/report.rs
d1d8ba793da56a0132331fa9b98c7ba989cddf96c415486aebf4b07ff000e15f  crates/archy-report/tests/fixtures/report-contracts.json
1c939cd29337335612ff74eca86ebe389a87cdb87d3fc85cac1ebfa07c25cbe2  schemas/generation.schema.json
33fb78152bffb9b2f7962c297af69d4cd2f692a578fce319f42a72a8d26b59de  schemas/report.schema.json
```

Review focus: confirm the v1 byte identity against the captured run, v2 version choice and attempt-count invariant, and the boundary between rejection diagnostics and immutable accepted variants. The original handoff's compatibility claim is superseded by this one.
