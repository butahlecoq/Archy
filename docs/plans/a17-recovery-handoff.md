# A17 recovery audit and handoff

Source baseline: `57ba4f3b55d093da340adc5b4e05558efa3cc315` on `codex/a17-recovery-audit`. This is a bounded A17 verification slice, not release acceptance. The release contract is `docs/specs/release-0.1.txt` sections 3.2, 5, 7, and 9 and `docs/plans/backlog.md` A17.

## Fault matrix before this slice

| Boundary or fault | Implemented behavior | Evidence at baseline | A17 gap |
|---|---|---|---|
| Accepted core run and revision | Project lock, bounded staged artifacts, file and directory sync, no-replace run and revision publication; accepted chain and artifact hashes checked on reads | `crates/archy-core/src/storage.rs`; `crates/archy-core/tests/storage.rs` | No power-loss or target filesystem proof |
| Sync or publication error | Private test-only EIO points cover staging, run, revision, orphan recovery, and replay sync; uncertain publication asks for same-key retry | `storage.rs` interruption tests; `docs/plans/task03-storage-{handoff,review-round1}.md` | Synthetic EIO does not model process death at every point or real kernel sync failure |
| Process death after run publication | Existing child abort leaves a published orphan; same request recovers one revision | `storage.rs::interruption_tests::aborted_child_run_is_recovered_once` | No separate proof for death while a run is still staged and a prior accepted run exists |
| Duplicate and stale mutations | Same key and request returns original receipt; changed request and stale revision conflict; persistent lock rejects a competing process | `crates/archy-core/tests/storage.rs`; `crates/archy-cli/tests/core_workflow.rs` | Concurrent CLI mutation race and MCP single-apply remain unproven |
| Deterministic design rerun | Seeded CLI runs preserve variant and evaluation bytes across independent project roots; golden fixture compares metrics and reports after mapping execution IDs | `crates/archy-cli/tests/{core_workflow,golden_artifacts}.rs` | No real backend image reproducibility or complete version-pinned target rerun |
| Editable history | CLI edit and restore tests retain prior evidence; CLI integrity recomputes evaluation and rejects consistently rehashed forged results | `crates/archy-cli/tests/core_workflow.rs`; `crates/archy-cli/src/run_integrity.rs` | No end-to-end interrupted edit or cancellation recovery |
| Backend process and artifacts | Adapter has bounded typed protocols, process outcome handling, and artifact validators | `crates/archy-adapters/src/{process,artifacts}.rs`; adapter tests | No integrated storage transaction wrapping Blender or IFC output; production render and export are incomplete |
| MCP mutation | Release spec requires nine tools with keys, revisions, destinations and human approval | `docs/specs/release-0.1.txt` section 5; no `crates/archy-mcp` source in this baseline | MCP implementation, duplicate request recovery, and approval gate remain open |

## Bounded plan

Use the published `ProjectStore` API in a new integration test. Commit one accepted run, start a separate process that stages the next run, terminate it with a real process kill after it reports that staging is complete, then reopen the store and retry the same request. Compare the first run's exact accepted inventory before and after; require one new revision and an identical replay receipt. Leave the abandoned staging directory intact for diagnosis. This exercises a distinct transaction boundary without modifying the active storage tests or production code.

## Prior art and dependency choice

