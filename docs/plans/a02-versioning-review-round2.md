# A02 historical compatibility: independent rereview

**Scoped verdict: PASS.** The previously failing five-revision golden test now passes. The corrected comparison detected added or changed accepted-run files and rejected nonempty transient lock/staging entries in disposable fault probes. The legacy preference and rehashed semantic-forgery tests also passed, and the frozen file hashes match. This accepts only the reviewed historical-compatibility slice; A02's new canonical schema and preference policy remain open. The earlier request for a separate Sol-medium review remains unverified because this session's model identity cannot be independently established as Sol medium.

Base `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`; the reviewed candidate is uncommitted. Read `docs/plans/a02-versioning-freeze.md`, the updated handoff, the prior independent review, and the actual changed test and verifier. No source files, accepted fixture files, commits or remote state were changed.

| Requirement | Verdict and evidence |
| --- | --- |
| Original golden failure | **PASS.** `golden_artifacts` exited 0, 1/1, including the curated five revisions and the subsequent fresh workflow. Previously it failed because semantic verification created `.archy/lock` and `.archy/staging` in the copied tree. |
| No masking of accepted-evidence mutation | **PASS for tested changes.** The test copies the fixture to a baseline and a verification copy before any verification, and compares both against the baseline afterward. In the verification copy only, it checks the lock is an empty regular file and staging is a directory, removes the lock and uses `remove_dir` for staging, which fails if nonempty. It then compares all names and file bytes recursively. Independent injections into a disposable verification copy made the test fail for an added run file, modified accepted report, nonempty lock and nonempty staging. The first failed on the unexpected entry; the modified report failed integrity verification. The tracked fixture's 59-file aggregate stayed equal to the frozen value before and after. These cases do not prove behavior under every possible concurrent filesystem attack. |
| Legacy preferences, source pinning and replay | **PASS, scoped.** `legacy_preferences_keep_exact_v1_bytes_and_survive_source_edits` passed. It verifies the recorded source and normalized hashes, preserves four legacy strings as data in snapshots/reports, verifies after the live source becomes malformed, rejects changed-source replay, restores it and replays the same run and revision. The fixture is generated with byte-identical v1 code, rather than a previously captured nonempty historical run. Acceptance of a string is not evidence it executes. |
| Rehashed forgeries and version labels | **PASS for exercised cases.** The ten-class `consistently_rehashed_forgery_never_grants_trusted_status` test passed; the legacy test rejects a rehashed v2 normalization label. Re-running the previous independent six-case probe against this frozen candidate rejected v2/missing/unknown labels in root, intermediate and selected runs, malformed stored v1 YAML and a hidden stored-source edit with `ARTIFACT_INVALID`. Malformed v1 input was reported as a source error rather than a v2 profile. No v2 implementation is accepted here. |
| Exact v1 source moves and freeze | **PASS.** Both `git show HEAD:<old path> | cmp -s - <v1 path>` checks exited 0. All 12 entries in the freeze match actual SHA-256 values; the candidate source and handoff hashes checked before and after execution were unchanged. The freeze file itself was separately hashed below. |

## Commands and observed results

