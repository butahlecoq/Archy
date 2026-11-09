# A15 mutation repair: independent round-two review

Scope: the uncommitted A15 generate/evaluate repair at `f1e2477047dbafbacfba3229daddab0ca58ebe02` in this worktree. I read `AGENTS.md`, release spec §5 and §9, the prior review, repair handoff and freeze, mutation slice, `storage.rs`/`PendingRun`, shared CLI entry points, MCP server, schemas, and live tests. No source was edited. This review used one process as reviewer, with no child agents or other worktree writes.

## Verdict

**Accept the two scoped repairs for A15 review, with a pathname-race limitation; do not treat this as integration approval or target Omarchy acceptance.** No new verified violation of the repair intent survived the probes below. The former staging leak for a missing evaluation variant is gone in the exercised path. The former false success on an observed root/source replacement is gone at the checked publication phases. A concurrent, uncooperative rename remains outside the guarantee because pathname checks and `renameat`/revision publication are separate operations.

## Findings and evidence

1. **Observed committed-but-detached outcome is honest and recoverable.** I used a separate live Python adversary thread watching the revision directory with Linux inotify `IN_MOVED_TO`. On revision-file publication it renamed `bound` to `moved`, made a replacement `bound`, and copied the project source. The real `archy-mcp` stdio request returned `ok:false`, `REVISION_CONFLICT`, `retriable:true`, with “commit may have occurred” in the message. The pinned `moved` root contained exactly one revision; the replacement root contained zero. After restoring `moved` to `bound`, the identical key/request returned revision `1`; revision count remained one. I repeated the post-revision inotify schedule for source replacement: it also returned `REVISION_CONFLICT`, `retriable:true`, with one revision, then replayed revision `1` after restoring the original source inode. This proves detection and exact-key replay for these observed schedules, not an atomic path lease.
2. **All four synchronized core publication cases pass.** `binding_swap_at_publication_boundaries_never_returns_success` ran root and source replacement at callback 1 (before run rename) and callback 2 (after run publication, before revision publication). At callback 1 it refused without an accepted revision; at callback 2 it returned a retriable uncertain error and recovered the published orphan on retry. The test asserts one accepted revision and one receipt after retry for each case. The live stdio `mutations.py` also swaps root and source after staging appears and requires a non-success response, restored-binding retry, and a single revision. These are actual processes and filesystem renames, but the callback test synchronizes at checks; neither excludes a swap between a check and an operation.
3. **Selector and argument rejection preserve owned staging.** The live stdio test snapshots `.archy/staging`, rejects an absent variant with `INVALID_SCHEMA`, rejects `count:0` as JSON-RPC `-32602`, then asserts unchanged staging and revision inventories. It also exercises overflow, traversal, and symlink destinations. The CLI `changed_absent_evaluation_selector_conflicts_before_validation` test passes: an already recorded key with changed selector receives conflict priority; exact recorded requests replay. Code review confirms `evaluate_locked` resolves recorded keys through `begin` before new-key source/selector validation, while new-key selector validation precedes `begin`. `generate_locked` checks a new-key count before `begin`. These tests establish the stated cases, not a universal guarantee that every later computation failure leaves staging empty.
4. **Pinned path and symlink semantics are coherent in the exercised cases.** `ProjectStore` writes relative to its opened root descriptor; managed descendants use scoped `openat2` and publication uses `NOREPLACE`. `BoundProjectRead` checks current root device/inode, source device/inode, and pinned bytes at entry, after lock, before action, at the two publication boundaries, and after accepted-run verification. A static symlink to the root was accepted by a live stdio generation; retargeting that symlink before the next request returned `REVISION_CONFLICT`, leaving one revision in the original target and no `.archy` in the new target. The core/desktop tests also cover symlinked source refusal and managed-path substitution. A symlink in the configured root path is followed for the binding comparison, so this is identity binding to its target, not a ban on symlinked root paths.
5. **Regular CLI service and wire contracts remain exercised.** The fresh-target Rust run passed core, CLI, and MCP suites with the single long history test skipped; the focused CLI core workflow had 19/19 and desktop readers 7/7. The fresh-target schema gate passed nine advertised MCP input/output pairs, modern and legacy stdio, live mutation, framing, and CLI fixtures. This supports the regular CLI service and wire behavior in these cases. It does not review the separately published compare integration.

## Exact commands and outputs

All commands below ran from `/home/butahlecoq/work/archy/.worktrees/a15-generate-evaluate-20261029`. `df -T . /tmp /home/butahlecoq/work/archy/.cargo-target-a15-generate-evaluate-20261029` reported `/dev/sdd ext4` for the workspace and target, while `/tmp` was `tmpfs`; `lsblk -o NAME,ROTA,TYPE,MOUNTPOINTS` reported `sdd 1 disk /`. Thus the fresh target was on the workspace's ext4 device; the host reports it as rotational, so I cannot honestly label it a verified SSD. It was a new Cargo target directory, not a reused build cache.

```sh
sha256sum -c docs/plans/a15-mutation-review-fix-freeze.sha256
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-generate-evaluate-20261029 cargo test --locked -p archy-core binding_swap_at_publication_boundaries_never_returns_success -- --nocapture
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-generate-evaluate-20261029 cargo test --locked -p archy-cli --test core_workflow -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-generate-evaluate-20261029 cargo test --locked -p archy-cli --test desktop_readers
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-generate-evaluate-20261029 cargo test --locked -p archy-core -p archy-mcp
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-round2-20261029 cargo test --locked -p archy-core -p archy-cli -p archy-mcp -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-round2-20261029 python3 scripts/check-schemas.py
```

