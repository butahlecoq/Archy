# A14 decision contract rereview handoff

Branch `codex/a14-decision-contract`, base `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`. Changes are uncommitted. Root independent review must approve the actual fixes before integration. This handoff is not that review.

## Reproduction and changes

The independent review probe was run against the original implementation: actual evaluator `site_containment` PASS and FAIL were rejected as unknown; after omitting the failing check, `DecisionRecord::validate` returned `Ok(())` and `required_checks_passed()` returned `true`. Neither result established an authorized decision.

The new contract accepts evaluator site and hard GFA checks and binds its claimed check result to digest-matched normalized project, canonical variant and evaluation bytes. It compares projected constraints and checks to those bytes and requires the evaluator's site, declared constraints, required flags and hard target inventory. It derives the claimed result from the individual checks, not the serialized aggregate. IDs, hashes, revision, timestamps and visible reason text have domain checks; generated schemas express format and length restrictions. Real evaluator tests cover the reported empty-constraint reproduction, rehashed hard target omission, and a 5,000-character upstream source preserved in an evaluator explanation. Schema tests drive actual serde plus explicit domain validation and the generated JSON schemas.

The claimed result still reflects untrusted bytes: a caller can forge and rehash the project and evaluation. No evaluator recomputation, input-to-normalized check, comparison-content validation, run membership or head authentication is performed here. A future integration must perform these checks against independently verified history and recomputed evaluation. Schema acceptance is only structural; domain validation and evidence binding are separate mandatory stages. The visible-text rule rejects listed invisible and combining ranges but is not a complete Unicode rendering engine. Serialized actors and decisions do not establish identity, consent or approved state. FAIL/UNKNOWN selection claims remain representable. No decision persistence, final approval, or Omarchy authority proof exists here.

## Round-two schema fix

The independent round-two review found that the published schemas accepted `2^64` for `expected_project_revision` and `timestamp.seconds` while Rust serde rejected it. The schema test reproduced this disagreement against the real serde probe before the fix. Both bounds now come from `u64::MAX` in the source Schemars annotations, and the published schemas include `maximum: 18446744073709551615`. The expanded schema/serde test covers revision `0`, `1`, `u64::MAX`, and `u64::MAX + 1` in proposal and decision records; timestamp seconds `-1`, `0`, `u64::MAX`, and `u64::MAX + 1` in decision records. This fixes representable integer bounds only; schema acceptance still confers no authority.

## Validation

Commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a14-decision-contract`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`.

- `cargo test --locked --offline -p archy-core --test decision_contract -p archy-eval --test decision_contract -p archy-report --test report -p archy-cli --test golden_artifacts`: passed, respectively 4, 4, 9 and 1 tests.
- `python3 tests/decision_contract_schema.py`: 50 serde/domain and schema cases passed.
- `python3 scripts/check-schemas.py`: passed source schema equality, existing serializer and CLI fixtures; doctor reported target acceptance deferred.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check`: passed.

No packages or dependencies were added. Real Omarchy, worker integration, broker authorization and independent rereview were not run. No A14 or release completion is claimed.

## Frozen SHA-256 source hashes

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

Legacy evidence hashes match the review: `Cargo.lock` `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40`; report library `53c5cad1a147b851bcfab84b532d77c0f67d1a358fdc4f5e1087b1897470088b`; templates `7a7023dcca1d92cee89b88fcd96160d57bf95fb41f2c9d7f01233f5c1022343e`; report contract fixture `2454a985022f8bc8d54078dceea17b9cbc07f5862960e1a270106993f37cf18d`.