The [Rust `File::sync_all` reference](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all) describes syncing file data and metadata and handling errors that `Drop` ignores. [SQLite's atomic commit notes](https://www.sqlite.org/atomiccommit.html) explain why sync ordering and filesystem assumptions matter after a process or power failure. This test uses only existing Rust standard library and locked workspace dependencies. A process kill tests OS process interruption; it does not establish sudden power-loss durability.

## Review status

`docs/plans/a17-recovery-review.md` found two harness issues. `docs/plans/a17-recovery-review-round2.md` confirmed those fixes and found an orphaned writer after test-parent SIGKILL. Both correction rounds are recorded below. Independent rereview of the latest version remains open.

## Observed recovery

The new `recovery_staging` test completed on Linux 6.6.87.2 WSL2 with Rust 1.99.1. The child reported a staged `data` artifact, then the parent terminated it with signal 9. Reopening the project found revision 1. `read_run` rejected the staged UUID as unaccepted. A retry with key `interrupted` and expected revision 1 committed revision 2. Replaying the identical request returned the same receipt, and the accepted receipt list contained exactly revisions 1 and 2. The abandoned staging artifact still contained `unaccepted`; the retry's artifact contained `after-restart`.

One captured invocation gave the following SHA-256 inventory. The manifest and revision hashes contain a random run ID and change between test invocations. The test compares the actual before and after maps within each invocation.

| Accepted file | Before kill | After kill |
|---|---|---|
| `runs/<first-run-id>/data` | `070c160a6299c5438070b1aa737b14fc2992ed49579c14264884886a5876f971` | `070c160a6299c5438070b1aa737b14fc2992ed49579c14264884886a5876f971` |
| `runs/<first-run-id>/manifest.json` | `878f8d4fd89b8e47c40689d44aca6895002b18c40a4474f5905eae581f9cff43` | `878f8d4fd89b8e47c40689d44aca6895002b18c40a4474f5905eae581f9cff43` |
| `.archy/revisions/00000000000000000001.json` | `152ca6a0743d4b352f71c2ae960195e577717da29c6b0c9abeff6343a89e73e8` | `152ca6a0743d4b352f71c2ae960195e577717da29c6b0c9abeff6343a89e73e8` |

The failure observation is the child's signal 9 and its unaccepted staged run. The pass observation is an unchanged accepted inventory and one successful same-request retry. This does not demonstrate failure-log retention for a backend job, because this test has no backend job.

## Commands and frozen source hashes

Commands run from this worktree, with `CARGO_BUILD_JOBS=2`, `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-recovery`, and `RUSTUP_AUTO_INSTALL=0`:

```sh
cargo test --locked --offline -p archy-core --lib storage::interruption_tests::aborted_child_run_is_recovered_once
cargo test --locked --offline -p archy-core --test recovery_staging -- --nocapture
cargo test --locked --offline -p archy-core
cargo clippy --locked --offline -p archy-core --all-targets -- -D warnings
cargo test --locked --offline -p archy-cli --test core_workflow --test golden_artifacts
rustfmt --check --edition 2024 crates/archy-core/tests/recovery_staging.rs
```

These commands exited 0 on the initial version, before the review correction. The full core suite then reported 7 unit, 2 recovery integration, 2 run-integrity, 20 storage integration, and 12 validation tests passing. The CLI command reported 16 `core_workflow` and 1 `golden_artifacts` test passing. `cargo fmt --all -- --check` and `git diff --check` also exited 0. The correction's checks are listed below.

Frozen source SHA-256 values after the final validation pass:

| File | SHA-256 |
|---|---|
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `crates/archy-core/src/storage.rs` | `6d061879feafddff6f90918f0f454b78592b75a810acca615d0e8d0d106d01c8` |
| `crates/archy-core/src/run.rs` | `0d04853449b8bfa47d4fa626a98b4c3b5156c509bab9ec1d5f3985fc4cf8b86c` |
| `crates/archy-core/tests/recovery_staging.rs` | `8159de07de83f61e37f4431f3771d8d4168568625296092e7344b9dea468cab7` |
| `crates/archy-cli/src/run_integrity.rs` | `13f4d1445421f9de6a97ddc4431e2ef52b59b54f4a078d4faed1375d7e25b1ef` |
| `crates/archy-cli/tests/core_workflow.rs` | `48ad2ca1a930d3f614bfd19ff67d9517befdf61449bb1446094950ab950e1d70` |
| `crates/archy-cli/tests/golden_artifacts.rs` | `978a048e78d98657eef1835bc8fb845b4cc2556a8991cd3bc2333d21e0d9611d` |
| `docs/recovery.md` | `547f16e1e44bbeab701720061d3539bce88397d727fc20ba92b9b7379df55160` |
| `docs/plans/a17-recovery-review.md` | `d952ca47ad4561001bb10c7428c40aa3c224ca14075865bc481b0eb0996b1ba4` |
| `docs/plans/a17-recovery-review-round2.md` | `5c35012c9af0854d7d6a34313a464c1e348e9629aa106790f6b2cb3dbedf5b6d` |

To freeze the review input again, run `sha256sum` on those files and compare this table. No dependency, production source, or active storage test was changed.

## Review correction and rereview evidence

Before editing, the review's disposable path probe exited 101 and created `.archy` in the supplied project. Its staged-file deletion probe exited 101, left its child alive, and left its fixture present. Both probes cleaned their disposable process group and directory afterward.

The helper now ignores `ARCHY_RECOVERY_CHILD_ROOT`. The parent exclusively creates a mode-0700 temporary fixture and passes an open directory descriptor as the child's standard input. The descriptor is the explicit capability to the fixture; the helper does not accept a project path from the environment. A scoped fixture removes its own directory on unwind. A scoped writer kills and waits for the child before fixture cleanup. The normal test retains the staged bytes until after the SIGKILL and recovery assertions.

The `inherited_path_cannot_select_the_writer_project` test checks both a standalone helper with null input and an active writer with a valid fixture descriptor while the environment points to an unrelated project. Neither changes the unrelated project. `parent_unwind_reaps_writer_and_removes_fixture` deliberately removes the readiness marker and staged file in separate cases, forces a read panic, and checks the guard recorded a signal-9 child exit and removed its fixture. These expected panics appear in `--nocapture` output and are caught by the test.

After the round-one correction, the following commands exited 0. The focused test then reported 4 passed, including the helper's standalone no-op and the two adversarial tests. The normal recovery test asserted signal 9, the unchanged accepted inventory, revision 2, the exact replay receipt, and retained unaccepted staging bytes. These results predate the parent-death correction below.

In the final focused run, the before and after SHA-256 values were identical for `data` (`070c160a6299c5438070b1aa737b14fc2992ed49579c14264884886a5876f971`), `manifest.json` (`ceb5d37c295fb908a4c1d52bda376276516b9265d3285a1a88af0432a75fa8f8`), and revision 1 (`e3d9dd887dee257ac64425f70208ecb39b9ccbf9ddc905ac7be4fb1e7645a93e`). Run IDs make the last two values vary between executions.

```sh
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-recovery RUSTUP_AUTO_INSTALL=0 cargo test --locked --offline -p archy-core --test recovery_staging -- --nocapture
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-recovery RUSTUP_AUTO_INSTALL=0 cargo clippy --locked --offline -p archy-core --test recovery_staging -- -D warnings
rustfmt --check --edition 2024 crates/archy-core/tests/recovery_staging.rs
```

The review's exact two probe scripts can be rerun against this target with the following command, from this worktree. The path probe returned exit 0 with only `project.json` present. The failure probe returned parent exit 101, `child_alive_after_parent_panic=False`, and `missing_file_panic=True`. A separate version of the same probe also checked `fixture_exists=False` after the parent exited.

```sh
python3 - <<'PY'
import pathlib,re,subprocess,sys
review=pathlib.Path('docs/plans/a17-recovery-review.md').read_text()
blocks=re.findall(r"```sh\npython3 - <<'PY'\n(.*?)\nPY\n```",review,re.S)
assert len(blocks)==3
for label,block in zip(('path','parent-panic'),blocks[1:]):
 script=block.replace('/.tools/targets/a17-review/', '/.tools/targets/a17-recovery/')
 result=subprocess.run([sys.executable,'-c',script],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=40)
 print(label,'exit',result.returncode)
 print(result.stdout,end='')
 if result.stderr: print(result.stderr,end='')
 if result.returncode: raise SystemExit(result.returncode)
PY
```

The child stages `unaccepted`, while the retry writes `after-restart`. This proves request identity and one published retry for this fixture, not deterministic artifact equality across the interruption. The separate CLI golden test compares deterministic variant and evaluation bytes. SIGKILL tests process interruption; sudden-power-loss durability remains unproven.

## Parent SIGKILL correction

Before the correction, the round-two disposable probe reproduced `parent_exit=-9`, `child_alive=True`, and `fixture_exists=True`. Its `finally` block killed the isolated process group and removed its own temporary root.

The test parent now passes the writer a private Unix socket as well as the existing directory capability. A writer thread reads the socket while the writer stages and waits. When the parent dies, its socket endpoint closes and the writer exits. EOF remains observable if the parent dies before the reader starts, so this mechanism does not depend on checking a parent PID after registration. The read also has a 30-second timeout. The focused timeout test exercises the same watchdog with a 100-millisecond timeout while the peer remains open. This is a test-harness mechanism, not product worker supervision.

The first compile attempt exited 101 because `Stdio` does not implement `From<UnixStream>`. Converting the socket to `OwnedFd` fixed that compile error; the final checks below use that version.

`sigkill_of_test_parent_stops_writer_and_preserves_owned_evidence` starts a separate test parent, waits until its writer has staged `unaccepted`, and sends SIGKILL to that parent. It checks that the writer's `/proc` entry disappears or becomes a zombie within 10 seconds, then verifies the retained revision and staged bytes. The surviving outer test removes its own fixture and checks that it is gone. A zombie is stopped but still needs its new parent to reap it; this test does not claim to reap the orphan. If the outer test runner itself is killed, no destructor runs and its private temporary fixture can remain. There is no global cleanup service. A caller that created a disposable root can inspect and remove that root after the process has stopped.

The final focused command below exited 0: 8 tests passed, including three standalone helper no-ops, the writer SIGKILL/replay case, the inherited-path case, both unwind fault cases, parent SIGKILL, and the shortened watchdog-timeout case. The expected caught panic messages are part of the unwind test. The final captured accepted inventory was identical before and after writer SIGKILL: data `070c160a6299c5438070b1aa737b14fc2992ed49579c14264884886a5876f971`, manifest `547d3e12aa42c49e80353f1dd1234b4b0fcf4c67a31ebfa28ad2119cc3871957`, and revision 1 `86cfda2b08c8581f58b1d15e73fb2f50e199c1ec42ea936ce9f533faca91f7db`. UUID-dependent hashes vary by run.

```sh
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-recovery RUSTUP_AUTO_INSTALL=0 cargo test --locked --offline -p archy-core --test recovery_staging -- --nocapture
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-recovery RUSTUP_AUTO_INSTALL=0 cargo clippy --locked --offline -p archy-core --test recovery_staging -- -D warnings
rustfmt --check --edition 2024 crates/archy-core/tests/recovery_staging.rs
```

All three exited 0. The prior review's path and panic probes also exited 0 with no unrelated project state and no live child after panic. The round-two parent-kill probe, with its target path changed from `a17-review` to `a17-recovery`, printed `parent_exit=-9 child_alive=False fixture_exists=True`. Its `finally` block removed the isolated test root. Run the exact probe from `docs/plans/a17-recovery-review-round2.md` with that target substitution to reproduce it.
