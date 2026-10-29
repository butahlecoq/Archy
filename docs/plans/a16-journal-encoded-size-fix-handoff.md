# A16 journal encoded-size fix: independent rereview handoff

Date: 2026-10-29. Worktree: `/home/butahlecoq/work/archy/.worktrees/a16-human-decision-f1e2477-20261029`. Base: `f1e2477047dbafbacfba3229daddab0ca58ebe02`. Review finding: `docs/plans/a16-human-decision-review-sol.md`.

## Change and evidence

`Journal::append` now rejects a serialized record larger than the same 64 KiB bound enforced by `scan()`, before hashing, pending-file creation or rename. The existing 16 KiB raw payload limit remains. The regression drives the public journal API with 16,384 NUL characters, expects `InvalidInput`, verifies that no file exists and reads remain possible, then appends 10,000 NUL characters and verifies the payload after a fresh open. Probe records are non-decision data. This addresses the reviewed 98,450-byte accepted-event failure without changing the journal format or approval surface.

The test failed before the fix because `append` returned `Ok(1)` for the oversized encoded event. After the fix, `cargo test -p archy-authority --locked --offline` passed: 2 library, 13 socket boundary, 8 journal boundary and 1 display tests; no failures. `cargo fmt --all -- --check` passed. Commands used `CARGO_TARGET_DIR=$PWD/.cargo-target-a16-fix`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, and `RUSTUP_AUTO_INSTALL=0` for Cargo. The target is local and isolated.

`cargo test --workspace --locked --offline --quiet` also exited 0 from the same isolated target. It reported no failures and two ignored target gates. The existing `edit_history_beyond_sixty_four_runs_stays_readable_and_replayable` test took 108.98 seconds.
`cargo clippy -p archy-authority --all-targets --locked --offline -- -D warnings` exited 0.
Final `sha256sum -c docs/plans/a16-journal-encoded-size-fix.sha256` and `git apply --reverse --check docs/plans/a16-journal-encoded-size-fix.patch` passed. Repository-wide `git diff --check` exited 2 on the already present CRLF `Cargo.toml` members line (`trailing whitespace`); that unrelated file was left unchanged by this fix.

## Exact rereview identity and integration

`docs/plans/a16-journal-encoded-size-fix.patch` is the incremental two-file patch against the independently reviewed A16 source, not a replacement for the frozen full A16 patch. `git apply --reverse --check` passed in this fixed worktree. `docs/plans/a16-journal-encoded-size-fix.sha256` records exact SHA-256 hashes of both final source files and the incremental patch. Verify it with `sha256sum -c docs/plans/a16-journal-encoded-size-fix.sha256`. The older `a16-human-decision-source.sha256` remains the historical pre-fix identity and will fail for these two files by design.

| Artifact | SHA-256 |
| --- | --- |
| `authority/src/journal.rs` | `bd82ddb9e3de443fcd21e9cf8037c60f9638a540291cc6117157e772e4e6b655` |
| `authority/tests/journal_boundary.rs` | `c8f8121c1a81bbbca9b8b5ffb4f21971f86cf6c803a354911e4536798e60060a` |
| `docs/plans/a16-journal-encoded-size-fix.patch` | `d609fa3c88ff5b9cd629f05f30c04aaf3a1bf0ba4d26011170bcd4897b03025f` |

Integration head supplied by the user: `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1` (process fixture change atop `8e36e11`). Its diff from `f1e2477` does not touch `authority/src/journal.rs` or `authority/tests/journal_boundary.rs`; `Cargo.lock` overlaps the broader A16 branch and must retain both dependency sets when integrated. Apply the A16 source and then this incremental patch, or carry the two final source files directly, and rerun the regression on the integrated tree. Do not drop this check while resolving the lockfile. No commit or push was made for this fix.

## Remaining limitations

The daemon still has no journal append or approval writer; both packet channels deny decision opcodes. The journal remains an opaque record store without decision provenance, protected-history validation or an independent rollback anchor. Native distinct-UID systemd, logind, Polkit, packaging and power-loss gates remain untested here on WSL2. This fix does not authorize approved IFC status or a human-positive decision path.
