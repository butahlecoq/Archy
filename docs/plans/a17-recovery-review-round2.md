# A17 recovery review: round two

**Verdict: ISSUES for the requested all-path cleanup guarantee.** Both round-one findings are fixed for normal execution and Rust panic unwinding. The normal recovery scenario passes. A separate controlled SIGKILL of the test parent leaves the staging child alive and its temporary fixture present, because neither `Drop` guard runs. This is a test-harness interruption limit; it does not disprove the staged-writer recovery test. The handoff describes unwind cleanup, so it does not claim this stronger guarantee.

Base `57ba4f3b55d093da340adc5b4e05558efa3cc315`; the new test and handoffs remain untracked at this base. I read the previous independent review, updated handoff, recovery instructions and complete updated test. The existing storage implementation and request identity were reviewed in round one. These observations are from Linux `6.6.87.2-microsoft-standard-WSL2`, Rust/Cargo 1.99.1, with the requested separate SSD target. No product edits, commit, push or main-checkout writes were made.

## Verdicts

| Requirement | Result |
| --- | --- |
| Lone inherited environment path cannot select an unrelated project | **PASS.** The helper no longer reads `ARCHY_RECOVERY_CHILD_ROOT`; it resolves its root through the directory opened on stdin at `/proc/self/fd/0`. With stdin `/dev/null`, the original path repro exits 0 and leaves only `project.json`. The Cargo adversarial test also supplies an unrelated environment path while a real fixture directory descriptor is on stdin, then confirms the unrelated project is unchanged. A process that deliberately supplies a directory as stdin supplies an actual descriptor capability; this is a separate authority boundary, not something an environment path alone can manufacture. |
| Cleanup on ordinary panic/unwind | **PASS for the exercised paths.** `Writer::drop` attempts child kill and wait before `Fixture::drop` attempts removal, using reverse local drop order. The four-test target forces missing marker and missing staged-data reads inside `catch_unwind`; both record a reaped signal-9 child and absent fixture. The original external staged-file-removal probe now reports parent exit 101, child not alive and missing-file panic; a second run additionally observes fixture absent. The guards discard kill/removal errors, so this does not prove cleanup under OS errors. |
| Cleanup on **all** parent exits | **ISSUE, reproduced.** The parent-SIGKILL probe below reports `parent_exit= -9 child_alive= True fixture_exists= True`. Destructors do not run when the parent is forcibly killed. The external probe then kills the isolated process group and removes its disposable root; no live test writer was left behind. **Next step:** if forced parent termination is within the required cleanup contract, use an external supervisor or another parent-death mechanism that bounds the child and fixture lifetime, and test that boundary. Otherwise document the parent-kill exception explicitly. A reaper must be a surviving process if a process has itself been killed; this probe does not establish a product recovery defect. |
| Real staged writer SIGKILL and accepted history | **PASS, scoped.** `PendingRun::write` completes before `child-ready`; the parent verifies staged `unaccepted` bytes, calls `kill_and_wait`, and asserts signal 9. It reopens `ProjectStore`, observes revision 1, rejects the staged run as unaccepted, and compares accepted artifact/manifest/revision hashes before and after kill and again after retry. This reopens the store in the surviving test process; it is not a fresh operating-system process restart. |
| Exact request and replay | **PASS, scoped.** Child, retry and replay all call `request("interrupted", 1)` with the same request bytes, key, destination and source hash. Retry publishes revision 2 once, exact receipt replay returns the same receipt, receipt history has exactly the two expected revisions, and the staged run remains unaccepted. The original staged artifact is `unaccepted` and the retry artifact is `after-restart`; this demonstrates request identity and publication behavior, **not deterministic artifact equality**. |
| Documentation and target gates | **PASS for stated limits.** `docs/recovery.md` distinguishes the different staged/retry bytes and excludes sudden power loss, network filesystem durability, backend cancellation and MCP retries. No real Omarchy, backend, MCP, power-loss or full A17 acceptance is established by this review. |

## Independent Cargo results

Exact command, from the audit worktree:

```sh
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-review cargo test --locked --offline -p archy-core --test recovery_staging -- --nocapture
```

