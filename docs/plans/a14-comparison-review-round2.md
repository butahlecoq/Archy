# A14 passive comparison: independent review, round 2

**Verdict: PASS for the scoped passive comparison rereview.** The round 1 U+202E finding did not reproduce after the correction. No new verified issue was found in the requested scope. Human approval, export completion, and real Omarchy acceptance remain open.

## Frozen source

Worktree HEAD was `4cfd0b82e86c1fa57fb1611eb8f844f3df586cd0`. Before and after the probes, running the following from this worktree returned `OK` for all **18** entries: 17 source, fixture, and review entries, plus the handoff itself.

```sh
sha256sum -c /home/butahlecoq/work/archy/.tools/targets/a14-compare/a14-comparison-handoff.sha256
```

Selected SHA-256 values, including the manifest that supplies the complete list:

```text
4a95d155bb5b0cfda4ab70f4c62d507ce2948371bb84c89cf5e94764a49f1ea7  /home/butahlecoq/work/archy/.tools/targets/a14-compare/a14-comparison-handoff.sha256
91534af196de162205de77aa39e93c9baa3e42bc17b0f3ac6bcd1b23be8bcc81  docs/plans/a14-comparison-handoff.md
4bfe49fbe94f2e0dda3ba74e5cca8d65ca961bd751e876bb95d5425dca96ce05  crates/archy-report/src/compare.rs
7bd9313d4637ee500c5532a6b6fb235fe714f060f6fbc4e810abe8e92396e916  crates/archy-report/tests/compare.rs
ccd8151bff74593a42030ac91d6b7be4b6b6593a020163a06a76f4fe292d6cc8  crates/archy-cli/tests/compare.rs
796705db31a220c7ecaac810987e164423b0d32cb535182377a7b47c72acce97  crates/archy-report/tests/fixtures/comparison.schema.json
7a7023dcca1d92cee89b88fcd96160d57bf95fb41f2c9d7f01233f5c1022343e  crates/archy-report/src/templates.rs
2454a985022f8bc8d54078dceea17b9cbc07f5862960e1a270106993f37cf18d  crates/archy-report/tests/fixtures/report-contracts.json
```

## Independent actual CLI probe

Run from the worktree after the focused Cargo test below builds the binary. This fixture includes U+202E, directional isolates, U+2028/U+2029, unsafe Markdown, ordinary Arabic and accented text, and an emoji joined with U+200D. It creates two batches and a later evaluation, edits the current project file, and verifies source and artifact digests against the selected stored run. It snapshots files and directories before comparison, including the existing v1 reports and revision state.

