# A17 recovery review: round three

**Scoped verdict: PASS for the reproduced parent-death and staged-writer cases.** The round-two orphaned live-writer finding is addressed on the exercised Linux host. The helper stops after its test parent is killed; the owned fixture remains available as evidence until the surviving outer test or probe removes it. An uncatchably killed outermost process cannot run its fixture destructor, and no global cleanup is claimed. Full A17 and release acceptance remain open.

Reviewed base `57ba4f3b55d093da340adc5b4e05558efa3cc315`, plus the untracked updated test and documents in this worktree. I read `a17-recovery-review-round2.md`, the updated handoff, the complete test and recovery instructions. No source, commit, push, main checkout or host installation was changed. Test process creation was limited to the requested Cargo harness and isolated disposable probes; no child agents were used.

| Requirement | Verdict and evidence |
| --- | --- |
| Parent SIGKILL after writer staging | **PASS, scoped.** The independent round-two probe, modified only to poll for up to five seconds after parent death, printed `parent_exit= -9 child_alive= False fixture_exists= True`. The helper was no longer running; a zombie or absent PID counts as stopped, not as reaped. The disposable probe then killed its isolated process group and removed its own root. The new Cargo test independently stages a nested writer, kills its test parent, checks the writer is absent/zombie, and confirms revision 1 and staged `unaccepted` bytes remain. |
| Race and bounded watchdog | **PASS for design and exercised boundary.** `UnixStream::pair` exists before spawn; the writer inherits its endpoint on stdout and starts a reader thread before staging. Parent endpoint closure produces EOF even if death occurs before that reader is scheduled. `set_read_timeout(30s)` bounds its single-byte read; the `watchdog_exits_when_channel_stays_open_past_timeout` test uses 100 ms to exercise timeout with its peer open. The kernel socket/descriptor mechanism is a test-harness supervision measure, not product worker supervision; arbitrary filesystem stalls and all possible OS errors were not injected. |
| Unrelated path and malformed inherited environment | **PASS for the exercised cases.** An environment path is not read as authority. The original path probe with `ARCHY_RECOVERY_CHILD_ROOT` and null stdin returned 0 and left only the unrelated `project.json`. The Cargo test also supplies an unrelated path while passing a real fixture directory descriptor, and verifies the unrelated project has no `.archy` and unchanged bytes. A caller explicitly passing a directory descriptor and socket supplies those capabilities; an environment path alone does not. |
| Panic unwind | **PASS for the forced missing marker and data cases.** The Cargo test catches the expected panics, checks the writer was waited with signal 9, and checks its owned fixture was removed. The original external missing-file probe returned parent 101 and `child_alive_after_parent_panic=False`. `Drop` suppresses cleanup errors, so this does not prove cleanup after filesystem or process-control errors. |
| Real staged-writer kill, hash preservation and replay | **PASS for one staged boundary.** The helper writes `unaccepted` before its readiness marker. The parent confirms staged bytes, kills and waits for signal 9, reopens the store in the surviving process, checks revision 1 and refusal of the unaccepted run, then retries the identical `request("interrupted", 1)`. It commits revision 2 once, replay returns the exact receipt, and first-run accepted data/manifest/revision hashes remain unchanged before and after retry. The retry writes `after-restart`: request identity is proven, not deterministic artifact equality. |

## Independent commands and results

```sh
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-review cargo test --locked --offline -p archy-core --test recovery_staging -- --nocapture
```

Exit 0: **8 passed, 0 failed**. Three helper tests are no-ops when no directory/socket capability is supplied. Two expected read panics are caught by the unwind test. The main recovery test printed `staging child termination signal: Some(9)` and `retry committed revision: 2`. Its before/after accepted hashes were data `070c160a6299c5438070b1aa737b14fc2992ed49579c14264884886a5876f971`, manifest `53f76822bc717de4708c164a98240998bd6ace967265c48fef0da7cd282f3670`, and revision 1 `0f8fefcf1b385ae28a564b76884759834fb09188ca068accdfb7bec5c1322ef6`. Run UUIDs change the latter two across invocations.

Original round-one path and panic scripts, run verbatim from the stored review against the compiled SSD target:

```sh
python3 - <<'PY'
import pathlib,re,subprocess,sys
review=pathlib.Path('docs/plans/a17-recovery-review.md').read_text()
blocks=re.findall(r"```sh\npython3 - <<'PY'\n(.*?)\nPY\n```",review,re.S)
for label,block in zip(('path','panic'),blocks[1:]):
 result=subprocess.run([sys.executable,'-c',block],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=40)
 print(label,'exit',result.returncode); print(result.stdout,end=''); print(result.stderr,end=''); assert result.returncode==0
PY
```

