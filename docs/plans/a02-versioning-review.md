# A02 historical compatibility: independent review

**Verdict: ISSUES.** The v1 moves are byte-identical, the focused legacy and semantic-forgery tests pass, and rehashed lineage probes reject the tested unsupported labels. The required five-revision Cargo golden test fails reproducibly in this worktree: the new whole-tree assertion treats the lock and staging directory created by semantic verification as unexpected fixture changes. Do not accept the slice until that gate passes on the frozen candidate. This review was performed by the current session's agent; Sol-medium model identity was not independently available, so the requested separate Sol-medium review remains unverified. A02 itself remains open.

Base `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`. The candidate consists of tracked modifications and untracked v1 files, fixture and handoff; it is not the clean committed base. Read `AGENTS.md`, `docs/plans/a02-versioning-handoff.md`, `/home/butahlecoq/work/archy/.tools/research/a02-normalization-contract.md`, moved DTO/parser files, verifier, storage behavior, old and new workflow tests and golden test. No product source, commit, push, main checkout or host installation was changed.

## Findings and requirements

| Requirement | Result |
| --- | --- |
| Five-revision Cargo golden test and byte preservation | **ISSUE, reproduced twice.** `cargo test --locked --offline -p archy-cli --test golden_artifacts` exits 101 at `golden_artifacts.rs:58`: copied `.archy` entries are `["lock", "revisions", "staging"]` while tracked original has `["revisions"]`. `verify_run` calls `ProjectStore::lock()`, which creates lock/staging inside the copied fixture, before the newly added recursive entry comparison. The assertion occurs before the test's fresh two-run workflow, so that part did not complete. The independent read-only desktop probe below verified all five saved revisions and confirmed all 59 copied file hashes unchanged, but it does not turn the failing Cargo gate into a pass. **Fix:** compare every original accepted file and hash while separately allowing only the known transient lock/staging entries created in the disposable copy, or verify through a truly read-only semantic path; keep checks against missing, modified or unexpected accepted evidence. Rerun the golden and workspace gates without modifying the tracked golden bundle. |
| Exact v1 moves and public exports | **PASS.** `git show HEAD:crates/archy-core/src/project.rs | cmp -s - crates/archy-core/src/project/v1.rs` and the equivalent validation command both exit 0. Their base/current hashes match below. The wrappers re-export v1 types, `parse_project`, `normalize` and `InputFormat`; `parse_project_v1` names the same implementation. Source-derived schema checks passed. |
| Stored version dispatch and full integrity | **PASS for tested v1.** `verify_one` accepts only normalized `(schema_version=0.1.0, normalization_version=si-rectangle-v1)`, parses stored source through `parse_project_v1`, and checks exact serialized normalized bytes/digest. The existing code continues to recompute generation, variants, evaluations, report and events, and traverses/validates each parent. The full workflow target passes ten consistently rehashed semantic-forgery classes and unsupported algorithm rejection. The nested probes below rehashed artifacts, manifests and revision links before exercising the actual CLI. No v2 profile exists or was tested. |
| Legacy source snapshot, preferences, replay and conflicts | **PASS for fixture.** `legacy_preferences_keep_exact_v1_bytes_and_survive_source_edits` pins source and normalized SHA-256, checks source/report preservation for two ordinary wishes and two executable-looking/numeric-claim strings as passive historical data, verifies after malformed live source edit, rejects changed-source replay, restores the source and returns the original run/revision, and rejects a rehashed normalized-version forgery. This fixture is generated under the byte-identical v1 implementation; it is not a previously captured nonempty historical run. The wider workflow suite passes malformed replay conflict and immutable-run evaluation after source edit. No evidence says that the stored strings execute. |
| Broad current preferences and canonical migration | **PASS for preservation; A02 remains open.** An independent `validate` probe accepted and returned unchanged five varied architectural wishes including Unicode. Current admission remains v1, `project_id` and `value_m`; this slice does not implement the future canonical contract or settle any preference taxonomy. Preserve legitimate broad semantics and historical raw text in future work. |
| Source and fixture publication | **PASS for inspected bytes; scope limitation.** Source and handoff hashes were identical before/after. A copied tracked golden tree was verified read-only and matched all 59 original file hashes before/after. This is an application integrity check, not a signature or protection against an attacker able to rewrite all history and its hashes. |

## Commands and results

