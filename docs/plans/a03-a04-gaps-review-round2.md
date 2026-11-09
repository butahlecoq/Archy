# A03/A04 versioned followup: independent round two review

**Verdict: scoped PASS.** No verified finding in the versioned A03/A04 followup. I reviewed the complete worktree diff and the captured historical fixture against the two prior review documents. No implementation edit, commit, push, or host install was made.

## Evidence

- **Historical v1:** The captured exact-base exhausted run (`legacy-exhausted-run.json`) passed `verify_run` and the test compared every restored file with its original bytes afterward. The v1 verifier retains the base generator's sampling, suppression, construction, and exhaustion wording. An independent `desktop status` probe accepted the untouched fixture (`exit 0`).
- **Unchanged successful generation:** With the seed-42 courtyard input, the exact-base binary and two candidate invocations produced identical `generation.json` and variant artifact bytes. Each used `chacha8-u53-massing-v1`, three attempts, three variants, and no diagnostics. The golden workflow test passed.
- **v2 diagnostics:** Fresh CLI runs with the legal narrow L-shaped site used `chacha8-u53-massing-diagnostics-v2`. At width `0.000003` m, the completed run had four attempts, one variant, and three footprint rejections. At width `0.0000021` m, the exhausted run had 100 attempts and no variants; ordered rejection groups were 94 notch/remaining-leg errors followed by six dimension errors. In each run, accepted + rejected + duplicates equaled attempts; `report.json` matched `generation.json` diagnostics, Markdown displayed them, and exhaustion text appeared only for the exhausted run. The CLI verifier accepted both generated runs in `core_workflow` tests.
- **Forged provenance and history:** On disposable copies of the historical fixture, changing only the manifest to the supported v2 version and rehashing its revision returned `ARTIFACT_INVALID: generation result differs from supported algorithm`. Changing both manifest and generation to v2, inserting fabricated diagnostics, and updating the generation, manifest, and revision hashes returned the same rejection. Altering the accepted revision receipt's run ID returned `ARTIFACT_INVALID: invalid run receipt`. Existing tests also reject an unsupported algorithm and consistently rehashed artifact forgeries.
- **Report, schema, and A04:** `render_report` requires a v2 diagnostic count balance and nonempty rejection reasons, while historical v1 reports deserialize with diagnostics absent. The source-derived generation and report schemas and serialized fixtures matched. The analytic test confirms measured values and limits for supported valid checks and `INVALID_GEOMETRY` for malformed geometry; no synthetic `UNKNOWN` or passing report is emitted for that invalid input. This does not claim an end-to-end valid check with genuinely missing evidence.

## Commands and scope

All Cargo work used the fresh target `/home/butahlecoq/work/archy/.tools/targets/a03-a04-review-round2`, with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2`.

```sh
cargo test --locked -p archy-generate -p archy-eval -p archy-report -p archy-cli --test generation --test analytic --test report --test core_workflow --test golden_artifacts
python3 scripts/check-schemas.py
git diff --check
```

Results: 19 core workflow, 1 golden, 13 analytic, 6 generation, and 9 report tests passed. Schema checks passed for all 14 source schemas, Rust serializer fixtures, and 26 CLI negative fixtures. Doctor's expected exit 1 reported target acceptance deferred. `git diff --check` passed. I ran `sha256sum` on all 15 frozen changed files listed in the followup handoff; every value matched, including the captured run fixture and both schemas. Temporary projects were removed after the probes.

This verdict covers the versioned followup and listed local checks. It does not claim Omarchy, real backend parity, or full workspace acceptance.
