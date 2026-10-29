# A18 demo independent review, round 2 — 29 October 2026

**Decision:** The two round-one defects are repaired for the current driver. The demo is credible evidence for the partial `init` → `validate` → `generate` → `evaluate` → `compare` journey only. A18 and release 0.1 remain open: the real CLI has no supervised `render`, `export-ifc`, or approval command, and no GLB, PNG, IFC, or human decision was produced. No source, retained project, other worktree, commit, installation, push, or child agent was changed or used in this review.

## Finding

**Low — `docs/cli.md` promises a backend flow the revised driver no longer implements.** Line 7 says that, once `render` is present, the driver will invoke it, require a selected variant, stop with `AWAITING_SELECTION` if absent, and proceed to `export-ifc`. In `scripts/demo.py:106-111`, an advertised `render` command still leads directly to `DEMO STOPPED: UNAVAILABLE: ... no reviewed CLI receipt and artifact contract`; `--export-variant` is parsed but not used. The README's future wording is conditional and the handoff repair section describes the actual boundary correctly. Update `docs/cli.md` when the supervised contract is defined; until then, state that the current driver always stops after comparison even if backend commands are advertised. This is a documentation error, not evidence of a current false export.

## Independent checks

- **Original fake-success attack:** A disposable copy of `tests/demo/review_shim.py` delegated core commands to the specified real binary, advertised `render` and `export-ifc`, and would return exit-0 `{}` for either. With `--export-variant variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b`, the revised driver exited **2** with the reviewed-contract boundary. It invoked neither backend command, printed no `Draft export complete`, preserved two runs, and produced zero GLB/PNG/IFC files.
- **Original malformed evaluation attack:** The same shim removed `all_required_checks_passed` from an otherwise real evaluation receipt. The driver exited **2** with `DEMO STOPPED: ... all_required_checks_passed`, no traceback, and both runs retained. Shim modes `wrong_checks_type`, `duplicate_ids`, `wrong_ids`, and `same_run` also exited 2 without tracebacks or false export success.
- **Additional receipt probes:** Direct calls to the actual `receipt()` function rejected missing/blank run ID; missing, short, duplicate or non-string variant IDs; missing or non-boolean check result; missing, boolean or zero revision; missing or mismatched run path; invalid JSON, array and null top-level JSON. All 16 cases raised `DemoStopped`. The driver also checks evaluation ID order, different run ID and later revision before comparison. Receipt checks establish shape and relationships; they do not independently prove on-disk run contents from an untrusted future CLI.
- **No overwrite:** The six demo tests passed against the real binary, including existing-directory and symlink collisions with intact sentinels. In a separate probe a shim created `comparison.json` with `sentinel` after the real `compare --json` returned. Exclusive `open("x")` stopped the driver with exit 2; the sentinel survived, `comparison.md` was not written, and both runs remained. The CLI's own occupied-target refusal remains the authority for a race at `init`.
- **Real binary integration:** A fresh disposable project run exited **2** at absent `render`. Help, init, validate, generate, evaluate and both compare calls exited 0; two run directories and both comparison files existed. No render/export command ran and no GLB/PNG/IFC file existed. The retained review-fix transcript reports the same boundary.
- **Freeze and inventory:** Current SHA-256 for `scripts/demo.py`, `tests/demo/test_demo.py`, `tests/demo/review_shim.py`, review-fix transcript and review-fix inventory exactly match the five hashes in the handoff repair section. The earlier seven handoff hashes also match. `sha256sum -c` passed **32/32** for each retained project; separate enumeration found exactly 32 regular files in each, no missing or unlisted files and no GLB/PNG/IFC. The source freeze applies to those listed files; this round-two report is new.

## Exact commands

Working directory: `/home/butahlecoq/work/archy/.worktrees/a18-demo-20261029`. Binary: `/home/butahlecoq/work/archy/.cargo-target-a18/debug/archy`.

```sh
/home/butahlecoq/work/archy/.cargo-target-a18/debug/archy --help
ARCHY_BIN=/home/butahlecoq/work/archy/.cargo-target-a18/debug/archy python3 -m unittest discover -s tests/demo -v
sha256sum scripts/demo.py tests/demo/test_demo.py tests/demo/review_shim.py README.md CONTRIBUTING.md docs/cli.md docs/plans/a18-demo-transcript.txt docs/plans/a18-demo-artifacts.sha256 docs/plans/a18-demo-reviewfix-transcript.txt docs/plans/a18-demo-reviewfix-artifacts.sha256
(cd /var/tmp/archy-a18-demo-20261029-final && sha256sum -c /home/butahlecoq/work/archy/.worktrees/a18-demo-20261029/docs/plans/a18-demo-artifacts.sha256)
(cd /var/tmp/archy-a18-demo-20261029-reviewfix && sha256sum -c /home/butahlecoq/work/archy/.worktrees/a18-demo-20261029/docs/plans/a18-demo-reviewfix-artifacts.sha256)
```

The independent attack matrix used this exact disposable shim invocation; `A18_REPRO` is unset for the fake backend case and set to each listed mutation for the receipt cases:

