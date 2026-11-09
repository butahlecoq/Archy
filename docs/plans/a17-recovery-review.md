# A17 staged recovery: independent review

**Verdict: ISSUES.** The normal staged-process-kill recovery claim passes the focused test, but the new test helper can write to a caller-supplied project path and can leave its child running if the parent panics before `kill()`. Both were reproduced in disposable directories. Fix the test harness before accepting this slice. This is not A17 or release acceptance.

Review base: `57ba4f3b55d093da340adc5b4e05558efa3cc315`. The test and two author documents were **untracked** at that base; they are present in this worktree, not part of the committed base. I read `AGENTS.md`, the handoff, `docs/recovery.md`, the entire new test, the relevant public storage and request implementation, existing storage and interruption tests, and the A17 backlog. Linux `6.6.87.2-microsoft-standard-WSL2`, Rust `1.99.1`, Cargo `1.99.1`. No product source, issue state, commit, push or main checkout was changed.

## Requirement findings

| Requirement | Verdict and evidence |
| --- | --- |
| Real writer reaches staging before SIGKILL | **PASS.** `child_stages_until_killed` obtains the lock and a `PendingRun`, calls `write("data", b"unaccepted", ...)`, then writes `child-ready` and waits. `PendingRun::write` calls `write_synced`, including file `sync_all`, before returning. The parent waits up to 10 seconds for the marker, reads the staged bytes, calls `Child::kill`, waits and asserts Unix signal 9. The child cannot enter commit on this control path. The marker proves the write completed; it does not prove the child had already entered the sleep loop. |
| Prior accepted revision/inventory survives restart | **PASS for this fixture and boundary.** The test compares SHA-256 maps of the first run's complete immediate file entries and revision-1 file before kill, after reopen, and after retry; it also reads the accepted artifact through `read_run`. An independent live filesystem snapshot below matched the post-kill map printed by the test. It covers a flat one-artifact fixture, not arbitrary nested inventories or every fault boundary. |
| Same immutable request and one retry, exact replay | **PASS for request/receipt.** Child, retry and replay all call `request("interrupted", 1)`, which constructs the same schema, key, expected revision, `runs` destination, operation, `count=1,seed=42` bytes, source hash and `None` source run. Retry commits revision 2, replay returns the exact `RunReceipt`, revision stays 2, receipts equal `[first, second]`, and the abandoned staging file remains unaccepted. A changed expected revision with the same key returns `REVISION_CONFLICT`; the separately rerun existing storage test also rejects changed request bytes and a stale new key. The child wrote `unaccepted`, while the retry writes `after-restart`: this demonstrates request identity and exactly-once publication, **not** deterministic regeneration or identical artifact content across the interruption. The second Cargo-discovered test `child_stages_until_killed` normally returns immediately without its environment variable; its standalone `ok` is not additional recovery evidence. |
| Failure cleanup is bounded | **ISSUE, reproduced.** If reading the staged file fails after readiness and before `child.kill()`, the parent panics and drops `Child` without killing/waiting. The isolated fault probe below produced `parent_exit=101 child_alive_after_parent_panic=True missing_file_panic=True`. On this path the temporary root also bypasses the success-only `remove_dir_all`. The test has a timeout cleanup for readiness, but no general guard for the other panic paths. **Fix:** own the child in a kill-and-wait guard and the fixture in a scoped cleanup guard; exercise a controlled pre-kill failure and verify both child exit and fixture cleanup. |
| Helper cannot mutate arbitrary user paths | **ISSUE, reproduced.** The standalone test reads `ARCHY_RECOVERY_CHILD_ROOT` and opens `project.json` under that supplied path, then `lock()` creates `.archy`, `staging`, `revisions`, and `lock` before `begin()` checks revision. The isolated probe below supplied a fresh temporary project and observed those files created with test exit 101. If the environment variable is inherited in an ordinary Cargo run, the helper is activated without proof that the path was created by its parent; a compatible revision-1 project can be staged and the helper then loops. **Fix:** restrict helper activation and bind its root to an authenticated parent-created disposable fixture, with a bounded protocol and cleanup; do not rely solely on an arbitrary environment path. This is a test-helper issue, not a claim that the product CLI accepts arbitrary output paths. |
| Documentation describes recovery and evidence limits | **PASS, scoped.** `docs/recovery.md` gives same key, expected revision, input and destination guidance and distinguishes staging from published orphans. It explicitly excludes sudden power loss, network filesystem durability, backend cancellation and MCP retries. The handoff separates WSL evidence from Omarchy, backend, MCP, cancellation and target filesystem gates. Existing source tests include synthetic EIO boundaries and a separate real abort after run publication, but those tests were not independently rerun here. |

