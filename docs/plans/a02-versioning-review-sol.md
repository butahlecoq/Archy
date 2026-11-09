# A02 historical compatibility: independent Sol medium review

**Scoped verdict: PASS for the frozen historical compatibility slice; A02 remains open.** Two fresh, independent Codex reviewers ran as `gpt-6-sol` with `model_reasoning_effort=medium` and read-only sandboxes. Their session IDs were `01a188ff-5fef-7970-8c77-4b5bfa93a5c9` and `01a188ff-6007-7630-a8d5-c338453781b2`. Both found no new reproducible compatibility failure. Their sandbox prevented Cargo execution, so the coordinating review ran the tests and mutation probes below in a fresh writable target. No implementation, accepted fixture, commit, or remote state was changed.

Base and branch: `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`, `codex/a02-versioning`. The reviewed state is an uncommitted working tree. I read `a02-versioning-{handoff,freeze,review,review-round2}.md`, `AGENTS.md`, the entire tracked diff, both untracked v1 files, the added fixture, the complete verifier, and the affected tests. The first review's golden failure was corrected in the frozen candidate; this report independently repeats that gate.

## Findings

| Requirement | Evidence and limit |
| --- | --- |
| Frozen identities | All 12 freeze-list hashes below match. The freeze file itself hashes to `0db779c45fc8243f6228091be040f72cf1965c6f0dba8ce354ad5ae009eb99a9`. The accepted-project tree has 59 files and aggregate SHA-256 `dc78571a1468af8b789adb7021d5080c56b24eb82c08cd114764d9284bd4c909`, using sorted worktree-relative paths, NUL, each lowercase file hash, newline. |
| Exact v1 move | `git show HEAD:crates/archy-core/src/project.rs | cmp -s - crates/archy-core/src/project/v1.rs` and the corresponding validation command both exit 0. The wrappers re-export the moved implementation, and `parse_project_v1` aliases that parser. There is one active v1 parser/normalizer in the current tree, not divergent duplicate logic. The old source remains in the base commit as historical Git content. |
| Version and source binding | `verify_one` reads stored `normalized.json`, accepts only `schema_version=0.1.0` and `normalization_version=si-rectangle-v1`, reparses the stored input bytes through `parse_project_v1`, and requires exact serialized normalized bytes and digest. The six consistently rehashed lineage/source probes below returned `ARTIFACT_INVALID`, including labels on ancestors. This is a v1-only dispatch, not an implemented v2 profile. |
| Legacy behavior and preferences | The focused legacy test pins source SHA-256 `d3b51c16a980a422b60b972de9e47ca176f9ed2d937706743efac191950fdd75` and serialized normalized SHA-256 `5665a318acf3abe884945905371fab72415405276bcb99783bc78fec2ea10bf3`. It checks stored source/normalized bytes, all four preference strings in the report, verification after a malformed live-source edit, changed-source replay conflict, same-key replay after restoration, and rejection of a rehashed v2 label. The fixture was produced with v1 code; it is not a captured older preference-bearing run. Its executable-looking text remains passive historical data. |
| Five-run golden and mutation detection | The golden test passes and checks all five accepted revisions plus the fresh workflow. It compares baseline, original fixture, and verified copy recursively, removing only a verified empty lock file and empty staging directory from the disposable copy. Each of four injected mutations in that copy caused the test to fail: unexpected file, modified report, nonempty lock, and nonempty staging. This establishes detection for those probes, not arbitrary concurrent filesystem attacks. |
| Future migration risk | Current public admission, generator/evaluator types, report renderer, and algorithm implementations remain v1/shared. Future v2 work must keep a versioned v1 replay path and avoid changing shared v1 output bytes. No v2 DTO/parser/normalizer, profile tuple, canonical `id`/`bound`/`unit` output, or bounded preference policy was added. These are open A02 work, not failures of this historical slice. |

## Commands and observed results

All commands ran from `/home/butahlecoq/work/archy/.worktrees/a02-versioning`. The reviewers were launched afresh with `codex exec -m gpt-6-sol -c model_reasoning_effort='"medium"' -s read-only --ephemeral -C /home/butahlecoq/work/archy/.worktrees/a02-versioning` and separate review prompts. Their startup logs explicitly reported `model: gpt-6-sol`, `reasoning effort: medium`, and `sandbox: read-only`. Neither reviewer's prior-session result was resumed. Reviewer 2's attempted Cargo command exited 101 before tests because its sandbox denied `.cargo-build-lock`; that exit is not a test failure.

```sh
git rev-parse HEAD
git branch --show-current
git diff --check
git show HEAD:crates/archy-core/src/project.rs | cmp -s - crates/archy-core/src/project/v1.rs
git show HEAD:crates/archy-core/src/validation.rs | cmp -s - crates/archy-core/src/validation/v1.rs
sha256sum Cargo.lock crates/archy-core/src/lib.rs crates/archy-core/src/project.rs crates/archy-core/src/project/v1.rs crates/archy-core/src/validation.rs crates/archy-core/src/validation/v1.rs crates/archy-cli/src/run_integrity.rs crates/archy-cli/tests/core_workflow.rs crates/archy-cli/tests/golden_artifacts.rs tests/fixtures/briefs/legacy-adversarial-preferences.json docs/plans/a02-versioning-handoff.md docs/plans/a02-versioning-review.md docs/plans/a02-versioning-freeze.md
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-version-review-sol-fresh cargo test --locked --offline -p archy-cli --test core_workflow legacy_preferences_keep_exact_v1_bytes_and_survive_source_edits
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-version-review-sol-fresh cargo test --locked --offline -p archy-cli --test core_workflow consistently_rehashed_forgery_never_grants_trusted_status
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-version-review-sol-fresh cargo test --locked --offline -p archy-cli --test golden_artifacts
```

