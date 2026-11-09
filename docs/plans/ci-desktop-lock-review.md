# Independent CI desktop lock review

**Finding: a forked child's guard drop releases a live parent's lock.** The candidate should not be merged with an unconditional `unlock()` in `Drop` if a `ProjectStore` caller may fork and drop the inherited guard in the child. In a single-threaded Rust process, I acquired a real `LockedProject`, verified that a second lock conflicted, forked, and had the child drop its inherited guard and exit. A second child then successfully acquired a new `ProjectStore` lock while the original parent's guard was still alive. Output: `inherited_guard_child=405120 competitor_child=405121 parent_guard_alive=true competitor_acquired=true`. The second child is a genuine separate contender. This is a new premature release caused by the candidate's explicit unlock of the shared open file description; closing only the child's inherited descriptor would leave the parent's descriptor and lock intact.

This finding requires a caller to fork without immediately execing and to drop the inherited guard in that child. The existing `Command::pre_exec` regression does not drop the guard, so it does not cover this case; the CI failure has not been tied to a captured forked child or inode. POSIX `flock` is advisory, and a process already holding the inherited descriptor could deliberately unlock it even before this change; the new issue is that ordinary guard destruction in the child now does so automatically. No repo product code directly calls raw `fork`, but `ProjectStore` is public and this is a concrete change to its lock lifetime. A narrow candidate correction is to record the acquiring process ID in the guard and explicitly unlock only when the guard is dropped in that same process. A forked child's drop then closes its descriptor without releasing the parent's lock. Recheck owner PID handling, the deterministic inherited-descriptor case, real contention, and this child-drop case before accepting that correction.

The candidate does address a demonstrated product-level lifecycle mechanism: on normal guard drop, explicit `flock` unlock releases the lock despite a child still holding the inherited description. It preserves nonblocking acquisition in the tested normal path and does not retry CI failures. The deterministic regression and all 21 storage integration tests passed independently, including `persistent_lock_rejects_competing_process`; that test exercises a separate `exec`ed contender while the guard is live. These passing tests do not cover the child-drop finding above.

## CI and evidence boundaries

`gh run view 40928920889 --attempt 1 --log-failed` confirms the push failure at `57ba4f3`: `desktop_readers.rs:136:10`, the same retriable `RevisionConflict` and message, with 5 passed and 1 failed. The prior PR failure and this push failure have the same observed symptom; the precise CI holder is unobserved. The previously documented local failure on parallel run 5 and the controlled fork probe support inheritance as a plausible mechanism, not proof of either CI process's identity.

`O_CLOEXEC` closes the descriptor on successful exec, not at fork. A normal `Command` child blocked before exec can retain the description; the new regression demonstrates release despite that child. `Drop` does not run on `abort`, `SIGKILL`, or `_exit`, so explicit unlock cannot cure a crash while an inherited child retains the description; the lock remains until all description references close or an explicit unlock occurs. The existing abort recovery test checks recovery after an aborted child with no demonstrated surviving inherited holder; it is not proof of immediate release in that latter case.

`Drop` discards `File::unlock` errors. If unlock fails and an inherited descriptor remains open, closing the parent's descriptor may leave the lock held until the child execs or exits: a liveness limitation. An unlock error is not evidence that a competing writer can acquire the lock; no failure injection was run. The lock descriptor is owned by the guard and ordinarily valid when `Drop` runs. Borrowing ties pending mutations to the guard, so safe code cannot normally drop that guard while such a borrow is live.

## Reproduction commands

The candidate files at start had these SHA-256 values, matching the handoff:

```text
d16c892bc38948b7c56080813cab52cd7d96c96f90bb27fad1c34af040a72ab0  crates/archy-core/src/storage.rs
2e83ef2bb92db02683fbc1c5e05d8532dcc3c4a6a2c77e5d06aca637f84dd503  crates/archy-core/tests/storage.rs
```

`RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent cargo test --locked -p archy-core --test storage -j 2 -- --nocapture` passed 21/21, including both specified lock tests, without compiler warnings.

The child-drop probe compiled the actual candidate `archy_core` library, with the following command and stdin source; it writes its executable only to the requested independent target directory and its fixture to a UUID-like PID-specific directory under `/tmp`:

```sh
RUSTUP_AUTO_INSTALL=0 rustc --edition=2024 \
  -L dependency=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps \
  --extern archy_core=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps/libarchy_core-b31ade73cb4dc09f.rlib \
  -o /home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/fork-guard-probe - <<'RS'
use std::fs;
unsafe extern "C" { fn fork() -> i32; fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32; fn _exit(status: i32) -> !; }
fn wait_child(pid: i32) -> i32 { let mut status = 0; assert_eq!(unsafe { waitpid(pid, &mut status, 0) }, pid); status }
fn main() {
    let root = std::env::temp_dir().join(format!("archy-fork-guard-probe-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("project.json"), b"source").unwrap();
    let store = archy_core::ProjectStore::open(&root.join("project.json")).unwrap();
    let guard = store.lock().unwrap();
    assert!(store.lock().is_err());
    let holder_child = unsafe { fork() };
    assert!(holder_child >= 0);
    if holder_child == 0 { drop(guard); unsafe { _exit(0) } }
    assert_eq!(wait_child(holder_child), 0);
    let competing_child = unsafe { fork() };
    assert!(competing_child >= 0);
    if competing_child == 0 { let acquired = store.lock().is_ok(); unsafe { _exit(if acquired { 0 } else { 1 }) } }
    let competitor_status = wait_child(competing_child);
    println!("inherited_guard_child={holder_child} competitor_child={competing_child} parent_guard_alive=true competitor_acquired={}", competitor_status == 0);
    drop(guard);
    fs::remove_dir_all(root).unwrap();
}
RS
/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/fork-guard-probe
```

The probe uses raw `fork` in a single-threaded process; it does not imply that the CI test did the same child-side drop.

Additional independent checks, with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent`:

```text
cargo test --locked -p archy-core --lib aborted_child_run_is_recovered_once -j 2 -- --nocapture
# passed 1/1
cargo test --locked -p archy-cli --test desktop_readers -j 2 --no-run
# built without warnings
for run in $(seq 1 30); do output=$(/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps/desktop_readers-44961eb718d73bb7 --test-threads=6 2>&1); status=$?; if [ "$status" -ne 0 ]; then printf 'FAILED run=%s\n%s\n' "$run" "$output"; exit "$status"; fi; done; printf '30/30 six-thread desktop runs passed\n'
# 30/30 runs passed, 6 tests each
cargo clippy --locked -p archy-core --all-targets -j 2 -- -D warnings
# passed
git diff --check
# passed
sha256sum crates/archy-core/src/storage.rs crates/archy-core/tests/storage.rs
# d16c892bc38948b7c56080813cab52cd7d96c96f90bb27fad1c34af040a72ab0  crates/archy-core/src/storage.rs
# 2e83ef2bb92db02683fbc1c5e05d8532dcc3c4a6a2c77e5d06aca637f84dd503  crates/archy-core/tests/storage.rs
```

The end hashes match the initial hashes. The full workspace suite was not rerun independently; the handoff reports that it passed. No source, test, CI, or settings changes were made in this review.