## Independent test and inventories

Commands run from the audit worktree (all exited 0):

```sh
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-review cargo test --locked --offline -p archy-core --test recovery_staging -- --nocapture
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-review cargo test --locked --offline -p archy-core --test storage commit_replay_and_conflict_preserve_prior_bytes
```

The Cargo recovery target reported **2 passed, 0 failed**, including the no-op standalone helper described above. It printed `staging child termination signal: Some(9)` and `retry committed revision: 2`. The storage target reported **1 passed, 19 filtered**. A separate run paused the parent after the child had staged and independently enumerated/hash-read every fixture file before resuming it. That run exited 0 and printed signal 9 and revision 2. Its actual accepted files were:

| File in that run | Independent pre-kill SHA-256 | Test's post-kill SHA-256 |
| --- | --- | --- |
| `runs/db7132db-8d75-493c-9a9d-abc0c1ff0c3c/data` | `070c160a6299c5438070b1aa737b14fc2992ed49579c14264884886a5876f971` | same |
| `runs/db7132db-8d75-493c-9a9d-abc0c1ff0c3c/manifest.json` | `30541ef90faaf25225f0011fff06bbad8ece6f8ffcf87d4f38cf9e674c35f7e6` | same |
| `.archy/revisions/00000000000000000001.json` | `22c42f3f79a2652fe37dfc4df45aa4676776b2650af9cfff6dd7de4f5510b72a` | same |

The independent pre-kill inventory also contained `.archy/lock` (empty), `project.json` (`41cf6794ba4200b839c53531555f0f3998df4cbb01a4d5cb0b94e3ca5e23947d`), `child-ready` (`8d906d61f92c9696387e566ecee255b733c79536074173a8eab915bad2d1fa28`) and `.archy/staging/97a3fee2-f4c1-45fd-b27b-dc123dee2913/data` (`1cd8c53e4945f89988bdf494d9471ffa1dd5782de5142a782ee9bb961e629f93`). The test itself verifies staging remains with its original bytes after retry and that accepted revision 2 contains `after-restart`. UUID-dependent hashes vary between runs. Post-kill accepted hashes are from the test's filesystem reads, rather than a second external snapshot.

Exact command for the independent pre-kill inventory and the resumed test:

```sh
python3 - <<'PY'
import glob,hashlib,os,pathlib,signal,subprocess,tempfile,time
exe=next(x for x in glob.glob('/home/butahlecoq/work/archy/.tools/targets/a17-review/debug/deps/recovery_staging-*') if os.path.isfile(x) and os.access(x,os.X_OK))
with tempfile.TemporaryDirectory(prefix='archy-a17-inventory-') as t:
 env=dict(os.environ,TMPDIR=t); env.pop('ARCHY_RECOVERY_CHILD_ROOT',None)
 with open(t+'/out','w+') as output:
  p=subprocess.Popen([exe,'--exact','killed_staging_writer_preserves_accepted_inventory_and_retry_applies_once','--nocapture'],stdout=output,stderr=subprocess.STDOUT,env=env,start_new_session=True)
  stopped=False
  try:
   limit=time.monotonic()+5
   while time.monotonic()<limit and p.poll() is None:
    roots=list(pathlib.Path(t).glob('archy-recovery-staging-*'))
    if roots and (roots[0]/'.archy/revisions/00000000000000000001.json').exists() and list((roots[0]/'.archy/staging').iterdir()):
     os.kill(p.pid,signal.SIGSTOP); stopped=True; break
    time.sleep(.0001)
   assert stopped
   root=roots[0]; limit=time.monotonic()+3
   while not (root/'child-ready').exists() and time.monotonic()<limit: time.sleep(.001)
   assert (root/'child-ready').exists()
   files=sorted(path for path in root.rglob('*') if path.is_file())
   print('LIVE BEFORE KILL:',[(str(x.relative_to(root)),hashlib.sha256(x.read_bytes()).hexdigest()) for x in files],flush=True)
   os.kill(p.pid,signal.SIGCONT); stopped=False
   rc=p.wait(timeout=5); output.flush(); output.seek(0); print('EXIT',rc); print(output.read())
  finally:
   if stopped:
    try: os.kill(p.pid,signal.SIGCONT)
    except ProcessLookupError: pass
   try: os.killpg(p.pid,signal.SIGKILL)
   except ProcessLookupError: pass
   if p.poll() is None: p.wait(timeout=3)
PY
```

