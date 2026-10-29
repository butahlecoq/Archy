# Project lock integration evidence

The independently reviewed lock change was copied from its `ea6c37f` worktree
onto integration commit `6ff5d47`. The reviewed source hashes matched before
integration. The review in `ci-desktop-lock-review-round2.md` exercises owner
and inherited guard behavior, PID namespaces, missing `/proc`, and a separate
competing process. It also reports 100 successful parallel desktop test runs.

GitHub Actions runs 40926887755, 40928920889, and 40931535999 failed with a
project lock conflict in desktop or comparison tests. The first two passed on
rerun. The particular lock holder was not captured, so inherited descriptors
are a plausible cause, not a proven explanation of those CI failures.

The combined tree was checked with a newly created Cargo target directory at
`.tools/targets/ci-combined-f506-6ff5-fix-fresh`, offline and with two build
jobs. `cargo fmt --all -- --check`, `cargo clippy --locked --workspace
--all-targets -j 2 -- -D warnings`, and `cargo test --locked --workspace -j 2`
passed, as did `python3 scripts/check-schemas.py`. Logs are under
`.tools/ci-combined-fresh-logs` in the workspace wrapper.

An earlier combined Clippy run reused a target directory from another worktree
and failed with unresolved imports despite the exports being present in source.
The fresh-target checks supersede those cross-worktree cache results.

If process identity cannot be established, the guard falls back to close-only
release; inherited descriptors can retain the lock. Dropping an owner is not
possible after abrupt termination. These checks do not establish that a future
GitHub Actions run will pass.
