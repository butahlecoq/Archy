# A22 read-only desktop status review, round 1

The user needs a panel that reads a selected project's verified run metadata without granting open or launch authority. **Verdict: ISSUES.** The fixed CLI path, bounded transport, and normal native flow work in the reviewed development profile. One P2 response-validation gap lets the panel display a different selected run than the one implied by the page or requested by the user.

## Finding

**P2. The selected run is not bound to the request or page.** [`DesktopModel.js`](../../integrations/omarchy/plugin/DesktopModel.js) validates `runs` and `selected_run.summary` separately, then accepts the response without comparing their run IDs or the selected summary fields. [`DesktopProcess.qml`](../../integrations/omarchy/plugin/DesktopProcess.qml) does not pass its requested `selectedRunId` to that validation. On an initial page request with no `--run`, the published H2 CLI selects the first page run. On an explicit `--run`, it returns the requested run. The panel can instead display an internally inconsistent status document with no error. This is a malformed-response defense gap. I did not observe the frozen published CLI emit such a document, and the panel has no H3 action that could execute the displayed metadata.

I ran the following direct model probe in `/home/butahlecoq/work/archy/.worktrees/a22-desktop`:

```sh
node - <<'JS'
const fs=require('fs'),vm=require('vm');
const m=vm.createContext({});
vm.runInContext(fs.readFileSync('integrations/omarchy/plugin/DesktopModel.js','utf8'),m);
const s=JSON.parse(fs.readFileSync('tests/fixtures/desktop/accepted_status.json'));
const requested=s.runs[1].run_id, wrong=s.runs[0].run_id;
s.selected_run.summary.run_id=wrong;
const p=m.newProbe(7,'status'); m.append(p,'stdout',JSON.stringify(s));
const result=m.complete(p,0,0,7);
console.log(JSON.stringify({requested,returned:result.status?.selected_run?.summary?.run_id,accepted:!!result.status,expected:'reject selected_run whose ID differs from requested --run'}));
JS
```

It printed `requested: 62040078-5b31-46af-95f6-07205fdac442`, `returned: 860e2341-e139-44b1-a49d-2a5e2ceedd23`, and `accepted: true`. The expected result is no accepted status for a response whose selected run differs from the requested run.

I also fed a disposable altered `accepted_status.json` to the existing immutable Quickshell test helper. This command changes only the fixture content supplied to the container:

```sh
python3 - <<'PY'
import json, pathlib, sys
from unittest.mock import patch
sys.path.insert(0, str(pathlib.Path('tests/omarchy/integration').resolve()))
from test_headless_desktop import run_case, ROOT
path = ROOT / 'tests/fixtures/desktop/accepted_status.json'
original = pathlib.Path.read_text
def altered(self, *args, **kwargs):
    text = original(self, *args, **kwargs)
    if self == path:
        status = json.loads(text)
        status['selected_run']['summary'] = status['runs'][1]
        return json.dumps(status)
    return text
with patch.object(pathlib.Path, 'read_text', altered):
    result, argv = run_case('normal')
print(json.dumps({'argv': argv[1], 'first_page_run': result['status']['runs'][0]['run_id'], 'displayed_selected_run': result['status']['selected_run']['summary']['run_id'], 'error': result['error']}))
PY
```

The actual `DesktopProcess.qml` reported the first page run as `860e2341-e139-44b1-a49d-2a5e2ceedd23`, displayed selected run `62040078-5b31-46af-95f6-07205fdac442`, and returned `error: ""`. Its command had no `--run`, so the selected run should have matched the first page run. This proves the gap through the QML component, not just the model function. Bind the response to the in-flight request. For an explicit run, require that ID and its page summary; for the default, require the first page summary or null for an empty page.

## Checks and limits

- `node --test tests/omarchy/integration/desktop_model.test.js` exited 0 with 5 tests. The six opt-in native `NativeDesktop` tests passed. They cover fixed argv with a path containing spaces, selected run, stale-cursor recovery, malformed/flood/stderr failures, capability refusal, and manifest switching.
- The opt-in `PublishedDesktop` test passed against the frozen publication binary mapped read-only at `/usr/bin/archy`. It covered capabilities and genesis status, not an accepted-run selection. The doctor valid-report native test passed for exits 0 and 1. Four static plugin contract tests passed.
- The panel uses one fixed executable, typed argv, cleared environment, explicit absolute manifest input, bounded ASCII JSON, exit-0 and empty-stderr acceptance, and a single in-flight probe with cancellation. H3 report, preview, and Blender buttons remain disabled. The tests and source support these points. The generic Quickshell 0.3.2 / Qt 6.12 container runs on WSL; it does not load Omarchy `Main.qml` with its theme or prove installed package identity or actual Omarchy behavior.

The review follows **Boundary Discipline** by checking the status JSON where the external CLI result enters the panel. **Test Behavior, Not Implementation** led to the actual QML mismatch probe. **Prove It Works** led to fresh model, native, published-CLI, and doctor checks. I used poteto-mode's concise report style and interrogate's run-backed finding rule without child agents, as the user required. I made no product edit, commit, or push.

## Frozen source identity

Base HEAD was `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`. The review covered its modified tracked files and the untracked slice files. These SHA-256 values were read from the A22 worktree before this review document was added:

| Path | SHA-256 |
| --- | --- |
| `docs/omarchy/plugin.md` | `b15d7fbe225260d198ba20a012e2e79513a28f841cc14c43132f4159a9945e18` |
| `integrations/omarchy/plugin/Main.qml` | `13bd99ec055c54c5c276d55e3329c48388e7478c54682c0d5d7d94dbc8583107` |
| `integrations/omarchy/plugin/DesktopModel.js` | `37a8216b44ef953de8b057ff10de639f7cb1cc486242ef4c8065cd14e1648324` |
| `integrations/omarchy/plugin/DesktopProcess.qml` | `604703a066df35cdfe76de6be5c6a35084a802da34a38145fad2238fed15a8af` |
| `tests/omarchy/integration/test_headless_doctor.py` | `512ed5800c21352ab2043150b980c79d020c6a6772727b652d5c1896f0695aa7` |
| `tests/omarchy/integration/test_plugin_contract.py` | `ad42d6fc012b07b068f895acebaca66766d1a297a6e2443df7bb06eafa44a9c7` |
| `tests/omarchy/integration/desktop_model.test.js` | `ca189c964467c9bb7d4727399f917534008bcc2e03784130fe3a69a056cd224c` |
| `tests/omarchy/integration/fixtures/archy-desktop-fixture.sh` | `7db8f4e9e2a9fc8e6c194c6722dc65a6e392668440f570f785a7039392e8ae66` |
| `tests/omarchy/integration/headless/desktop-shell.qml` | `e37118dbab77d95434801850746802111be5e6a41d7d2a51cdbe2851e2ebca71` |
| `tests/omarchy/integration/test_headless_desktop.py` | `683454800a3af39597e7d5d0e6319661c1b0dbb1777da13acad858399472bca0` |
| `docs/plans/omarchy-desktop-status-handoff.md` | `d6419288d7753d446bef3a048a299edbe06d9002010821234a88e840b5083944` |