## Controlled failure probes

Both probes used the compiled Cargo test binary from the requested target, disposable temporary directories, and no user project. This is the exact path probe:

```sh
python3 - <<'PY'
import glob,json,os,pathlib,subprocess,tempfile
binary=glob.glob('/home/butahlecoq/work/archy/.tools/targets/a17-review/debug/deps/recovery_staging-*')
binary=[p for p in binary if os.access(p,os.X_OK) and os.path.isfile(p)][0]
with tempfile.TemporaryDirectory(prefix='archy-a17-review-path-') as d:
 root=pathlib.Path(d); (root/'project.json').write_bytes(b'source')
 env=os.environ.copy(); env['ARCHY_RECOVERY_CHILD_ROOT']=d
 run=subprocess.run([binary,'--exact','child_stages_until_killed','--nocapture'],env=env,capture_output=True,text=True,timeout=5)
 print(json.dumps({'exit':run.returncode,'files_created':sorted(str(p.relative_to(root)) for p in root.rglob('*')),'stale_revision_conflict':'RevisionConflict' in run.stdout}))
PY
```

Output: `{"exit": 101, "files_created": [".archy", ".archy/lock", ".archy/revisions", ".archy/staging", "project.json"], "stale_revision_conflict": false}`. The last boolean searches stdout only and does **not** establish the error classification; the finding is the observed filesystem mutation.

The following exact probe isolates the test in a new process group, waits for revision 1 and a staged directory, pauses the parent, deletes only the disposable staged `data` after readiness, resumes the parent, checks the child, and finally kills the entire test process group and removes the temporary fixture:

```sh
python3 - <<'PY'
import glob,os,pathlib,signal,subprocess,tempfile,time
binary=next(p for p in glob.glob('/home/butahlecoq/work/archy/.tools/targets/a17-review/debug/deps/recovery_staging-*') if os.path.isfile(p) and os.access(p,os.X_OK))
for attempt in range(10):
 with tempfile.TemporaryDirectory(prefix='archy-a17-review-fault-') as tmp:
  env=os.environ.copy(); env.pop('ARCHY_RECOVERY_CHILD_ROOT',None); env['TMPDIR']=tmp
  with open(os.path.join(tmp,'output.log'),'w+') as out:
   p=subprocess.Popen([binary,'--exact','killed_staging_writer_preserves_accepted_inventory_and_retry_applies_once','--nocapture'],env=env,stdout=out,stderr=subprocess.STDOUT,start_new_session=True)
   stopped=False
   try:
    deadline=time.monotonic()+3; roots=[]
    while time.monotonic()<deadline and p.poll() is None:
     roots=list(pathlib.Path(tmp).glob('archy-recovery-staging-*'))
     if roots and (roots[0]/'.archy/revisions/00000000000000000001.json').exists() and (roots[0]/'.archy/staging').exists():
      candidates=list((roots[0]/'.archy/staging').iterdir())
      if candidates:
       os.kill(p.pid,signal.SIGSTOP); stopped=True; break
     time.sleep(0.0001)
    if not stopped: continue
    root=roots[0]; deadline=time.monotonic()+2
    while not (root/'child-ready').exists() and time.monotonic()<deadline: time.sleep(0.001)
    files=list((root/'.archy/staging').glob('*/data')) if (root/'.archy/staging').exists() else []
    if not files or not (root/'child-ready').exists(): continue
    children=[]
    for task in pathlib.Path(f'/proc/{p.pid}/task').iterdir(): children.extend((task/'children').read_text().split())
    print('marker_present=True child_pids=',children,'staged_bytes=',files[0].read_bytes(),flush=True)
    files[0].unlink(); os.kill(p.pid,signal.SIGCONT); stopped=False
    rc=p.wait(timeout=3)
    alive=[pid for pid in children if pathlib.Path('/proc',pid,'stat').exists() and pathlib.Path('/proc',pid,'stat').read_text().split()[2]!='Z']
    out.flush(); out.seek(0); output=out.read()
    print('parent_exit=',rc,'child_alive_after_parent_panic=',bool(alive),'missing_file_panic=',('No such file or directory' in output),flush=True)
    break
   finally:
    if stopped:
     try: os.kill(p.pid,signal.SIGCONT)
     except ProcessLookupError: pass
    try: os.killpg(p.pid,signal.SIGKILL)
    except ProcessLookupError: pass
    if p.poll() is None: p.wait(timeout=3)
else: print('probe missed staging window')
PY
```

