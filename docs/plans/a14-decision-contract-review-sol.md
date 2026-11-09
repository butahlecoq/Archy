# A14 decision contract: independent Sol review after rounds 1 and 2

**Verdict: scoped pass for the contract fixes.** I found no new blocking defect in this data-contract slice. This is not approval of A14, a serialized `DecisionRecord`, or a real human decision. Comparison, decision persistence, trusted history and evaluator recomputation, broker authentication, and Omarchy acceptance remain open.

## Evidence reviewed

I read the updated handoff, both prior independent reviews, ADR 0005, the changed and added source, schemas, tests, evaluator, variant deserializer, identifier validation, release spec, and A14 backlog. HEAD was `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`. All 11 handoff SHA-256 values matched before review; they are listed below and matched again after tests. Handoff SHA-256: `23bc604a792ee8cbc1298de6ec2a7be2236b0a2ad6babb8a35eb625828fbea1f`.

The real evaluator always emits required `site_containment`; it emits required `target_gfa` for a hard GFA target. `ReviewEvidence::validate` requires the site and every declared constraint check. `claimed_checks_passed_against` checks the hard target inventory against the normalized brief, checks digest and projection equality, deserializes the canonical variant through its validating deserializer, and derives its boolean from the individual checks. The evaluator report deserializer separately rejects a contradictory aggregate. Invalid project identifiers fail domain validation. Proposal JSON rejects added actor or decision fields. Source search found no production consumer that persists or projects a `DecisionRecord` as approved state.

In a disposable Rust crate using the real parser and evaluator, an inside-site variant returned `true`, an outside-site variant returned `false`, and a hard GFA variant with failing `target_gfa` returned `false`. Removing and rehashing `site_containment` or `target_gfa` returned `Err((InvalidSchema, "checks"))`. Changing the outside-site FAIL to PASS in both the check and aggregate, then rehashing the altered evaluation, returned `Ok(true)`. This is the documented untrusted-byte limit, not an authorization path: input-to-normalized linkage, comparison contents, run membership, head authenticity, evaluator recomputation, and human consent are still required at a trusted boundary. A malformed `../bad` project ID returned `Err((InvalidSchema, "project_id"))`.

The published Draft 2020-12 schemas and actual Rust serde plus `validate()` agreed at revision `0`, `1`, `u64::MAX`, and `u64::MAX + 1` for both records, and at seconds `-1`, `0`, `u64::MAX`, and `u64::MAX + 1` for decisions. Both reject U+200B-only text; both accept visible `工程 🏠` and `e` plus combining acute. Two parser-stage distinctions remain: JSON Schema regards JSON `1.0` as an integer, while serde rejects it for `u64`; the schema accepts U+E0100-only text, while runtime visibility validation rejects it. JSON Schema cannot distinguish the numeric spelling `1.0` from `1`; ADR 0005 explicitly requires serde and runtime validation after schema validation and reserves remaining Unicode visibility rules for runtime. Neither discrepancy grants authority or defeats the mandatory runtime checks.

## Commands and results

All Cargo work used fresh `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a14-decision-review-sol`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, and `RUSTUP_AUTO_INSTALL=0`.

```sh
cargo test --locked --offline -p archy-core --test decision_contract -p archy-eval --test decision_contract -p archy-report --test report -p archy-cli --test golden_artifacts
# 4 core, 4 evaluator, 9 report, 1 golden workflow: all passed
python3 tests/decision_contract_schema.py
# 50 serde/domain and schema cases passed
python3 scripts/check-schemas.py
# source schemas, Rust serializers, fixtures and CLI checks passed; doctor target acceptance deferred
cargo fmt --all -- --check
git diff --check
# both passed
```

I also ran a standalone Python here-doc against `debug/examples/decision_contract_probe` and `jsonschema.Draft202012Validator` for the exact integer and Unicode matrix above, and a disposable `/tmp/archy-sol-review-*` Rust crate via `cargo run --offline --quiet --manifest-path .../Cargo.toml` for the real evaluator and rehash probes. The disposable crate was removed automatically. No implementation file was edited.

## Frozen source SHA-256

```text
71b559f93c5547578c06880a61becf3798ea9fd2fd69716676dcc27137f87d57  crates/archy-core/src/decision_contract.rs
fc621799989ecf8f2992bc3261ebff05c0f352befb7a97b3030ef9980890cb0f  crates/archy-core/src/lib.rs
d218aa00c4017f28225b3ee08d4754747dc04275557ace0746b903b254a9551f  crates/archy-core/examples/contracts.rs
2db8ccd801cd00a32a9149ef1110f197881ddfb0b5182bf639c543be7ded9d54  crates/archy-core/examples/decision_contract_probe.rs
c71f8e02fc31b9b8bd6d3f3c45f085d660008a8f43d4a83cf3e17872e89f0bb1  crates/archy-core/tests/decision_contract.rs
acb05f850731b4b6818d6b281aaa389be64c5db7b8dc549e7bab1300556d4085  crates/archy-eval/tests/decision_contract.rs
ab5d4ae87545ca61edc0afc7c34d91eee84c1c4fb43b5fd7fe9ac53712b2772d  tests/decision_contract_schema.py
c424d19cf37c74cece9e9a62e6e0ff8051c7230456dfceb2c7da7cbcf05805ef  schemas/proposal.schema.json
5a59a0d06d30ec49a09cd2c0e5839d3a21baad409af441e03b0fe9135b3283ed  schemas/decision-record.schema.json
f634ede637336fa9ada824e9bac7a6891c36a02528fb577f5a293dbb245eb8fd  docs/adr/0005-approval-policy.md
b6f1ba7253352afe4db4f0b6368adaf5a3b5f59a7bf521051bfe739ac7a0152f  docs/plans/a14-decision-contract-review.md
```

Legacy snapshots stayed unchanged: `Cargo.lock` `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40`, report library `53c5cad1a147b851bcfab84b532d77c0f67d1a358fdc4f5e1087b1897470088b`, templates `7a7023dcca1d92cee89b88fcd96160d57bf95fb41f2c9d7f01233f5c1022343e`.