Exit 0: **4 passed, 0 failed**. The standalone child test is a no-op without a directory on stdin. Two expected `No such file or directory` panic messages are caught by `parent_unwind_reaps_writer_and_removes_fixture`. The main recovery test printed signal `Some(9)` and revision `2`. Its before/after values were identical:

| Accepted item | SHA-256 before and after |
| --- | --- |
| `data` | `070c160a6299c5438070b1aa737b14fc2992ed49579c14264884886a5876f971` |
| `manifest.json` | `c0d3227ad7be0989587d099a9b083ffd17ec80f01f51db92c5ac147667f060a2` |
| revision 1 | `6607c38d7bab029451a12e6b2b05111fe20f438b5a328fe57c72c3a8cd8628ee` |

Manifest/revision hashes vary with random run IDs. Test assertions also compare the inventory after the retry.

## Repeated round-one probes

Exact command to rerun the original two scripts verbatim from the prior review, with the already-built test binary in the requested target:

```sh
python3 - <<'PY'
import pathlib,re,subprocess,sys
review=pathlib.Path('docs/plans/a17-recovery-review.md').read_text()
blocks=re.findall(r"```sh\npython3 - <<'PY'\n(.*?)\nPY\n```",review,re.S)
assert len(blocks)==3
for label,block in zip(('path','parent-panic'),blocks[1:]):
 result=subprocess.run([sys.executable,'-c',block],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=40)
 print(label,'exit',result.returncode)
 print(result.stdout,end='')
 if result.stderr: print(result.stderr,end='')
 if result.returncode: raise SystemExit(result.returncode)
PY
```

Output:

```text
path exit 0
{"exit": 0, "files_created": ["project.json"], "stale_revision_conflict": false}
parent-panic exit 0
marker_present=True child_pids= ['441649'] staged_bytes= b'unaccepted'
parent_exit= 101 child_alive_after_parent_panic= False missing_file_panic= True
```

The `stale_revision_conflict` field is an old stdout-only search and is irrelevant to the path result. A separate invocation inserted a fixture-existence check into the same earlier panic probe:

```sh
python3 - <<'PY'
import pathlib,re,subprocess,sys
review=pathlib.Path('docs/plans/a17-recovery-review.md').read_text()
block=re.findall(r"```sh\npython3 - <<'PY'\n(.*?)\nPY\n```",review,re.S)[2]
old="rc=p.wait(timeout=3)\n    alive="
new="rc=p.wait(timeout=3)\n    print('fixture_exists_after_parent_exit=',root.exists(),flush=True)\n    alive="
assert old in block
result=subprocess.run([sys.executable,'-c',block.replace(old,new)],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=40)
print('probe exit',result.returncode); print(result.stdout,end=''); print(result.stderr,end='')
assert result.returncode==0
PY
```

Output included `fixture_exists_after_parent_exit= False`, `parent_exit= 101`, `child_alive_after_parent_panic= False`, and `missing_file_panic= True`.

## Parent SIGKILL probe

This exact command uses a fresh temporary root and isolated process group, waits for the child-ready marker, kills the test **parent** while it is stopped, checks the child and fixture, and cleans up the group and root in `finally`:

```sh
python3 - <<'PY'
import glob,os,pathlib,signal,subprocess,tempfile,time
exe=next(p for p in glob.glob('/home/butahlecoq/work/archy/.tools/targets/a17-review/debug/deps/recovery_staging-*') if os.path.isfile(p) and os.access(p,os.X_OK))
with tempfile.TemporaryDirectory(prefix='archy-a17-parent-kill-') as tmp:
 env=dict(os.environ,TMPDIR=tmp); env.pop('ARCHY_RECOVERY_CHILD_ROOT',None)
 with open(tmp+'/out','w+') as out:
  p=subprocess.Popen([exe,'--exact','killed_staging_writer_preserves_accepted_inventory_and_retry_applies_once','--nocapture'],stdin=subprocess.DEVNULL,stdout=out,stderr=subprocess.STDOUT,env=env,start_new_session=True)
  stopped=False
  try:
   end=time.monotonic()+5
   while time.monotonic()<end and p.poll() is None:
    roots=list(pathlib.Path(tmp).glob('archy-recovery-staging-*'))
    if roots and (roots[0]/'.archy/revisions/00000000000000000001.json').exists() and list((roots[0]/'.archy/staging').iterdir()):
     os.kill(p.pid,signal.SIGSTOP); stopped=True; break
    time.sleep(.0001)
   assert stopped
   root=roots[0]; end=time.monotonic()+3
   while not (root/'child-ready').exists() and time.monotonic()<end: time.sleep(.001)
   assert (root/'child-ready').exists()
   children=[]
   for task in pathlib.Path(f'/proc/{p.pid}/task').iterdir(): children.extend((task/'children').read_text().split())
   assert children
   os.kill(p.pid,signal.SIGKILL); stopped=False; rc=p.wait(timeout=3)
   alive=[pid for pid in children if pathlib.Path('/proc',pid,'stat').exists() and pathlib.Path('/proc',pid,'stat').read_text().split()[2]!='Z']
   print('parent_exit=',rc,'child_alive=',bool(alive),'fixture_exists=',root.exists(),'child_pids=',children)
  finally:
   if stopped:
    try: os.kill(p.pid,signal.SIGCONT)
    except ProcessLookupError: pass
   try: os.killpg(p.pid,signal.SIGKILL)
   except ProcessLookupError: pass
   if p.poll() is None: p.wait(timeout=3)
PY
```

