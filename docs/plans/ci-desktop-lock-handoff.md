# CI desktop lock handoff

Base: `ea6c37f5698f50b091e635945a55b8f5e41b8975`, branch `codex/ci-desktop-lock`. This is an uncommitted candidate. **A scoped independent rereview of the exact snapshot is required before publication.**

## Findings and change

The initial CI failure was `desktop_readers.rs:136`, a retriable lock conflict during sequential generation. The independent diagnosis in `/home/butahlecoq/work/archy/.tools/reviews/ci-lock-independent.md` reproduced the parallel failure on run 5 and got 100 isolated passes. A controlled probe showed that a forked child can keep a `CLOEXEC` open file description, and therefore its `flock`, alive until exec or exit. The exact child and inode involved in the CI failure were not captured; inheritance is a supported hypothesis, not proof of that particular event's owner.

The first candidate explicitly unlocked every `LockedProject` on drop. The independent review in `docs/plans/ci-desktop-lock-review.md` found that a child dropping its inherited guard unlocks the parent's still-live guard. Running its `fork-guard-probe` reproduced `parent_guard_alive=true competitor_acquired=true`. That candidate had SHA-256 `d16c892bc38948b7c56080813cab52cd7d96c96f90bb27fad1c34af040a72ab0` for `crates/archy-core/src/storage.rs` and `2e83ef2bb92db02683fbc1c5e05d8532dcc3c4a6a2c77e5d06aca637f84dd503` for `crates/archy-core/tests/storage.rs`. It must not be published.

An ordinary PID check passed that review case, but was insufficient: using nested PID namespaces, the parent and child both saw PID 1, and the child again released the live parent's lock. A controlled run of the PID-only intermediate candidate printed `owner_pid=1 child_pid=1 parent_guard_alive=true competitor_acquired=true`. A further real-guard probe showed that after an owning parent unlocks, an inherited child guard could begin, write and commit while a new guard held the lock. Its regression failed before ownership checks with `[1, 1, 1]` for those three operations.

The revised guard records the acquiring PID and PID namespace device/inode from `/proc/thread-self/ns/pid`. It explicitly unlocks only when the current identity matches. A guard with a verified owner refuses use from a different process, including inherited `PendingRun` writes and commits, so it cannot mutate after the owner releases the shared lock. If identity is unavailable at acquisition or drop, explicit unlock is omitted, preserving the original close-only exclusion behavior; when the identity was recorded but cannot be verified later, guarded operations return `PermissionDenied`. No retries or acquisition weakening were added. This intentionally restricts using a verified guard after fork and adds `/proc` identity checks to guarded operations.

Process termination without `Drop` still releases the lock when the last descriptor closes. If a child retains the descriptor after the owner exits or is killed, the lock remains until that child closes it or exits, as before; explicit unlock cannot run on `_exit`, abort or SIGKILL. Unlock errors are ignored in `Drop`, so a surviving inherited descriptor can retain the lock if unlock fails. No unlock-failure injection was run.

`crates/archy-core/tests/fork_lock.rs` is a dedicated single-threaded test executable, configured in `crates/archy-core/Cargo.toml`, so the forked child can exercise real Rust guard operations without invoking allocation-dependent code after forking a multithreaded test harness. It checks child guard drop against a separate process, inherited `begin`/write/commit, and owner termination with and without an inherited descriptor. The existing `dropped_lock_releases_when_forked_child_still_has_descriptor` checks the opposite direction: owner guard drop releases the lock while a forked child is held before exec. The separate PID namespace mode is run explicitly because creating user/PID namespaces is not universally permitted in CI. No CLI files or dependencies changed; core storage and the core test target can collide with parallel work in those files.

## Validation

All Cargo commands use `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock`.

- The original review executable reproduced the premature unlock exactly; the new child-drop regression failed on the first candidate with `competing process acquired lock`, then passed with ownership checks.
- The inherited mutation regression failed with `[1, 1, 1]` on the earlier candidate and passed after guarding operations.
- The standalone PID namespace probe failed on the PID-only intermediate candidate and passed with namespace-aware identity. The final dedicated test's namespace mode also exited 0 with parent and child both PID 1.
- A separate mount-namespace probe hid `/proc`: it printed `proc_unavailable=true inherited_child_conflict=true acquired_after_child_exit=true`, confirming the close-only fallback.

- Final `cargo test --locked --workspace -j 2` exited 0. The dedicated fork lifecycle executable passed; `storage` passed 21/21, including the owner-drop and competing-process checks; the core interruption tests passed 7/7, including aborted-child recovery; desktop readers passed 6/6. One pre-existing ignored test elsewhere in the workspace remained ignored.
- The final desktop test binary passed 100/100 runs with `--test-threads=6`.
- Final workspace Clippy with `-D warnings`, `cargo fmt --all -- --check`, `git diff --check`, Cargo metadata, and the schema script all exited 0. The schema script used `python3` because this host does not provide `python`.

Exact validation commands from the worktree root:

```sh
export RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock
cargo test --locked -p archy-core --test fork_lock -j 2 -- --test-threads=6
unshare --user --map-root-user --pid --fork /home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/debug/deps/fork_lock-47d00a5a2b571647 pid-namespace
cargo test --locked --workspace -j 2
cargo clippy --locked --workspace --all-targets -j 2 -- -D warnings
cargo fmt --all -- --check
cargo metadata --locked --format-version 1 --no-deps >/tmp/archy-ci-lock-final-metadata.json
python3 scripts/check-schemas.py
git diff --check
test_bin=/home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/debug/deps/desktop_readers-685808d9807c78c4
for run in $(seq 1 100); do if ! "$test_bin" --test-threads=6 >/tmp/archy-ci-lock-rereview-stress.log 2>&1; then cat /tmp/archy-ci-lock-rereview-stress.log; echo "FAILED run=$run"; exit 1; fi; done
```

The actual `/proc` fallback probe source is retained at `/home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/no-proc-probe.rs`, SHA-256 `24d2fdf8cfdc0495e25bf88b2674e2954a055043ba87aeeb9de7470976220fff`. It was compiled against the current `archy-core` rlib and run with:

```sh
RUSTUP_AUTO_INSTALL=0 rustc --edition=2024 -L dependency=/home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/debug/deps --extern archy_core=/home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/debug/deps/libarchy_core-b31ade73cb4dc09f.rlib -o /home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/no-proc-probe /home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/no-proc-probe.rs
ARCHY_NOPROC_ROOT=$(mktemp -d /tmp/archy-noproc-probe.XXXXXX) unshare --user --map-root-user --mount --fork sh -c 'mount -t tmpfs tmpfs /proc && exec /home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/no-proc-probe'
```

Exact source snapshot for rereview (SHA-256):

```text
5e974a15c2df33a384f013afd2d5065eb208edadbe91504395946cd39d8c02ad  crates/archy-core/Cargo.toml
905a2934ae681e02710a3dccdfccde9a6cdb6aaae47cf3352ab417843e823d99  crates/archy-core/src/storage.rs
2e83ef2bb92db02683fbc1c5e05d8532dcc3c4a6a2c77e5d06aca637f84dd503  crates/archy-core/tests/storage.rs
5c10c79fc58a1e250bc09f96504c49737ce95d4470d301a190b5fce160db8277  crates/archy-core/tests/fork_lock.rs
70bed4a7d42132843d18e691adb50168f4ccc192f67e017573b23c8d97791dea  docs/plans/ci-desktop-lock-review.md
```

No commit or push was made. The independent review above applies to the superseded candidate; this revised snapshot still requires a scoped independent rereview.