Output: `marker_present=True child_pids= ['415013'] staged_bytes= b'unaccepted'` and `parent_exit= 101 child_alive_after_parent_panic= True missing_file_panic= True`. The PID is specific to that invocation. A prior probe that paused during revision-1 staging missed the intended window and did not establish the finding; the successful probe explicitly waits for revision 1.

## Source identity and remaining gates

`sha256sum` was run before and after testing on the following exact files; all values matched on both reads. The seven author-frozen hashes also matched the handoff.

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `Cargo.toml` | `2e9a70f386ec6f57c88300dd18c6ab210efcc4b449c7ea1a4c325ee62336665d` |
| `crates/archy-core/Cargo.toml` | `426bfac83847bae297fffea306294a32e611bd61d264fad64c2b96c9023e112f` |
| `crates/archy-core/src/storage.rs` | `6d061879feafddff6f90918f0f454b78592b75a810acca615d0e8d0d106d01c8` |
| `crates/archy-core/src/run.rs` | `0d04853449b8bfa47d4fa626a98b4c3b5156c509bab9ec1d5f3985fc4cf8b86c` |
| `crates/archy-core/tests/storage.rs` | `252ba273a5e6afc1705be96ef8b421c73a2e423c271a13061a4a6e9670b5bf97` |
| `crates/archy-core/tests/recovery_staging.rs` | `eb7a81b4fdca83be28d35c0974ce2c52430000a4f5953f39c25e4b295c1d3974` |
| `crates/archy-cli/src/run_integrity.rs` | `13f4d1445421f9de6a97ddc4431e2ef52b59b54f4a078d4faed1375d7e25b1ef` |
| `crates/archy-cli/tests/core_workflow.rs` | `48ad2ca1a930d3f614bfd19ff67d9517befdf61449bb1446094950ab950e1d70` |
| `crates/archy-cli/tests/golden_artifacts.rs` | `978a048e78d98657eef1835bc8fb845b4cc2556a8991cd3bc2333d21e0d9611d` |
| `docs/plans/a17-recovery-handoff.md` | `cc1afae9a49f64c8dd24cfcf0cb370cb677a30644651b0f27f9722001d4ebce9` |
| `docs/recovery.md` | `20e1ce8f50982b0cf0f7829880b1e259804a02b36205182cfb336656e39f095b` |

The exact identity command was `sha256sum Cargo.lock Cargo.toml crates/archy-core/Cargo.toml crates/archy-core/src/storage.rs crates/archy-core/src/run.rs crates/archy-core/tests/storage.rs crates/archy-core/tests/recovery_staging.rs crates/archy-cli/src/run_integrity.rs crates/archy-cli/tests/core_workflow.rs crates/archy-cli/tests/golden_artifacts.rs docs/plans/a17-recovery-handoff.md docs/recovery.md`.

After fixing both harness issues, rerun the focused test and failure probes on the corrected frozen source. Full A17 still needs the backlog's other transaction boundaries and fault classes, retained backend failure logs, actual cancellation and concurrent/MCP single-apply checks, pinned deterministic manifest/metric reruns, and relevant target checks. Sudden-power-loss and real filesystem durability, actual Omarchy, Blender/IFC and MCP behavior remain unverified here; a SIGKILL cannot establish those gates.