All Cargo commands ran in this worktree with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-version-review`:

```sh
cargo test --locked --offline -p archy-cli --test golden_artifacts
cargo test --locked --offline -p archy-cli --test core_workflow legacy_preferences_keep_exact_v1_bytes_and_survive_source_edits
cargo test --locked --offline -p archy-cli --test core_workflow consistently_rehashed_forgery_never_grants_trusted_status
```

All three exited 0, each with one passing test. The previous review ran the 17-test workflow suite and source-derived schema checker against the same verifier/core files; those broader checks were not independently rerun after this test-only correction. The author's broader workspace/Clippy results are handoff evidence, not independent results here. The tracked accepted-project fixture aggregate before and after these checks was 59 files, `dc78571a1468af8b789adb7021d5080c56b24eb82c08cd114764d9284bd4c909`, using the freeze's worktree-relative path/NUL/file-hash/newline recipe.

To rerun the earlier nested lineage and source-forgery probe from the previous review:

```sh
python3 - <<'PY'
import pathlib,re,subprocess,sys
review=pathlib.Path('docs/plans/a02-versioning-review.md').read_text()
blocks=re.findall(r"```sh\npython3 - <<'PY'\n(.*?)\nPY\n```",review,re.S)
assert len(blocks)==1
p=subprocess.run([sys.executable,'-c',blocks[0]],capture_output=True,text=True,timeout=20)
print('probe exit',p.returncode); print(p.stdout,end=''); print(p.stderr,end=''); assert p.returncode==0
PY
```

Probe exit 0; each of `ancestor-v2`, `intermediate-mixed`, `selected-unknown`, `ancestor-missing`, `v1-bad-source` and `hidden-source-edit` produced CLI exit 1 with `ARTIFACT_INVALID`. The first four reported unsupported profile; the latter two reported stored source invalid and source/normalized mismatch, respectively. The script itself prints results; the assertions in the Cargo tests provide independent negative test enforcement.

## Disposable golden mutation probes

The commands below run the Cargo-discovered golden test binary in isolated process groups and temporary roots. They pause that test after its **copied** project has created `.archy/lock`, mutate only the copied fixture, then resume it. A nonzero test exit is expected. Cleanup kills the isolated process group and removes the temporary root.

```sh
python3 - <<'PY'
import glob,os,pathlib,signal,subprocess,tempfile,time
exe=next(p for p in glob.glob('/home/butahlecoq/work/archy/.tools/targets/a02-version-review/debug/deps/golden_artifacts-*') if os.path.isfile(p) and os.access(p,os.X_OK))
for mode in ('unexpected-file','modified-report'):
 with tempfile.TemporaryDirectory(prefix='archy-a02-golden-fault-') as tmp:
  env=dict(os.environ,TMPDIR=tmp)
  with open(tmp+'/out','w+') as out:
   p=subprocess.Popen([exe,'--exact','curated_run_chain_and_fresh_cli_workflow_match_golden_evidence','--nocapture'],stdin=subprocess.DEVNULL,stdout=out,stderr=subprocess.STDOUT,env=env,start_new_session=True)
   stopped=False
   try:
    deadline=time.monotonic()+10; root=None
    while time.monotonic()<deadline and p.poll() is None:
     roots=list(pathlib.Path(tmp).glob('archy-golden-*'))
     if roots and (roots[0]/'saved/.archy/lock').exists():
      os.kill(p.pid,signal.SIGSTOP); stopped=True; root=roots[0]; break
     time.sleep(.0002)
    assert stopped and root
    runs=sorted((root/'saved/runs').iterdir()); target=runs[0]
    if mode=='unexpected-file': (target/'unexpected-probe').write_bytes(b'probe')
    else:
     report=target/'report.md'; report.write_bytes(report.read_bytes()+b'probe')
    os.kill(p.pid,signal.SIGCONT); stopped=False
    rc=p.wait(timeout=15); out.flush(); out.seek(0); text=out.read()
    print(mode,'exit',rc,'unexpected_entry_detected',('unexpected-probe' in text),'integrity_failure',('ArtifactInvalid' in text or 'ARTIFACT_INVALID' in text or 'hash' in text),'test_failed',('FAILED' in text))
    assert rc!=0 and 'FAILED' in text
   finally:
    if stopped:
     try: os.kill(p.pid,signal.SIGCONT)
     except ProcessLookupError: pass
    try: os.killpg(p.pid,signal.SIGKILL)
    except ProcessLookupError: pass
    if p.poll() is None: p.wait(timeout=3)
