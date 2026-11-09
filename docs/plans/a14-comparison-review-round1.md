# A14 passive comparison: independent review, round 1

**Verdict: ISSUES for publication of this bounded comparison slice.** One reproducible Markdown presentation defect remains. This review does not assess A14 approval/export completion or Omarchy acceptance.

## Scope and source identity

Reviewed worktree `/home/butahlecoq/work/archy/.worktrees/a14-compare` at base `4cfd0b82e86c1fa57fb1611eb8f844f3df586cd0`. I read `AGENTS.md`, the release specification, backlog A14, the handoff, and the A14 approval research. I independently SHA-256 checked every one of the 14 files in the handoff's frozen source list, including the generated comparison schema, the CLI shared integration files, and the research document: **14/14 match**. The changed file inventory matched the handoff; legacy report source and fixtures were not edited.

## Verified finding

### Unicode directional control survives Markdown escaping

`crates/archy-report/src/compare.rs:174-195` escapes Markdown syntax and `char::is_control()` characters, but U+202E RIGHT-TO-LEFT OVERRIDE is a Unicode format character and passes through. A valid brief can put it in a constraint's declared authority. The comparison Markdown then contains a raw directional override in the authority line, which can visually reorder its evidence label. This is an output trust issue; it does not create approval state.

Reproduce from this worktree using the built binary in the requested target directory:

```sh
python3 - <<'PY'
import json, pathlib, subprocess, tempfile
root = pathlib.Path.cwd()
binary = '/home/butahlecoq/work/archy/.tools/targets/a14-review/debug/archy'
with tempfile.TemporaryDirectory(prefix='archy-a14-bidi-') as d:
    project = pathlib.Path(d) / 'project.json'
    source = json.loads((root / 'tests/fixtures/briefs/courtyard.json').read_text())
    source['constraints'][0]['authority'] = 'user\u202e:devorppa'
    project.write_text(json.dumps(source))
    generate = subprocess.run([binary, 'generate', '--project', str(project), '--count', '1', '--seed', '42', '--json'], capture_output=True, text=True)
    compare = subprocess.run([binary, 'compare', '--project', str(project)], capture_output=True, text=True)
    line = next(x for x in compare.stdout.splitlines() if 'authority:' in x)
    print('generate', generate.returncode, 'compare', compare.returncode)
    print('line_repr', repr(line))
    print('raw_bidi', '\u202e' in compare.stdout)
PY
```

Observed:

```text
generate 0 compare 0
line_repr '- height: MaxHeight, bound 12 m; source: user\\_brief; authority: user\u202e:devorppa (unverified declaration)'
raw_bidi True
```

Expected: the Markdown authority line contains no raw U+202E (for example, a visible escaped marker); `raw_bidi False`. The JSON may preserve the original declaration as data, but the human-facing Markdown must not let it change display direction. Add a regression probe through the rendered Markdown path.

## Checks that passed

- Real binary: generated three candidates, compared in JSON and Markdown, and validated the actual JSON document against `crates/archy-report/tests/fixtures/comparison.schema.json` with Draft 2020-12 validation: zero schema errors. The output carried policy, source run/input/normalized/manifest digests, per-candidate variant/evaluation digests, SI units, metric method versions, declared constraint source/authority marked unverified, subjective preferences, daylight/cost uncomputed, and professional limits.
- Real binary: after two generation batches and a later evaluation run, the default comparison selected the second generation batch; `--run` selected the first. Replacing the current project file with invalid JSON did not change the stored source digest or selection. The comparison left the full project file inventory byte-identical, including old `report.json` and `report.md` artifacts.
- Real binary: with a 5 m maximum height, all three candidates retained objective ranks and visible required `FAIL` checks. A modified old `report.md` returned `ARTIFACT_INVALID` without writes. An evaluation metric changed to `999999` with consistently updated artifact, manifest, and revision receipt digests also returned `ARTIFACT_INVALID` (`stored evaluation differs from full recomputation`) without writes.
- Real binary: `<script>`, ANSI ESC, Markdown link, pipe, and newline text in source/authority/preferences did not survive as executable Markdown or terminal ESC. A reserved `target_gfa` constraint identifier was rejected with `INVALID_SCHEMA`, so the suspected unit collision did not survive reproduction.
- Disposable domain harness: two equal GFA objectives retained dense ranks `[1, 1]`; an absent objective had null value/rank and sorted last; a synthetic `UNKNOWN` check remained visible. A genuine accepted run cannot currently produce UNKNOWN through the evaluator; the synthetic harness tested report behavior only, not storage acceptance.
- `cargo fmt --all -- --check`: pass. `cargo clippy --workspace --all-targets --locked -- -D warnings`: pass. `cargo test --locked -p archy-report --test compare -p archy-cli --test compare`: 6 passed. `cargo test --workspace --locked`: pass, with the pinned IFC worker integration test ignored by default. `python3 scripts/check-schemas.py`: pass. Cargo commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a14-review`, `CARGO_BUILD_JOBS=2`, and `RUSTUP_AUTO_INSTALL=0`; the schema script used the same environment.

The comparison introduces no approval, actor, decision event, selection state, or external publication authority. A14 approval and export remain open; actual Omarchy and worker acceptance are deferred. No product sources, main checkout, other worktrees, commits, pushes, or host packages were changed in this review.
