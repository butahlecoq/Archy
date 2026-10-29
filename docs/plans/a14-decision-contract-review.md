# A14 decision/proposal domain contract: independent review

**Scoped verdict: ISSUES.** The contract cannot faithfully represent current evaluator checks and accepts impossible bindings and an invisible-only reason. Its schemas also accept simple values rejected by domain validation. No unauthorized approval was demonstrated: this slice has no decision persistence, approved-state projection, or authentication mechanism.

## Verified findings

1. **Required evaluator checks cannot be represented faithfully.** `ReviewEvidence::validate` accepts only checks whose IDs occur in `constraints`. Every evaluation emits the required internal `site_containment` check; a hard GFA target adds `target_gfa`. Neither is a declared `NormalizedConstraint`, and a synthetic site constraint would also have a zero bound rejected by the validator. The probe below passes actual evaluator results into the contract and both PASS and FAIL site checks are rejected as unknown. Omitting a required failing check produces a serde-round-tripped `Select` claim that validates and reports `required_checks_passed() == true`. The future broker must independently verify the actual evaluation; this result does not authorize selection.

2. **Impossible bindings pass structural validation.** The validator checks IDs only for nonblank text. A serialized selection claim containing `../not-a-project`, `../not-a-run`, `../not-a-variant` and revision zero validates. The project parser rejects that project ID, and accepted run receipts require UUID run IDs. This is an invalid contract claim, not a demonstrated path traversal or current storage write.

3. **An invisible-only reason passes.** A selection reason consisting solely of U+200B ZERO WIDTH SPACE validates. It supplies no visible explanation despite the required reason field.

4. **Schema and domain validation diverge.** The proposal schema accepts a whitespace-only reason and a malformed digest that `Proposal::validate` rejects after serde deserialization. Schema acceptance alone therefore does not establish the published domain rules. Express representable constraints in the schema or explicitly expose the required second validation stage; cross-field and evidence verification still require runtime checks.

## Exact reproduction

Run from the worktree. The temporary harness uses the real core and evaluator; Cargo runs offline in the requested isolated target. Python's `jsonschema` checks the published Draft 2020-12 schema.