PY
```

Output: `unexpected-file exit 101 unexpected_entry_detected True integrity_failure False test_failed True`; `modified-report exit 101 unexpected_entry_detected False integrity_failure True test_failed True`.

```sh
python3 - <<'PY'
import glob,os,pathlib,signal,subprocess,tempfile,time
exe=next(p for p in glob.glob('/home/butahlecoq/work/archy/.tools/targets/a02-version-review/debug/deps/golden_artifacts-*') if os.path.isfile(p) and os.access(p,os.X_OK))
for mode in ('nonempty-lock','nonempty-staging'):
 with tempfile.TemporaryDirectory(prefix='archy-a02-transient-fault-') as tmp:
  env=dict(os.environ,TMPDIR=tmp)
  with open(tmp+'/out','w+') as out:
   p=subprocess.Popen([exe,'--exact','curated_run_chain_and_fresh_cli_workflow_match_golden_evidence','--nocapture'],stdin=subprocess.DEVNULL,stdout=out,stderr=subprocess.STDOUT,env=env,start_new_session=True); stopped=False
   try:
    end=time.monotonic()+10
    while time.monotonic()<end and p.poll() is None:
     roots=list(pathlib.Path(tmp).glob('archy-golden-*'))
     if roots and (roots[0]/'saved/.archy/lock').exists(): os.kill(p.pid,signal.SIGSTOP); stopped=True; break
     time.sleep(.0002)
    assert stopped
    archy=roots[0]/'saved/.archy'
    if mode=='nonempty-lock': (archy/'lock').write_bytes(b'probe')
    else: (archy/'staging/unexpected').write_bytes(b'probe')
    os.kill(p.pid,signal.SIGCONT); stopped=False
    rc=p.wait(timeout=15); out.flush(); out.seek(0); text=out.read()
    print(mode,'exit',rc,'failed',('FAILED' in text))
    assert rc!=0 and 'FAILED' in text
   finally:
    if stopped:
     try: os.kill(p.pid,signal.SIGCONT)
     except ProcessLookupError: pass
    try: os.killpg(p.pid,signal.SIGKILL)
    except ProcessLookupError: pass
    if p.poll() is None: p.wait(timeout=3)
PY
```

Output: `nonempty-lock exit 101 failed True`; `nonempty-staging exit 101 failed True`.

## Frozen hashes

The 12 frozen entries are listed exactly below. The handoff and candidate code hashes matched on the independent reads before and after tests; the prior review document was checked against the freeze on the final read. The freeze file is intentionally outside its own list and was independently hashed.

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `crates/archy-core/src/lib.rs` | `c92f273c2d9cbbb173a6d7b392c29513f90a1da4fc72492b177511f03d3b61c1` |
| `crates/archy-core/src/project.rs` | `fca7c7db21d9eadda788934bd84332bcaeb81443182d141ce8d9b5477e0b77de` |
| `crates/archy-core/src/project/v1.rs` | `89eb57e2518e282389057b08c0791ad4813c474c58112f9a530c29c70f56fa6c` |
| `crates/archy-core/src/validation.rs` | `2354fb5fa868372d4ca793e616f3b6c3faff412bd38655a204778429bb121597` |
| `crates/archy-core/src/validation/v1.rs` | `10312f50576c34515db32cd1e6025bc19ee5531c147074b66982721746d391ca` |
| `crates/archy-cli/src/run_integrity.rs` | `b9ea3fafdd202f1f3c0c5f74c6390f1e1fa5641cd27e7abbd02d2da0d127517c` |
| `crates/archy-cli/tests/core_workflow.rs` | `ebd46ebe7ef1f06dd37e143130285b382cc497605ba082f3166eeff69166b56f` |
| `crates/archy-cli/tests/golden_artifacts.rs` | `8d2adcd3b4965174f286abb369d56db939cd9fe1deb4fa762aacd165c3bc1180` |
| `tests/fixtures/briefs/legacy-adversarial-preferences.json` | `d3b51c16a980a422b60b972de9e47ca176f9ed2d937706743efac191950fdd75` |
| `docs/plans/a02-versioning-handoff.md` | `c8a3f6caa25919f073602f80b669d6e87b18a77d04da1d9ef96d6d5cd212c541` |
| `docs/plans/a02-versioning-review.md` | `ffdfdfe9426d935ec3ce0ec400d9581887b49b39f56bdbd9c9c0ebcd5b93b3ad` |
| `docs/plans/a02-versioning-freeze.md` | `0db779c45fc8243f6228091be040f72cf1965c6f0dba8ce354ad5ae009eb99a9` |

These checks do not establish actual Omarchy/backend behavior, sudden-power-loss durability, a new v2 profile or A02 issue closure.
