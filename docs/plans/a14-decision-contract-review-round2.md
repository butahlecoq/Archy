# A14 decision contract independent rereview

**Scoped verdict: ISSUES.** The prior evaluator-check and invalid-identifier failures are addressed, but the published schemas still accept integers outside the Rust `u64` range. This is a schema/serde contract mismatch, not an approval or authorization bypass.

## Verified finding: schemas omit the u64 upper bound

Both schemas specify a minimum and the nonstandard annotation `format: uint64` for `expected_project_revision`; the decision schema does the same for `timestamp.seconds`. They do not set `maximum: 18446744073709551615`. A standard Draft 2020-12 validator therefore accepts `18446744073709551616`, while actual serde rejects it before domain validation. The immediately preceding value is accepted by both paths. This contradicts the handoff's claim that the schemas cover representable format restrictions and can cause a schema-valid request to fail at the Rust boundary. The separately required runtime validation remains necessary for cross-field and evidence rules.

Reproduce from the worktree:

```sh
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a14-decision-review CARGO_BUILD_JOBS=2 RUSTUP_AUTO_INSTALL=0 python3 - <<'PY'
import copy, json, pathlib, subprocess
from jsonschema import Draft202012Validator
h = 'a' * 64
e = dict(project_id='courtyard', variant_id='variant-' + h,
    source_run_id='00000000-0000-4000-8000-000000000000',
    expected_project_revision=1, expected_head_sha256=h, input_sha256=h,
    normalized_sha256=h, variant_sha256=h, evaluation_sha256=h, comparison_sha256=h,
    constraints=[], checks=[dict(constraint_id='site_containment', result='PASS',
        measured=2, limit=0, explanation='site', required=True)])
p = dict(schema_version='archy-proposal-v1', evidence=e, suggested_reason='review')
d = dict(schema_version='archy-decision-record-v1', evidence=copy.deepcopy(e),
    actor=dict(principal='alice', mechanism='claim'), timestamp=dict(seconds=1, nanos=0),
    decision='select', reason='review')
cases = []
for kind, base in [('proposal', p), ('decision', d)]:
    for value in [2**64 - 1, 2**64]:
        item = copy.deepcopy(base)
        item['evidence']['expected_project_revision'] = value
        cases.append((kind, 'revision', value, item))
for value in [2**64 - 1, 2**64]:
    item = copy.deepcopy(d)
    item['timestamp']['seconds'] = value
    cases.append(('decision', 'seconds', value, item))
results = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--offline', '--quiet', '-p', 'archy-core',
     '--example', 'decision_contract_probe'],
    input=json.dumps([{'kind': kind, 'value': item} for kind, _, _, item in cases]), text=True))
for (kind, field, value, item), actual in zip(cases, results, strict=True):
    name = 'proposal' if kind == 'proposal' else 'decision-record'
    schema = json.loads(pathlib.Path('schemas', name + '.schema.json').read_text())
    print(kind, field, value, 'schema', Draft202012Validator(schema).is_valid(item),
          'serde_domain', actual)
PY
```

Observed:

```text
proposal revision 18446744073709551615 schema True serde_domain True
proposal revision 18446744073709551616 schema True serde_domain False
decision revision 18446744073709551615 schema True serde_domain True
decision revision 18446744073709551616 schema True serde_domain False
decision seconds 18446744073709551615 schema True serde_domain True
decision seconds 18446744073709551616 schema True serde_domain False
```

Expected: each schema rejects `2**64` for these `u64` fields while accepting `2**64 - 1`.

## Evidence binding and authority boundary

Independent disposable harnesses called the real evaluator and `claimed_checks_passed_against`. An inside-site design returned `Ok(true)`; an outside-site design returned `Ok(false)`. Removing its required site check and rehashing the altered evaluation was rejected. A hard GFA result returned `Ok(false)`, and deleting its GFA check and rehashing was rejected. The focused evaluator tests also cover required flags, altered projections, required UNKNOWN/WARN/FAIL, and a 5,000-character upstream source and explanation.

