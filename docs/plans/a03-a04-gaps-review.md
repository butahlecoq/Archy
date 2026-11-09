# A03/A04 audited gaps review

**Verdict: ISSUES.** The frozen change makes previously accepted exhausted generation runs unreadable. It also discards construction-failure reasons when later samples fill the requested count. The focused analytic and golden checks pass.

## P1. Existing exhausted runs fail semantic verification

`crates/archy-generate/src/lib.rs` adds text to `exhaustion_explanations` but keeps `ALGORITHM_VERSION` as `chacha8-u53-massing-v1`. `crates/archy-cli/src/run_integrity.rs` recomputes the entire `GenerationResult` and requires its canonical bytes to equal the stored `generation.json`. Consequently a valid run saved by the prior implementation fails verification after this change. This also affects readers and operations that verify that run, even though its files and hashes remain intact.

I reproduced this on a disposable project using the exact-base binary and the frozen candidate binary. The base binary was built offline from a disposable `git archive` of `6ff5d47d4ea3ff9dc30200ac0283c3eb2ee2f3a8`, with two jobs and the separate target `/home/butahlecoq/work/archy/.tools/targets/a03-a04-review/base-snapshot`. Run from this worktree:

```sh
python3 - <<'PY'
import json, pathlib, tempfile, subprocess
old = pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a03-a04-review/base-snapshot/debug/archy')
new = pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a03-a04-review/debug/archy')
raw = json.loads(pathlib.Path('tests/fixtures/briefs/courtyard.json').read_text())
raw['site']['width_m'] = 0.0000021
raw['constraints'] = []
raw['generation']['shapes'] = ['l_shape']
with tempfile.TemporaryDirectory(prefix='archy-a03-legacy-') as d:
    project = pathlib.Path(d)/'project.json'
    project.write_text(json.dumps(raw))
    made = subprocess.run([old, 'generate', '--project', project, '--count', '1', '--seed', '42', '--json'], capture_output=True, text=True, check=True)
    print('base generation', json.loads(made.stdout)['exhaustion_explanations'])
    for label, binary in [('base', old), ('candidate', new)]:
        result = subprocess.run([binary, 'desktop', 'status', '--project', project, '--json'], capture_output=True, text=True)
        output = json.loads(result.stdout)
        print(label, 'exit', result.returncode, 'code', output.get('code', 'SUCCESS'), 'message', output.get('message', ''))
PY
```

Observed: base generation exited 0 with `Only 0 distinct representable canonical footprints found in 100 bounded attempts`; base desktop status exited 0. Candidate desktop status exited 1 with `ARTIFACT_INVALID` and `generation result differs from supported algorithm`. Expected: the previously accepted run remains readable and its immutable bytes remain valid. Preserve the old algorithm's verification semantics and version the new behavior, or otherwise provide an explicit backward-compatible verifier for the historical result and report bytes. Add a regression that creates a run with the base implementation and reads it with the candidate.

The base build command was `cargo build --locked -p archy-cli --quiet` with `RUSTUP_AUTO_INSTALL=0`, `CARGO_NET_OFFLINE=true`, two jobs, and the separate target above. The project and archived source were disposable and removed after the probe.

## P2. Failed samples disappear when a later sample succeeds

Both rejection maps and the duplicate count are emitted only inside `if result.variants.len() < count`. A legal JSON project with an L-only site width of `0.000003` m, no constraints, count 1, and seed 42 returned one variant after four attempts with `exhaustion_explanations: []`. Before the first success, at least one sample must have failed construction or representation. The result preserves no reason or count for it. This conflicts with A03's requirement to retain invalid candidates with reasons and avoid silent candidate filtering. The existing report contract forbids exhaustion text for complete results, so record attempt diagnostics through an explicitly designed compatible field or separate evidence path rather than presenting them as exhaustion. Add a mixed failure-then-success regression.

## Evidence and limits

The worktree is based on `6ff5d47d4ea3ff9dc30200ac0283c3eb2ee2f3a8`. Its changed inventory consists of the following three Rust files and the untracked handoff. Their SHA-256 values match the freeze:

```text
65d68c284f06ee8895587ac99b9dd0aae4a0743671313dd8366accfc4e070d89  crates/archy-generate/src/lib.rs
24cffa5b2874654d9311a619567021e4daf10c8f1015cb332592ce64e2626d08  crates/archy-generate/tests/generation.rs
1ccfb5d968004d69c1c1cd6934991cf055168d040db2c1514559387f0a8072fe  crates/archy-eval/tests/analytic.rs
```

With `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a03-a04-review`, `cargo test --locked -p archy-generate -p archy-eval` passed 6 generation and 13 analytic tests. Those include the parsed 2.1 µm L site with 100 rejected attempts, the directly supplied 1e153 m normalized spec with 100 construction attempts and overflow reason, repeatability, invalid geometry rejection, and retention of three constructed candidates that fail required height checks. The direct normalized overflow is not asserted to be parser-admissible.

An independently compiled disposable Rust probe called `generate_variants` twice for the directly supplied 1e153 m square and 10,000 storeys. It asserted equality and printed `attempts=100 variants=0 deterministic=true`, followed by `100 variant construction failures: Gross floor area overflow`.

`python3 scripts/check-schemas.py` exited 0, including 14 source schemas, the seed-42 generation/evaluation fixtures, and 26 CLI negative fixtures. `cargo test --locked -p archy-cli --test golden_artifacts` passed and compares saved workflow bytes. These checks establish that the successful seed-42 fixture and golden workflow remain unchanged, not that old exhausted runs remain readable.

A boundary probe with L-only parsed projects at widths of `2e-6`, `2.000000000001e-6`, and `2.1e-6` m returned respectively zero attempts with the resolution explanation, 100 attempts with 7 dimension and 93 notch rejections, and 100 attempts with 6 dimension and 94 notch rejections. The current messages derive from constructor errors; no unrelated error or compliance claim was observed in these cases.

A04 explicitly requires invalid polygons to be rejected and says missing evidence yields `UNKNOWN`. The invalid-footprint test correctly returns `INVALID_GEOMETRY`, not a fabricated successful report. Existing supported checks have measured values and limits, and the required-UNKNOWN aggregation test rejects a forged passing flag. The evaluator does not expose an end-to-end valid check with missing evidence; that part of the broader A04 wording remains unverified. Daylight, structural, access, cost, and local regulatory evidence remain explicitly uncomputed. No Omarchy or backend parity is claimed.
