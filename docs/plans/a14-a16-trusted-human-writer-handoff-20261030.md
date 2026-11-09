# A14/A16 protected writer follow-on: independent review handoff

30 October 2026. Isolated worktree: `.worktrees/a14-a16-trusted-human-boundary-20261030`. This follow-on builds on the round-two reviewed protected evidence and bus identity candidate. It has no commit, push, install, or child-agent review.

## Change and authority boundary

- `ProtectedProject::initialize` creates exactly `project/project.json` below a caller-supplied, administrator-fixed broker state directory. It requires the current broker UID, validates the bounded JSON source before writing, checks every ancestor without symlinks, creates the private directory and file exclusively, syncs the file and both directory entries, and rechecks path identity. Existing or interrupted initialization is refused; no agent-owned source file or accepted receipt is adopted.
- `ProtectedProject::generate_accepted` uses the CLI's `BoundProjectRead::generate` under the project lock. The broker computes the source, variants, evaluations, manifest and accepted revision from its protected source; callers supply only count, seed, idempotency key and expected revision. It rechecks the pinned path/source, verifies the published run against accepted history, caps new generation at 32 variants and the history at 16 revisions. Exact replay returns the same receipt; changed requests, stale revision, changed source and over-limit counts fail. This is an accepted-run writer, not a decision writer.
- `archy-project-ingest` is an administrator-invoked broker-UID binary with a fixed `/var/lib/archy-authority` state root. It takes bounded source JSON on stdin and `COUNT SEED`, initializes the single project, publishes a generated run and prints the receipt only after the write succeeds. It is not staged by the package or called by the daemon. A failed initialization/generation may leave private partial state for administrator recovery; it never adopts it on retry.
- The real generated-run review fixture now uses this writer. It still builds the A14 proposal only *after* reading the accepted broker-generated artifacts, then verifies the exact review snapshot. No proposal check or test-provided actor/authentication claim authorizes a decision.

## Exact local validation

- `cargo test --locked --offline -p archy-core -p archy-authority`: passed. Authority: 3 library, 1 review CLI, 13 boundary, 8 journal, 9 protected-project tests; core unit/interruption 8, decision contract 4, recovery staging 11, run integrity 2, storage 21, validation 12, plus fork-lock and doc tests.
- `cargo clippy --locked --offline -p archy-authority --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `python3 authority/tests/disposable_bus.py`: passed on the private development bus; it reported `ProcessFD`. It does not exercise a Rust service, target bus, logind or Polkit.
- The original round-two source freeze remains at `a14-a16-trusted-human-source.sha256`. The new `a14-a16-trusted-human-writer-source.sha256` freezes the current 13 source entries, including the new binary; verify with `sha256sum -c` from this worktree. The round-two review's hashes describe its earlier candidate and are intentionally retained as historical evidence.

## Integration and dependency notes

- Published integration head is `3b87d82871d7711873335b7fa76a787929b057d4`, with the reviewed A09 gate/accepted-run capture atop A15/A16. This worktree is older. Preserve the integration head's accepted-run capture and A15 behavior when applying this slice; do not overwrite its CLI/core files with this worktree's copies. The writer calls the existing public `BoundProjectRead` and `verify_history` APIs. Reconcile against the integrated A02 v2 revision/history schema before accepting an old lockfile or history implementation.
- This follow-on adds one local binary target and no registry dependency. The prior candidate's `archy-cli` and `rustix` edges are already represented in its lockfile. No new license notice closure is introduced; the authority binary remains unstaged, so its eventual package inventory and runtime closure still need review.

## Remaining authorization work

The daemon still stores A14 proposals in a separate private proposal store and does not serve protected snapshots or call this writer. There is no production system-bus method bound to the method header's unique sender and `GetConnectionCredentials` ProcessFD, no live active local logind session proof, no installed trusted human display or affirmative action, and no verified fresh non-retained Polkit `auth_self` challenge. There is no bounded sender/session/snapshot lease, no final project-lock head/digest recheck followed by journal decision append, and no approved projection. `OP_DECIDE` remains denied on both seqpacket channels. A broker/root actor can still roll back the journal tail without an external anchor. Native Omarchy, distinct-UID, mount-race and real human-positive tests remain deferred. **No human decision or approval is produced; IFC stays DRAFT.**

Independent review should try state-path replacement during initialization, partial-write recovery, exact replay after uncertain run publication, accepted-artifact tampering before a second generation, and merging the writer with A02 v2/A09 on the published integration head.
