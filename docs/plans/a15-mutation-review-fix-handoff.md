# A15 mutation review repair handoff

Worktree: `/home/butahlecoq/work/archy/.worktrees/a15-generate-evaluate-20261029`, base `f1e2477047dbafbacfba3229daddab0ca58ebe02`. This is an uncommitted repair of the two findings in `a15-generate-evaluate-review-sol.md`. Independent Sol review and integration remain outstanding. Published compare commit `8e36e11d1399dee37dc2b85a8e2225b473571602` is absent from this base. Integrate compare's live handler, output schema, `compare_stdio.py`, and shared server/tools/docs/schema runner edits with this slice; no other worktree was modified.

## Transaction behavior

`BoundProjectRead` checks the configured root descriptor identity, source inode and source bytes at entry, after acquiring the storage lock, immediately before the action, and after verifying the returned accepted run. MCP passes the same binding check into storage. `PendingRun::commit_checked` checks immediately before the staged run rename and again after run publication, before revision publication. A changed binding before the run rename refuses with `REVISION_CONFLICT` and leaves uncommitted staging evidence for normal recovery. A change detected after the run rename returns an explicit retriable uncertain error. A change detected after revision publication likewise returns an explicit retriable uncertain error; it does not roll back or delete the accepted revision. Exact same-key retry through the restored binding recovers an orphan or replays the accepted receipt once. Descriptor-relative writes never target the replacement root.

A filesystem check followed by rename is not atomic with an unrelated process renaming the configured path. A swap after the last check, a swap away and back between checks, or a swap between the final check and response may escape detection. The returned run path is therefore a receipt for the pinned project identity as observed at the checks, not an OS-enforced lease on its configured pathname. An OS-enforced namespace or cooperative external lock would be needed for a stronger pathname guarantee. Review this limit explicitly; the implementation does not claim one.

Evaluation validates its source operation and requested variant before `begin` for a new idempotency key. Generation validates count before `begin` for a new key. Existing keys still pass through storage replay/conflict first, preserving changed-request conflict priority and exactly-once semantics. No cleanup deletes staging evidence or accepted state.

## Evidence

The live stdio probe in `crates/archy-mcp/tests/mutations.py` observes a real staging directory, replaces root or source while generation is in flight, requires a non-success response, then restores the binding and retries the same key, requiring one accepted revision and exact replay. It also requires invalid evaluation selector and count calls to preserve the staging and revision inventories. The core storage test uses a separate filesystem worker and channels to swap root or source at the pre-run and post-run publication checks, then verifies refusal/uncertainty and one accepted revision after retry. These probes are phase synchronized; they do not prove exclusion of the check/rename race described above.

Validation results and the SHA-256 manifest are recorded after the final checks below. No host install, commit, push, approval, or child agent was used.

Final checks used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-generate-evaluate-20261029`:

- `cargo test --locked -p archy-core -p archy-mcp`: exit 0, including synchronized publication swap, recovery and storage tests.
- `cargo test --locked -p archy-cli --test core_workflow -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable`: exit 0, 19 tests. The long history test passed in an earlier run, which failed one selector conflict assertion before the ordering repair. That assertion passed separately after the repair. The full 20-test suite was not rerun after the repair.
- `cargo test --locked -p archy-cli --test desktop_readers`: exit 0, seven tests.
- `python3 scripts/check-schemas.py`: exit 0, including live stdio mutation and schema probes. An intermediate run failed only because the new test expected a tool error for `count: 0`; the schema correctly returned JSON-RPC `-32602`. The expectation was corrected and the full gate passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check`: exit 0.

`docs/plans/a15-mutation-review-fix-freeze.sha256` hashes the exact review files. Verify with `sha256sum -c docs/plans/a15-mutation-review-fix-freeze.sha256` from this worktree. The manifest includes this handoff and excludes itself.