```sh
python3 - <<'PY'
from pathlib import Path
import json, os, subprocess, tempfile
from jsonschema import Draft202012Validator
repo = Path.cwd()
with tempfile.TemporaryDirectory(prefix='archy-decision-review-') as d:
    p = Path(d)
    (p / 'src').mkdir()
    (p / 'Cargo.toml').write_text(f'''[package]
name = "decision-review-probe"
version = "0.0.0"
edition = "2024"
[dependencies]
archy-core = {{ path = "{repo}/crates/archy-core" }}
archy-eval = {{ path = "{repo}/crates/archy-eval" }}
serde_json = "1"
''')
    (p / 'src/main.rs').write_text(r'''
use archy_core::{parse_project, InputFormat, Variant, ShapeParameters, Check, CheckResult};
use archy_core::decision_contract::*;
fn main() {
    let mut source: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    source["constraints"] = serde_json::json!([]);
    let spec = parse_project(&source.to_string(), InputFormat::Json).unwrap();
    let make = |x| Variant::new(spec.project_id.clone(), 42, "chacha8-u53-massing-v1".into(),
        ShapeParameters::Rectangle { origin_xy_m: [x, 3.0], width_m: 10.0, depth_m: 20.0 }, 3, 3.2).unwrap();
    let inside = archy_eval::evaluate_variant(&spec, &make(3.0)).unwrap();
    let variant = make(-1.0);
    let outside = archy_eval::evaluate_variant(&spec, &variant).unwrap();
    let mut e = ReviewEvidence {
        project_id: spec.project_id.clone(), variant_id: variant.id.clone(),
        source_run_id: "00000000-0000-4000-8000-000000000000".into(),
        expected_project_revision: 1, expected_head_sha256: "a".repeat(64),
        input_sha256: "b".repeat(64), variant_sha256: "c".repeat(64),
        evaluation_sha256: "d".repeat(64), comparison_sha256: "e".repeat(64),
        constraints: vec![], checks: inside.checks.clone(),
    };
    println!("inside={:?} validate={:?}", inside.checks[0].result,
        e.validate().map_err(|x| (x.code, x.field)));
    e.checks = outside.checks.clone();
    println!("outside={:?} required={} validate={:?}", e.checks[0].result,
        e.checks[0].required, e.validate().map_err(|x| (x.code, x.field)));
    e.checks.clear();
    let base = DecisionRecord {
        schema_version: DECISION_SCHEMA_VERSION.into(), evidence: e.clone(),
        actor: ActorRecord { principal: "human".into(), mechanism: "claim".into() },
        timestamp: UnixTimestamp { seconds: 1_800_000_000, nanos: 0 },
        decision: DecisionKind::Select, reason: "review".into(),
    };
    let decoded: DecisionRecord = serde_json::from_str(&serde_json::to_string(&base).unwrap()).unwrap();
    println!("omitted_fail_select={:?} checks_passed={}", decoded.validate(),
        decoded.evidence.required_checks_passed());
    let mut invalid = decoded.clone();
    invalid.evidence.project_id = "../not-a-project".into();
    invalid.evidence.source_run_id = "../not-a-run".into();
    invalid.evidence.variant_id = "../not-a-variant".into();
    invalid.evidence.expected_project_revision = 0;
    let invalid: DecisionRecord = serde_json::from_str(&serde_json::to_string(&invalid).unwrap()).unwrap();
    println!("invalid_bindings={:?}", invalid.validate());
    source["project_id"] = "../not-a-project".into();
    println!("project_parser_invalid_id={:?}",
        parse_project(&source.to_string(), InputFormat::Json).map(|_| ()).map_err(|x| (x.code, x.field)));
    let mut invisible = decoded.clone();
    invisible.reason = "\u{200b}".into();
    let invisible: DecisionRecord = serde_json::from_str(&serde_json::to_string(&invisible).unwrap()).unwrap();
    println!("invisible_reason={:?}", invisible.validate());
    let mut proposal = Proposal { schema_version: PROPOSAL_SCHEMA_VERSION.into(),
        evidence: e.clone(), suggested_reason: " ".into() };
    let raw = serde_json::to_string(&proposal).unwrap();
    let parsed: Proposal = serde_json::from_str(&raw).unwrap();
    println!("blank_reason={:?}", parsed.validate().map_err(|x| (x.code, x.field)));
    println!("proposal_json={raw}");
    proposal.suggested_reason = "review".into();
    proposal.evidence.variant_sha256 = "bad".into();
    let raw = serde_json::to_string(&proposal).unwrap();
    let parsed: Proposal = serde_json::from_str(&raw).unwrap();
    println!("bad_digest={:?}", parsed.validate().map_err(|x| (x.code, x.field)));
    println!("digest_json={raw}");
    let mut guarded = decoded.clone();
    guarded.evidence.constraints = vec![ConstraintEvidence {
        id: "height".into(), kind: archy_core::ConstraintKind::MaxHeight, bound: 12.0,
        unit: "m".into(), source: "brief".into(), authority: "assumption".into(),
    }];
    guarded.evidence.checks = vec![Check { constraint_id: "height".into(),
        result: CheckResult::Unknown, measured: None, limit: Some(12.0),
        explanation: "unknown".into(), required: true }];
    for result in [CheckResult::Unknown, CheckResult::Fail, CheckResult::Warn] {
        guarded.evidence.checks[0].result = result;
        println!("required_{result:?}={:?}", guarded.validate().map_err(|x| (x.code, x.field)));
    }
    guarded.evidence.checks[0].required = false;
    println!("claimed_optional_warn={:?} checks_passed={}", guarded.validate(),
        guarded.evidence.required_checks_passed());
    guarded.timestamp.nanos = 1_000_000_000;
    println!("bad_timestamp={:?}", guarded.validate().map_err(|x| (x.code, x.field)));
}
''')
    env = os.environ.copy()
    env.update(CARGO_NET_OFFLINE='true',
        CARGO_TARGET_DIR='/home/butahlecoq/work/archy/.tools/targets/a14-decision-review',
        CARGO_BUILD_JOBS='2', RUSTUP_AUTO_INSTALL='0')
    result = subprocess.run(['cargo', 'run', '--offline', '--quiet', '--manifest-path',
        str(p / 'Cargo.toml'), '--', str(repo / 'tests/fixtures/briefs/courtyard.json')],
        capture_output=True, text=True, env=env)
    assert result.returncode == 0, result.stderr
    schema = json.loads((repo / 'schemas/proposal.schema.json').read_text())
    Draft202012Validator.check_schema(schema)
    for line in result.stdout.splitlines():
        if line.startswith(('proposal_json=', 'digest_json=')):
            label, value = line.split('=', 1)
            print(label + '_schema_accepts', Draft202012Validator(schema).is_valid(json.loads(value)))
        else:
            print(line)
PY
```

