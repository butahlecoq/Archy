# A02/A04 e61 long-history regression

## Candidate and freeze

- Candidate: `/home/butahlecoq/work/archy/.worktrees/a02-a04-e61-integration-20261030`.
- Base HEAD before and after: `e61c977d3e8b504dfafd2e81689e97f62c934e76`.
- Frozen patch: `/home/butahlecoq/work/archy/.tools/patches/a02-a04-e61-integration-20261030.patch`.
- SHA-256 of frozen patch before and after: `fb7d04e29a4a6b1f6d2e86494de82adcf9d42e32a1d32cf53c00dde36eeebe96`.
- `git diff --cached --binary --full-index | sha256sum` produced that same SHA-256 before and after. The 106 staged paths remained staged; `git diff --binary` was empty and there were zero untracked candidate files both times. The candidate was not edited.

## Exact test run

Target: `crates/archy-cli/tests/core_workflow.rs`, test `edit_history_beyond_sixty_four_runs_stays_readable_and_replayable`. It creates an initial run, performs 65 successive edits, verifies the latest and original runs, and checks for 66 receipts.

Working directory: `/home/butahlecoq/work/archy/.worktrees/a02-a04-e61-integration-20261030`.

```sh
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=1 \
  CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-a04-e61-long-history-sol-lMEToU5a \
  TMPDIR=/home/butahlecoq/work/archy \
  /usr/bin/time -p -o /home/butahlecoq/work/archy/.tools/targets/a02-a04-e61-long-history-sol-lMEToU5a/elapsed.txt \
  cargo test --locked --offline -p archy-cli --test core_workflow \
  edit_history_beyond_sixty_four_runs_stays_readable_and_replayable -- --exact --nocapture
```

The command's combined output was captured through `tee` to `/home/butahlecoq/work/archy/.tools/targets/a02-a04-e61-long-history-sol-lMEToU5a/test.log`, with `pipefail` and `${PIPESTATUS[0]}` recording Cargo's exit code separately in `exit-code.txt`. The unique target directory is on ext4. No timeout was imposed. No children/subagents were used.

**Result: pass.** Cargo exit code `0`. Relevant output:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9m 37s
     Running tests/core_workflow.rs (/home/butahlecoq/work/archy/.tools/targets/a02-a04-e61-long-history-sol-lMEToU5a/debug/deps/core_workflow-63a5d46bac4ee45c)

running 1 test
test edit_history_beyond_sixty_four_runs_stays_readable_and_replayable has been running for over 60 seconds
test edit_history_beyond_sixty_four_runs_stays_readable_and_replayable ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 342.80s
```

Elapsed from `/usr/bin/time -p`: `real 920.44`, `user 357.28`, `sys 44.31` seconds. Start `2026-10-29T23:07:49Z`, finish `2026-10-29T23:23:09Z` (30 October in Moscow). The 920.44 seconds includes fresh single-job compilation; the test itself took 342.80 seconds. Log SHA-256: `e8877eaf31a1a9f39b3f98ec1e2347a363e666f542f649c44b92f9cf5e2e769b`.

Scope limit: this invocation selected the exact named test; the other 22 tests in that integration binary were filtered out. This is direct evidence for the long-history regression on the frozen candidate, not a workspace-wide test result.