Output: `path exit 0`, `files_created: ["project.json"]`; `panic exit 0`, staged bytes `b'unaccepted'`, `parent_exit=101`, `child_alive_after_parent_panic=False`, `missing_file_panic=True`. The older script's stdout-only `stale_revision_conflict` field is irrelevant to path isolation.

Exact independent outer parent-kill probe, reusing the prior review's script and adding a bounded wait because immediate observation after SIGKILL races the watchdog:

```sh
python3 - <<'PY'
import pathlib,re,subprocess,sys
review=pathlib.Path('docs/plans/a17-recovery-review-round2.md').read_text()
blocks=re.findall(r"```sh\npython3 - <<'PY'\n(.*?)\nPY\n```",review,re.S)
block=blocks[-1]
old="   print('parent_exit=',rc,'child_alive=',bool(alive),'fixture_exists=',root.exists(),'child_pids=',children)"
new="   limit=time.monotonic()+5\n   while alive and time.monotonic()<limit:\n    time.sleep(.01)\n    alive=[pid for pid in children if pathlib.Path('/proc',pid,'stat').exists() and pathlib.Path('/proc',pid,'stat').read_text().split()[2]!='Z']\n   print('parent_exit=',rc,'child_alive=',bool(alive),'fixture_exists=',root.exists(),'child_pids=',children)\n   assert rc == -9 and not alive and root.exists()"
assert old in block
result=subprocess.run([sys.executable,'-c',block.replace(old,new)],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=20)
print('probe exit',result.returncode); print(result.stdout,end=''); print(result.stderr,end=''); assert result.returncode==0
PY
```

Output: `probe exit 0`, `parent_exit= -9 child_alive= False fixture_exists= True child_pids= ['480060']`. PID is invocation-specific. The preserved fixture is expected evidence; the disposable probe's `finally` removes its own root after inspection.

## Source identity and limits

The following command was run before and after the tests; every listed hash matched both times and all author-frozen values matched. The handoff hash was captured independently as well.

```sh
sha256sum Cargo.lock crates/archy-core/src/storage.rs crates/archy-core/src/run.rs crates/archy-core/tests/recovery_staging.rs crates/archy-cli/src/run_integrity.rs crates/archy-cli/tests/core_workflow.rs crates/archy-cli/tests/golden_artifacts.rs docs/recovery.md docs/plans/a17-recovery-handoff.md docs/plans/a17-recovery-review.md docs/plans/a17-recovery-review-round2.md
```

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `crates/archy-core/src/storage.rs` | `6d061879feafddff6f90918f0f454b78592b75a810acca615d0e8d0d106d01c8` |
| `crates/archy-core/src/run.rs` | `0d04853449b8bfa47d4fa626a98b4c3b5156c509bab9ec1d5f3985fc4cf8b86c` |
| `crates/archy-core/tests/recovery_staging.rs` | `8159de07de83f61e37f4431f3771d8d4168568625296092e7344b9dea468cab7` |
| `crates/archy-cli/src/run_integrity.rs` | `13f4d1445421f9de6a97ddc4431e2ef52b59b54f4a078d4faed1375d7e25b1ef` |
| `crates/archy-cli/tests/core_workflow.rs` | `48ad2ca1a930d3f614bfd19ff67d9517befdf61449bb1446094950ab950e1d70` |
| `crates/archy-cli/tests/golden_artifacts.rs` | `978a048e78d98657eef1835bc8fb845b4cc2556a8991cd3bc2333d21e0d9611d` |
| `docs/recovery.md` | `547f16e1e44bbeab701720061d3539bce88397d727fc20ba92b9b7379df55160` |
| `docs/plans/a17-recovery-handoff.md` | `cb814d859d8d70932627ebebf0bff2a3344bbc44720fb23b167ac098340b080f` |
| `docs/plans/a17-recovery-review.md` | `d952ca47ad4561001bb10c7428c40aa3c224ca14075865bc481b0eb0996b1ba4` |
| `docs/plans/a17-recovery-review-round2.md` | `5c35012c9af0854d7d6a34313a464c1e348e9629aa106790f6b2cb3dbedf5b6d` |

Sudden-power-loss durability, real backend or MCP recovery, real Omarchy and the remaining A17 transaction/fault matrix were not tested. The broader checks in the handoff are author-reported, not independently rerun in this review.