All Cargo/schema commands used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-version-review` from this worktree:

```sh
cargo test --locked --offline -p archy-cli --test core_workflow legacy_preferences_keep_exact_v1_bytes_and_survive_source_edits
cargo test --locked --offline -p archy-cli --test core_workflow
cargo test --locked --offline -p archy-cli --test golden_artifacts
python3 scripts/check-schemas.py
```

Results: focused legacy **1/1 pass**; full workflow **17/17 pass** including changed malformed inputs, source pinning, ten semantic forgeries, algorithm version, replay, edit/restore, and the over-64-run lineage test; golden **0/1, exit 101**, repeated alone with the same failure; schema checker **exit 0**, 14 schema files and actual serializers checked, golden normalized CLI and 26 negative CLI fixtures conform. Doctor's expected target-deferred exit was 1. The handoff's earlier full-workspace/golden pass was not reproduced at this candidate; its execution environment may have differed, but the cause of that discrepancy is not established here. No independent workspace Clippy/format or backend/Omarchy gate was run.

## Nested profile and source probes

Each case below copied the tracked five-revision project to a disposable directory, made only that copy's managed directories private, modified one stored artifact, updated its artifact/manifest hashes and lengths and the revision hash chain, then asked the actual CLI to verify selected revision 5 and its ancestry. All six returned exit 1 and `ARTIFACT_INVALID`:

| Case | Message |
| --- | --- |
| Revision 1 ancestor normalized label `si-rectangle-v2` | `unsupported recorded normalization profile` |
| Revision 4 parent normalized schema `0.2.0` with v1 label | `unsupported recorded normalization profile` |
| Revision 5 normalized label `unknown` | `unsupported recorded normalization profile` |
| Revision 1 normalization label removed | `unsupported recorded normalization profile` |
| Revision 4 stored v1 YAML replaced by `[` and consistently rehashed | `stored source invalid: ... unclosed bracket '[' ...` |
| Revision 4 stored source gains `preferences: [retain trees]` and is consistently rehashed, normalized bytes left unchanged | `source and normalized snapshot differ` |

The malformed v1 source is correctly reported as a source parsing error, not as v2. The test changes stored snapshots, not the live source. Full recomputation and legacy live-source edit/replay behavior are additionally covered by the passing Cargo tests. Exact probe:

```sh
python3 - <<'PY'
import hashlib,json,os,pathlib,shutil,subprocess,tempfile
source=pathlib.Path('examples/courtyard-housing/expected/accepted-project').resolve()
binary='/home/butahlecoq/work/archy/.tools/targets/a02-version-review/debug/archy'
def digest(b): return hashlib.sha256(b).hexdigest()
def encode(v): return json.dumps(v,separators=(',',':'),ensure_ascii=False).encode()
for label,number,artifact,field,value in [('ancestor-v2',1,'normalized.json','normalization_version','si-rectangle-v2'),('intermediate-mixed',4,'normalized.json','schema_version','0.2.0'),('selected-unknown',5,'normalized.json','normalization_version','unknown'),('ancestor-missing',1,'normalized.json','normalization_version',None),('v1-bad-source',4,'input.yaml',None,None),('hidden-source-edit',4,'input.yaml','append',None)]:
 with tempfile.TemporaryDirectory(prefix='archy-a02-version-review-') as t:
  root=pathlib.Path(t)/'project'; shutil.copytree(source,root)
  for tree in [root/'.archy',root/'runs']:
   os.chmod(tree,0o700)
   for p in tree.rglob('*'):
    if p.is_dir(): os.chmod(p,0o700)
  records=sorted((root/'.archy/revisions').glob('*.json')); parsed=[json.loads(p.read_bytes()) for p in records]
  rec=parsed[number-1]; run=root/rec['receipt']['path']; manifest_path=run/'manifest.json'; manifest=json.loads(manifest_path.read_bytes()); item=run/artifact
  if artifact=='normalized.json':
   v=json.loads(item.read_bytes())
   if value is None: del v[field]
   else: v[field]=value
   data=encode(v)
  elif field=='append': data=item.read_bytes()+b'\npreferences: [retain trees]\n'
  else: data=b'['
  item.write_bytes(data)
  for a in manifest['artifacts']:
   if a['path']==artifact: a['sha256']=digest(data); a['byte_len']=len(data)
  manifest['normalized_sha256' if artifact=='normalized.json' else 'input_sha256']=digest(data)
  raw=encode(manifest); manifest_path.write_bytes(raw); rec['receipt']['manifest_sha256']=digest(raw)
  previous=None
  for p,r in zip(records,parsed):
   r['previous_sha256']=previous; raw=encode(r); p.write_bytes(raw); previous=digest(raw)
  runid=parsed[4]['receipt']['run_id']
  p=subprocess.run([binary,'desktop','status','--project',str(root/'project.yaml'),'--run',runid,'--limit','1','--json'],capture_output=True,text=True)
  try: out=json.loads(p.stdout)
  except json.JSONDecodeError: out={'raw':p.stdout[:100]}
  print(label,'exit=',p.returncode,'code=',out.get('code'),'message=',out.get('message'))
PY
```

An independent read-only `archy desktop status --project <copy>/project.yaml --limit 20 --json` returned exit 0 and run revisions `[5,4,3,2,1]`; comparison of SHA-256 maps before/after returned `source_file_count 59 copy_unchanged True`. This observation uses a separate copied tree, without the Cargo golden test's mutating `verify_run` calls.

## Frozen source identities

The following command was run before and after testing and all hashes matched both times. The moved file hashes independently match `git show HEAD:<original path>`; `cmp` was also successful. The fixture source hash matches the handoff's pinned value. The focused Cargo test checked the v1 serialized normalized digest `5665a318acf3abe884945905371fab72415405276bcb99783bc78fec2ea10bf3`.

```sh
sha256sum Cargo.lock crates/archy-core/src/lib.rs crates/archy-core/src/project.rs crates/archy-core/src/project/v1.rs crates/archy-core/src/validation.rs crates/archy-core/src/validation/v1.rs crates/archy-cli/src/run_integrity.rs crates/archy-cli/tests/core_workflow.rs crates/archy-cli/tests/golden_artifacts.rs tests/fixtures/briefs/legacy-adversarial-preferences.json docs/plans/a02-versioning-handoff.md /home/butahlecoq/work/archy/.tools/research/a02-normalization-contract.md
```

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
| `crates/archy-cli/tests/golden_artifacts.rs` | `fc07a6c786d36af20031f54d2c4604a6c2cd9caa9b578e3e29f462afc053c74e` |
| `tests/fixtures/briefs/legacy-adversarial-preferences.json` | `d3b51c16a980a422b60b972de9e47ca176f9ed2d937706743efac191950fdd75` |
| `docs/plans/a02-versioning-handoff.md` | `a5775272db5cf4a54eb386c815f0e6db9c63a057b95a29346a72a5abf8c8dbad` |
| `/home/butahlecoq/work/archy/.tools/research/a02-normalization-contract.md` | `1789bb7fd782fc41ab5f058c63bc99a79636cd82f22a9351e8ba2b80b94d8baa` |