I also changed the outside-site evaluation's check result to PASS, changed its serialized aggregate to match, and recomputed its digest. The method returned `Ok(true)` for that forged claim although the actual evaluator returned FAIL. This is the documented boundary: matching attacker-controlled bytes does not establish their origin or recompute evaluation. Source search found no production consumer that uses this claim to persist or project approval. A future broker must verify accepted history, input-to-normalized linkage, comparison content, and recomputed evaluation before using the result; it must separately authenticate consent.

The 40 supplied serde/domain/schema cases passed, including invalid IDs, blank and U+200B-only reasons, normal Unicode, and long reason limits. A separate U+E0100-only reason is accepted by the structural schema but rejected by runtime visibility validation, consistent with the ADR's explicit reservation of remaining Unicode visibility rules to runtime.

## Frozen source and verification

HEAD was `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`. The 11 source hashes below matched the handoff both before and after testing. Handoff SHA-256: `70c2857e15781ead41e7c5aec97b3d644ee5deee5b567fb868e825e16f7eb6b2`.

```text
8c4619579b532c91b8a7470074bc26123c62c4796992c14fbff4e3e4b4fcdec0  crates/archy-core/src/decision_contract.rs
fc621799989ecf8f2992bc3261ebff05c0f352befb7a97b3030ef9980890cb0f  crates/archy-core/src/lib.rs
d218aa00c4017f28225b3ee08d4754747dc04275557ace0746b903b254a9551f  crates/archy-core/examples/contracts.rs
2db8ccd801cd00a32a9149ef1110f197881ddfb0b5182bf639c543be7ded9d54  crates/archy-core/examples/decision_contract_probe.rs
c71f8e02fc31b9b8bd6d3f3c45f085d660008a8f43d4a83cf3e17872e89f0bb1  crates/archy-core/tests/decision_contract.rs
acb05f850731b4b6818d6b281aaa389be64c5db7b8dc549e7bab1300556d4085  crates/archy-eval/tests/decision_contract.rs
cf9b0f21d36f3568739c781c2cb28a8031742ad8774be02c2c5f7302ba024d51  tests/decision_contract_schema.py
246426c542df58bb0fa3caf87800a85a2b7f12ffde8e7f62c563df36e10cedfb  schemas/proposal.schema.json
5dc2790f8209d79e0627ed76339d8d383db7ee179310ea7dbebea55f254924a2  schemas/decision-record.schema.json
f634ede637336fa9ada824e9bac7a6891c36a02528fb577f5a293dbb245eb8fd  docs/adr/0005-approval-policy.md
b6f1ba7253352afe4db4f0b6368adaf5a3b5f59a7bf521051bfe739ac7a0152f  docs/plans/a14-decision-contract-review.md
```

Legacy hashes also matched the handoff: `Cargo.lock` `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40`; report library `53c5cad1a147b851bcfab84b532d77c0f67d1a358fdc4f5e1087b1897470088b`; templates `7a7023dcca1d92cee89b88fcd96160d57bf95fb41f2c9d7f01233f5c1022343e`; report fixture `2454a985022f8bc8d54078dceea17b9cbc07f5862960e1a270106993f37cf18d`.

With `CARGO_NET_OFFLINE=true`, `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a14-decision-review`, `CARGO_BUILD_JOBS=2` and `RUSTUP_AUTO_INSTALL=0`, these passed:

```sh
cargo fmt --all -- --check
cargo test --locked --offline -p archy-core --test decision_contract -p archy-eval --test decision_contract -p archy-report --test report -p archy-cli --test golden_artifacts
python3 tests/decision_contract_schema.py
python3 scripts/check-schemas.py
```

The focused suite passed 4 core contract, 4 evaluator contract, 9 report and 1 golden artifact test. The golden test checked existing artifacts and fresh v1 reports, mapping dynamic run IDs where needed. Omarchy, broker authorization, persistence and export acceptance remain open.