The base/branch commands returned the identities above; `git diff --check` and both `cmp` commands exited 0. The fresh target initially compiled from scratch. Each focused Cargo command exited 0 with **1 passed, 0 failed**. The forgery test exercises ten consistently rehashed artifact classes; the golden test includes five stored revisions and a fresh two-run workflow.

The following exact command reused the six-case probe printed in `a02-versioning-review.md`, substituting only the binary in this fresh target. It exited 0. Cases `ancestor-v2`, `intermediate-mixed`, `selected-unknown`, `ancestor-missing`, `v1-bad-source`, and `hidden-source-edit` each produced CLI exit 1 and `ARTIFACT_INVALID`; the first four reported unsupported profile, the fifth invalid stored source, and the sixth source/normalized mismatch.

```sh
python3 - <<'PY'
import pathlib,re,subprocess,sys
review=pathlib.Path('docs/plans/a02-versioning-review.md').read_text()
blocks=re.findall(r"```sh\npython3 - <<'PY'\n(.*?)\nPY\n```",review,re.S)
assert len(blocks)==1
script=blocks[0].replace('/home/butahlecoq/work/archy/.tools/targets/a02-version-review/debug/archy','/home/butahlecoq/work/archy/.tools/targets/a02-version-review-sol-fresh/debug/archy')
p=subprocess.run([sys.executable,'-c',script],capture_output=True,text=True,timeout=40)
print('probe exit',p.returncode)
print(p.stdout,end='');print(p.stderr,end='')
assert p.returncode==0
PY
```

The disposable golden mutation probe ran the fresh binary at `/home/butahlecoq/work/archy/.tools/targets/a02-version-review-sol-fresh/debug/deps/golden_artifacts-5ce8a3aaa56748d1`. It launched the exact golden test under a private `TMPDIR`, paused its process when `saved/.archy/lock` appeared, and changed only its `saved` copy. The four modes and observed `(test exit, FAILED)` were `unexpected-file (101, true)`, `modified-report (101, true)`, `nonempty-lock (101, true)`, and `nonempty-staging (101, true)`. Reproduction command:

```sh
python3 - <<'PY'
import os,pathlib,signal,subprocess,tempfile,time
exe=pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a02-version-review-sol-fresh/debug/deps/golden_artifacts-5ce8a3aaa56748d1')
for mode in ('unexpected-file','modified-report','nonempty-lock','nonempty-staging'):
 with tempfile.TemporaryDirectory(prefix='archy-a02-sol-mutation-') as tmp:
  env=dict(os.environ,TMPDIR=tmp)
  with open(tmp+'/output','w+') as output:
   process=subprocess.Popen([str(exe),'--exact','curated_run_chain_and_fresh_cli_workflow_match_golden_evidence','--nocapture'],stdout=output,stderr=subprocess.STDOUT,env=env,start_new_session=True)
   stopped=False
   try:
    deadline=time.monotonic()+15
    while time.monotonic()<deadline and process.poll() is None:
     roots=list(pathlib.Path(tmp).glob('archy-golden-*'))
     if roots and (roots[0]/'saved/.archy/lock').exists():
      os.kill(process.pid,signal.SIGSTOP); stopped=True; root=roots[0]; break
     time.sleep(.0002)
    assert stopped, mode+' could not intercept copied fixture'
    saved=root/'saved'
    if mode=='unexpected-file':
     (next((saved/'runs').iterdir())/'unexpected-probe').write_bytes(b'probe')
    elif mode=='modified-report':
     report=next((saved/'runs').iterdir())/'report.md'; report.write_bytes(report.read_bytes()+b'probe')
    elif mode=='nonempty-lock':
     (saved/'.archy/lock').write_bytes(b'probe')
    else:
     (saved/'.archy/staging/unexpected-probe').write_bytes(b'probe')
    os.kill(process.pid,signal.SIGCONT); stopped=False
    rc=process.wait(timeout=20); output.flush(); output.seek(0); result=output.read()
    print(mode,'exit',rc,'FAILED',('FAILED' in result))
    assert rc!=0 and 'FAILED' in result
   finally:
    if stopped:
     try: os.kill(process.pid,signal.SIGCONT)
     except ProcessLookupError: pass
    try: os.killpg(process.pid,signal.SIGKILL)
    except ProcessLookupError: pass
    if process.poll() is None: process.wait(timeout=3)
PY
```

## Frozen SHA-256 identities

| Path | SHA-256 |
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

The two Sol reviewers' read-only limitation and the scoped passing tests are separate evidence. This review does not approve new preference semantics, v2 publication, backend behavior, or A02 closure.