Output: `parent_exit= -9 child_alive= True fixture_exists= True child_pids= ['442262']`. The child PID is specific to this run. A subsequent process listing found no live `recovery_staging` process.

## Frozen source identity

The following exact command was run before and after all probes. Every value matched between the two reads, and all nine frozen values in the updated handoff matched. Additional files were checked to identify the reviewed context.

```sh
sha256sum Cargo.lock Cargo.toml crates/archy-core/Cargo.toml crates/archy-core/src/storage.rs crates/archy-core/src/run.rs crates/archy-core/tests/storage.rs crates/archy-core/tests/recovery_staging.rs crates/archy-cli/src/run_integrity.rs crates/archy-cli/tests/core_workflow.rs crates/archy-cli/tests/golden_artifacts.rs docs/plans/a17-recovery-handoff.md docs/recovery.md docs/plans/a17-recovery-review.md
```

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `Cargo.toml` | `2e9a70f386ec6f57c88300dd18c6ab210efcc4b449c7ea1a4c325ee62336665d` |
| `crates/archy-core/Cargo.toml` | `426bfac83847bae297fffea306294a32e611bd61d264fad64c2b96c9023e112f` |
| `crates/archy-core/src/storage.rs` | `6d061879feafddff6f90918f0f454b78592b75a810acca615d0e8d0d106d01c8` |
| `crates/archy-core/src/run.rs` | `0d04853449b8bfa47d4fa626a98b4c3b5156c509bab9ec1d5f3985fc4cf8b86c` |
| `crates/archy-core/tests/storage.rs` | `252ba273a5e6afc1705be96ef8b421c73a2e423c271a13061a4a6e9670b5bf97` |
| `crates/archy-core/tests/recovery_staging.rs` | `e882f96d861e94b47b916b6af2610278200014bdad677fa3a78cfbfb7dcbf7dc` |
| `crates/archy-cli/src/run_integrity.rs` | `13f4d1445421f9de6a97ddc4431e2ef52b59b54f4a078d4faed1375d7e25b1ef` |
| `crates/archy-cli/tests/core_workflow.rs` | `48ad2ca1a930d3f614bfd19ff67d9517befdf61449bb1446094950ab950e1d70` |
| `crates/archy-cli/tests/golden_artifacts.rs` | `978a048e78d98657eef1835bc8fb845b4cc2556a8991cd3bc2333d21e0d9611d` |
| `docs/plans/a17-recovery-handoff.md` | `4a603c9028e3230fa23425a0c3c9eeb62d4a9fb6c2a99076a64750a6f3626097` |
| `docs/recovery.md` | `6e74ff22aa833782a46be2186e07bd721fa9b1f51205e8f1bd72911a303a4556` |
| `docs/plans/a17-recovery-review.md` | `d952ca47ad4561001bb10c7428c40aa3c224ca14075865bc481b0eb0996b1ba4` |

The author reports wider core, CLI, Clippy and formatting checks; they were not independently rerun in this round. Full A17 still requires its other fault boundaries, backend logs and cancellation, concurrent/MCP single-apply, deterministic manifest/metric reruns and applicable target verification.