The freeze check reported `OK` for all 17 entries. The synchronized test ran **one** matching test and passed. (An initial invocation with `--exact` matched zero tests; it is not evidence.) The 19-case CLI workflow, seven desktop reader cases, core/MCP suite, fresh-target combined suite, and fresh-target schema gate all exited `0`. The combined suite explicitly filtered out the long 64-run history case. The schema gate printed `MCP live generate/evaluate receipts, replay, conflict, scope, tamper and replacement: PASS` and `nine MCP input/output schema pairs match SDK descriptors`. Cargo warned that the core and MCP example targets share the output filename `contracts`; it did not fail the run.

Independent live probes used `python3 - <<'PY'` with `subprocess.Popen([debug/archy-mcp, '--project', project])` and actual `tools/call` JSON lines. The post-revision probe used `ctypes.CDLL(None).inotify_init1(os.O_NONBLOCK)`, `inotify_add_watch(fd, revisions, 0x80)`, then, upon `select`/`os.read`, `bound.rename(moved); bound.mkdir(); shutil.copyfile(source, project)`. It printed:

```text
watched= True swapped_on_revision_rename= True watcher_error= []
response= {"data": null, "error": {"code": "REVISION_CONFLICT", "field": "project", "message": "Configured project changed during mutation; commit may have occurred on the pinned root; retry the same idempotency key after restoring the project binding", "retriable": true}, "ok": false}
revision_counts= 1 0
retry= {"data": {"revision": "1", "run_id": "8689bf5b-4667-40da-803a-997380edb081", ...}, "ok": true}
revision_count_after_retry= 1
```

The exact retry body used the original key `post-revision-swap`, revision `0`, count `100`, seed `42`, destination `runs`. The first setup attempt waited for `.archy/revisions` before sending a request and timed out; it created no finding. The corrected run sent the request before waiting for the inotify watch and produced the output above. The symlink probe used the fresh-target binary, generated through `bound -> first`, retargeted `bound -> second`, and sent a second key/revision. It printed `initial= True 1 retarget= False REVISION_CONFLICT first_revisions= 1 second_has_archy= False`.

The source variant used the same inotify trigger and request body with key `source-postrevision`, then executed `project.rename(saved); alternate.rename(project)` where `alternate` declared `alternate-demo`. It restored the original inode with `project.unlink(); saved.rename(project)` before retry. Output was `swapped= True error= [] first= False REVISION_CONFLICT True revisions= 1` and `retry= True 1 revisions= 1`.

### Hashes

```text
9a8809fc6f07fa7b4590802642a627aeda59d5509b71f98d931a9820c8cae285  docs/plans/a15-mutation-review-fix-freeze.sha256
bd2f59776b87a80ae70935d1f3880d739edcc59299a830927692587d031a2c6c  AGENTS.md
8e76b2b39c2fbc37eea9e5da808baaaf657884deee3b4fd961bce053b8c24235  docs/specs/release-0.1.txt
92d6ed9093452c4e6c7e4524a0d6a4ef7a9bd6300362c10a8e1ca0ac0c96c9d2  docs/plans/a15-mutation-slice.md
3340d14c47affd93782183b8ec0f3d526ad0e321d87ac00247cbf8ba2931d3f6  crates/archy-core/src/storage.rs
d480c984df898c2f3cf273cfc2b7b510d4852994644f4a5c1695faf562612b9c  crates/archy-cli/src/project_read.rs
192ea532911884ffedecfe130717f86b0d148e0d4ace46fa0add39932c77dff4  crates/archy-cli/src/generate.rs
45b92664839100e0e5a62162096c68ccf71b753cc8dfe2029e615ef21ccee265  crates/archy-cli/src/evaluate.rs
04bce67051dc2f830da9254d1789df81f6521c7aa9b22bc63588af1f93ffb3e2  crates/archy-mcp/tests/mutations.py
b401b658334ba513877b31ce8e57af1d5cbabd4a65061730b7644738de76ae1e  docs/plans/a15-mutation-review-fix-handoff.md
```

The freeze file supplies the remaining seven exact file hashes and all 17 were independently verified with `sha256sum -c`.

## Precise boundary and integration status

The implementation can refuse a change **observed at a check** before run publication, return an explicitly retriable uncertain result when observed after run publication, and avoid writing through the replacement pathname because storage writes use the pinned descriptor. It cannot ensure that the configured pathname still names the pinned root or source at every instant of publication or response. An uncooperative process can rename the root or replace the source just after the final check, swap away and back between checks, or retarget a root symlink in that gap. A success receipt therefore describes the pinned project and the observations made, not an atomic claim that the configured path continuously named it. A stronger guarantee needs namespace control or a lock observed by all renamers. The inotify probe demonstrated detection on one real post-revision schedule; it does not prove every schedule is caught.

`git merge-base HEAD 8e36e11d1399dee37dc2b85a8e2225b473571602` returned `f1e2477047dbafbacfba3229daddab0ca58ebe02`. Published compare commit `8e36e11` overlaps `server.rs`, `tools.rs`, `stdio_contract.py`, `docs/mcp/README.md`, and `scripts/check-schemas.py`, and adds `compare_stdio.py` and its schema. This worktree still exposes compare as unavailable. A02 v2 versioning and A09 accepted binding remain in flight and were not reviewed here. This verdict covers the frozen A15 mutation repair only; it makes no claim about a merged target, conflict resolution, approval, install, commit, or push.