```sh
python3 - <<'PY'
import hashlib, json, pathlib, stat, subprocess, tempfile
repo = pathlib.Path.cwd()
binary = '/home/butahlecoq/work/archy/.tools/targets/a14-review/debug/archy'
sha = lambda b: hashlib.sha256(b).hexdigest()
with tempfile.TemporaryDirectory(prefix='archy-a14-r2-') as d:
    root = pathlib.Path(d)
    project = root / 'project.json'
    source = json.loads((repo / 'tests/fixtures/briefs/courtyard.json').read_text())
    authority = 'user\u202e:devorppa\u2066isolate\u2069'
    origin = 'source\u2028line\u2029para [link](javascript:alert(1)) | <script>'
    preference = 'café العربية 👩\u200d💻 \u2067RTL\u2069'
    source['constraints'][0].update(authority=authority, source=origin)
    source['preferences'] = [preference]
    raw = json.dumps(source, ensure_ascii=False).encode()
    project.write_bytes(raw)
    def cli(*args):
        result = subprocess.run([binary, *args], capture_output=True, text=True)
        assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
        return result.stdout
    def inventory():
        out = {}
        for path in root.rglob('*'):
            metadata = path.lstat()
            out[str(path.relative_to(root))] = (
                stat.S_IMODE(metadata.st_mode), metadata.st_size,
                sha(path.read_bytes()) if path.is_file() else 'dir')
        return out
    def generate(seed):
        return json.loads(cli('generate', '--project', str(project), '--count', '3', '--seed', str(seed), '--json'))
    first = generate(42)
    second = generate(43)
    cli('evaluate', '--project', str(project), '--all', '--json')
    project.write_text('{"edited":true}')
    before = inventory()
    markdown = cli('compare', '--project', str(project))
    latest = json.loads(cli('compare', '--project', str(project), '--json'))
    old = json.loads(cli('compare', '--project', str(project), '--run', first['run_id'], '--json'))
    controls = [0x202e, 0x2066, 0x2069, 0x2067, 0x2028, 0x2029]
    assert all(chr(c) not in markdown and f'\\[U+{c:04X}\\]' in markdown for c in controls)
    assert 'café العربية 👩\u200d💻' in markdown
    assert '<script>' not in markdown and '[link](javascript:' not in markdown and '&lt;script&gt;' in markdown
    assert latest['constraints'][0]['authority'] == authority
    assert latest['constraints'][0]['source'] == origin
    assert latest['subjective_preferences'] == [preference]
    assert latest['source_run_id'] == second['run_id'] and old['source_run_id'] == first['run_id']
    assert latest['observed_revision'] == 3 and latest['source_input_sha256'] == sha(raw)
    run = root / second['path']
    assert latest['source_manifest_sha256'] == sha((run / 'manifest.json').read_bytes())
    assert latest['source_normalized_sha256'] == sha((run / 'normalized.json').read_bytes())
    for candidate in latest['candidates']:
        variant = candidate['variant']['id']
        assert candidate['variant_sha256'] == sha((run / 'variants' / f'{variant}.json').read_bytes())
        assert candidate['evaluation_sha256'] == sha((run / 'evaluations' / f'{variant}.json').read_bytes())
        delta = next(m for m in candidate['evaluation']['metrics'] if m['name'] == 'target_gfa_delta')
        assert candidate['objective']['value'] == abs(delta['value']) and candidate['objective']['unit'] == 'm2'
    assert latest['constraints'][0]['authority_status'] == 'unverified declaration'
    assert latest['uncomputed'] == ['daylight', 'cost']
    assert before == inventory()
    print('markdown_controls=marked ordinary_unicode=preserved unsafe_markup=escaped')
    print('json_original=exact default_and_historical=correct provenance_digests=match revision=3 candidates=' + str(len(latest['candidates'])))
    print('project_inventory_and_legacy_reports=byte_identical approval_state=unchanged')
PY
```

Observed:

```text
markdown_controls=marked ordinary_unicode=preserved unsafe_markup=escaped
json_original=exact default_and_historical=correct provenance_digests=match revision=3 candidates=3
project_inventory_and_legacy_reports=byte_identical approval_state=unchanged
```

The unchanged inventory establishes that this comparison did not create or alter an approval event or other project state. It does not establish a human authorization boundary.

## Existing v1 evidence and focused checks

An independent binary probe copied `examples/courtyard-housing/expected/accepted-project` into a disposable directory, made the copied managed directories private, and ran `archy compare --project ... --json`. The accepted history compared successfully at revision 5 with one candidate. All ten existing `report.json`/`report.md` files remained byte-identical, and the entire copied file inventory was unchanged. The golden artifacts test also checked stored artifact hashes and fresh v1 report bytes against the saved evidence, mapping only dynamic run IDs.

Commands run from the worktree, with the requested isolated target and no network:

```sh
export CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a14-review
export CARGO_BUILD_JOBS=2
export RUSTUP_AUTO_INSTALL=0
cargo fmt --all -- --check
cargo test --offline --locked -p archy-report --test compare --test report -p archy-cli --test compare --test golden_artifacts
```

Both commands passed: four CLI comparison tests, one golden artifacts test, four report comparison tests, and nine report tests. The CLI comparison tests also cover forged/rehashed evidence and no-history behavior. No product source, main checkout, other worktree, commit, push, or host package was changed during this rereview.