```sh
python3 - <<'PY'
import os, pathlib, subprocess, tempfile
root = pathlib.Path.cwd()
binary = '/home/butahlecoq/work/archy/.cargo-target-a18/debug/archy'
with tempfile.TemporaryDirectory(prefix='a18-round2-') as temp:
    parent = pathlib.Path(temp)
    shim = parent / 'archy-shim'
    shim.write_bytes((root / 'tests/demo/review_shim.py').read_bytes())
    shim.chmod(0o755)
    cases = [('fake_backend_success', None, ['--export-variant', 'variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b'])]
    cases += [(mode, mode, []) for mode in ('missing_checks', 'wrong_checks_type', 'duplicate_ids', 'wrong_ids', 'same_run')]
    for name, mode, extra in cases:
        project = parent / name
        env = dict(os.environ, ARCHY_REAL_BIN=binary)
        if mode:
            env['A18_REPRO'] = mode
        result = subprocess.run(['python3', str(root / 'scripts/demo.py'), '--archy', str(shim), '--project-dir', str(project), *extra], capture_output=True, text=True, env=env)
        print(name, result.returncode, 'DEMO STOPPED:' in result.stderr, 'Traceback' in result.stderr, 'Draft export complete' in result.stdout, len(list((project / 'runs').iterdir())), sum(p.suffix in {'.glb', '.png', '.ifc'} for p in project.rglob('*')))
PY
```

Each matrix row printed `2 True False False 2 0`. The extra receipt and comparison collision probes are reproducible with these commands:

```sh
python3 - <<'PY'
import importlib.util, json
spec = importlib.util.spec_from_file_location('a18_demo', 'scripts/demo.py')
demo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(demo)
base = {'run_id':'run-1','variant_ids':['v1','v2','v3'],'all_required_checks_passed':True,'revision':1,'path':'runs/run-1'}
cases = {'missing_run_id':{'run_id':None},'blank_run_id':{'run_id':''},'missing_variant_ids':{'variant_ids':None},'short_ids':{'variant_ids':['v1','v2']},'duplicate_ids':{'variant_ids':['v1','v1','v3']},'nonstring_id':{'variant_ids':['v1',7,'v3']},'missing_checks':{'all_required_checks_passed':None},'wrong_checks_type':{'all_required_checks_passed':1},'missing_revision':{'revision':None},'bool_revision':{'revision':True},'zero_revision':{'revision':0},'wrong_path':{'path':'runs/other'},'missing_path':{'path':None}}
for name, changes in cases.items():
    try: demo.receipt(json.dumps(dict(base, **changes)), 'evaluate')
    except demo.DemoStopped: print(name, 'stopped')
    else: print(name, 'ACCEPTED')
for name, payload in [('invalid_json','{'),('json_array','[]'),('json_null','null')]:
    try: demo.receipt(payload, 'evaluate')
    except demo.DemoStopped: print(name, 'stopped')
    else: print(name, 'ACCEPTED')
PY
python3 - <<'PY'
import os, pathlib, subprocess, tempfile
root = pathlib.Path.cwd()
binary = '/home/butahlecoq/work/archy/.cargo-target-a18/debug/archy'
with tempfile.TemporaryDirectory(prefix='a18-collision-round2-') as temp:
    parent = pathlib.Path(temp)
    shim = parent / 'shim'
    project = parent / 'demo'
    shim.write_text('''#!/usr/bin/env python3
import os, pathlib, subprocess, sys
args=sys.argv[1:]
result=subprocess.run([os.environ["ARCHY_REAL_BIN"],*args],capture_output=True,text=True)
if args and args[0]=="compare" and "--json" in args and result.returncode==0:
    pathlib.Path(args[args.index("--project")+1]).parent.joinpath("comparison.json").write_text("sentinel",encoding="utf-8")
sys.stdout.write(result.stdout)
sys.stderr.write(result.stderr)
sys.exit(result.returncode)
''')
    shim.chmod(0o755)
    result = subprocess.run(['python3',str(root/'scripts/demo.py'),'--archy',str(shim),'--project-dir',str(project)],capture_output=True,text=True,env=dict(os.environ,ARCHY_REAL_BIN=binary))
    print(result.returncode, result.stderr.strip().splitlines()[-1], (project/'comparison.json').read_text(), (project/'comparison.md').exists(), len(list((project/'runs').iterdir())))
PY
```

All 16 receipt cases printed `stopped`. The comparison collision printed exit 2, `File exists`, `sentinel`, `False`, and two runs. The fresh real-binary integration command was:

```sh
tmp=$(mktemp -d /tmp/a18-real-round2-XXXXXX)
python3 scripts/demo.py --archy /home/butahlecoq/work/archy/.cargo-target-a18/debug/archy --project-dir "$tmp/demo"
```

That last command is expected to exit 2. Use a new absent directory for each replay. This Ubuntu WSL run is development evidence; Omarchy, containment, supervised backend, visual, independent IFC and release gates remain unverified.