Observed:

```text
inside=Pass validate=Err((InvalidSchema, "checks.constraint_id"))
outside=Fail required=true validate=Err((InvalidSchema, "checks.constraint_id"))
omitted_fail_select=Ok(()) checks_passed=true
invalid_bindings=Ok(())
project_parser_invalid_id=Err((InvalidSchema, "project_id"))
invisible_reason=Ok(())
blank_reason=Err((InvalidSchema, "suggested_reason"))
proposal_json_schema_accepts True
bad_digest=Err((InvalidSchema, "variant_sha256"))
digest_json_schema_accepts True
required_Unknown=Err((ConstraintFail, "checks"))
required_Fail=Err((ConstraintFail, "checks"))
required_Warn=Err((ConstraintFail, "checks"))
claimed_optional_warn=Ok(()) checks_passed=true
bad_timestamp=Err((InvalidSchema, "timestamp"))
```

Expected: actual required evaluator checks are representable and cannot be silently omitted from a passing selection claim; impossible identifier formats and invisible-only reasons fail validation; schema consumers are not told that a blank reason or malformed digest meets the domain contract. The optional-check result is a caller-supplied claim: a future broker must verify actual required flags and check inventory against immutable evidence.

## Source identity and checks

HEAD: `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`. These seven hashes were captured before and after the probes. The five digests supplied in the request, including the corrected proposal schema digest, match. No separate author handoff was found in the available worktree or `.tools` paths.

```text
479351fa3656a83df6be7894ba84147ae1d34b9da22eecd3ad9bca895f3ad071  crates/archy-core/src/decision_contract.rs
fc621799989ecf8f2992bc3261ebff05c0f352befb7a97b3030ef9980890cb0f  crates/archy-core/src/lib.rs
d218aa00c4017f28225b3ee08d4754747dc04275557ace0746b903b254a9551f  crates/archy-core/examples/contracts.rs
fcab298805c1a7457a403e5dcc199419d4e9ad1465ada10115b167ca04ef9778  crates/archy-core/tests/decision_contract.rs
b72a34fa7dc6028ba6a4153a86e0a5581c094c09f3c0634e7dd6bc190c5fa0fa  schemas/proposal.schema.json
4503f6aa120e10647919e5888381c09a050961f21686125a245d98794e4ef71b  schemas/decision-record.schema.json
14fe9bc81f367fc49a140ef6b9fe3fa9ae18871935c965a097fafcbd7e4da4eb  docs/adr/0005-approval-policy.md
```

Legacy snapshots remained unchanged: `Cargo.lock` SHA-256 `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40`; report library `53c5cad1a147b851bcfab84b532d77c0f67d1a358fdc4f5e1087b1897470088b`; report templates `7a7023dcca1d92cee89b88fcd96160d57bf95fb41f2c9d7f01233f5c1022343e`; report contract fixture `2454a985022f8bc8d54078dceea17b9cbc07f5862960e1a270106993f37cf18d`.

With `CARGO_NET_OFFLINE=true`, `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a14-decision-review`, `CARGO_BUILD_JOBS=2`, and `RUSTUP_AUTO_INSTALL=0`, the following passed:

```sh
cargo fmt --all -- --check
cargo test --offline --locked -p archy-core --test decision_contract -p archy-report --test report -p archy-cli --test golden_artifacts
python3 scripts/check-schemas.py
```

The focused tests passed 4 decision contract, 9 report, and 1 golden artifacts test. The golden test verifies existing accepted artifacts and fresh legacy report bytes, mapping dynamic run IDs where needed. The schema script checks source-derived schema equality and existing CLI fixtures; it does not test semantic parity between these new schemas and `validate()`.

Source search found no consumer of `DecisionRecord`, `Proposal`, or `ReviewEvidence` outside this module, tests, and schema exports. With the actual CLI and a disposable project, injecting an `approval` object returned `INVALID_SCHEMA`; invoking `approve` returned an unrecognized-subcommand error. The project bytes stayed unchanged and no `.archy` directory appeared. Serde accepts a syntactically valid record as an untrusted claim; `validate()` is a separate explicit call and does not authenticate actor or referenced bytes. No current operation projects or persists an authorized selection. Actual Omarchy, broker authorization, proposal persistence, decision persistence, and export acceptance remain unverified and open.
