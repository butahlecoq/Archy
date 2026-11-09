# Independent CI desktop lock rereview

**Scoped PASS:** no new exclusion failure was found in the frozen revision for the exercised Linux process, fork, exec, PID namespace, and `/proc` fallback cases. The previous review's actual separate-process probe now reports `parent_guard_alive=true competitor_acquired=false`. The revised guard fixes a real lock-lifetime mechanism that can affect product callers; neither the earlier PR failure nor push run `40928920889` has an identified lock holder, so the mechanism remains a plausible explanation of those CI failures, not a captured root cause.

The revised guard records the owner's PID and active PID namespace identity. It explicitly unlocks only when the current identity matches; a forked child's destructor therefore closes its descriptor without unlocking its parent's guard. Verified inherited guards and pending writes/commits reject a different owner. Safe Rust borrows keep the owner guard alive during its own pending operations, and the active process identity cannot normally change during a guard method. The comparison includes the namespace because an inner namespace can reuse the parent's numeric PID. No retry or weakened `try_lock` path was added.

When identity cannot be captured, the guard uses close-only release. A forked child retaining the description can retain the lock until close or exit; that is the preexisting fail-closed liveness behavior. In this fallback, inherited guard operations are not owner-checked, but an ordinary independent lock remains excluded while the inherited description stays open. When identity was captured but later cannot be checked, tested guard operations fail with `PermissionDenied` and drop omits explicit unlock. Explicit unlock errors are discarded; with inherited references, an unlock failure can likewise extend the lock lifetime. No unlock failure was injected. Abort, `_exit`, and SIGKILL do not run `Drop`; a surviving inherited descriptor can therefore retain the lock until it closes.

The raw-fork tests run in a dedicated `harness = false` executable with a single-threaded `main`, so their child-side Rust work is not performed after forking the usual multithreaded libtest harness. Its normal Cargo invocation runs all lifecycle checks. The separate storage test's `pre_exec` callback only does pipe I/O; it tests the opposite direction, owner drop while the child remains before exec. I found no unsound unsafe operation in the tested single-threaded fork scope. The PID namespace test requires user namespace support and is invoked separately; the ordinary CI Cargo test does not depend on that capability. The Cargo manifest's other diff is line-ending normalization.

## Independent results

- `fork_lock` lifecycle executable passed: child guard drop preserves parent exclusion against a separate execed contender; after owner release, inherited `begin`, pending `write`, and `commit` are denied; process exit is checked with and without a surviving inherited descriptor.
- Original round-one probe rebuilt against this revision: `inherited_guard_child=452755 competitor_child=452756 parent_guard_alive=true competitor_acquired=false`.
- `unshare --user --map-root-user --pid --fork ... fork_lock ... pid-namespace` passed. The test asserts parent and nested-namespace child both see PID 1 and that child drop leaves the parent's lock held.
- Storage integration tests passed 21/21, including owner-drop with child held before exec and a real execed competing process. The CLI desktop binary passed 100/100 runs with six test threads. Core Clippy with `-D warnings` and `git diff --check` passed.
- In an unprivileged user and private mount namespace, the retained acquisition-without-`/proc` probe printed `proc_unavailable=true inherited_child_conflict=true acquired_after_child_exit=true`. A separate independent probe acquired with `/proc` present, hid it in the private mount namespace, and printed `identity_lost_error=PermissionDenied inherited_conflict=RevisionConflict acquired_after_child_exit=true`.
- Full workspace tests were not independently rerun in this rereview; the handoff reports a pass. Stress passes do not prove the exact CI holder or guarantee a future CI pass.

## Snapshot hashes

Before and after checks, `sha256sum` returned exactly the handoff manifest:

```text
5e974a15c2df33a384f013afd2d5065eb208edadbe91504395946cd39d8c02ad  crates/archy-core/Cargo.toml
905a2934ae681e02710a3dccdfccde9a6cdb6aaae47cf3352ab417843e823d99  crates/archy-core/src/storage.rs
2e83ef2bb92db02683fbc1c5e05d8532dcc3c4a6a2c77e5d06aca637f84dd503  crates/archy-core/tests/storage.rs
5c10c79fc58a1e250bc09f96504c49737ce95d4470d301a190b5fce160db8277  crates/archy-core/tests/fork_lock.rs
70bed4a7d42132843d18e691adb50168f4ccc192f67e017573b23c8d97791dea  docs/plans/ci-desktop-lock-review.md
```

## Commands

From `/home/butahlecoq/work/archy/.worktrees/ci-desktop-lock`:

```sh
export RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent
sha256sum crates/archy-core/Cargo.toml crates/archy-core/src/storage.rs crates/archy-core/tests/storage.rs crates/archy-core/tests/fork_lock.rs docs/plans/ci-desktop-lock-review.md
cargo test --locked -p archy-core --test fork_lock -j 2 -- --test-threads=6
unshare --user --map-root-user --pid --fork /home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps/fork_lock-47d00a5a2b571647 pid-namespace
cargo test --locked -p archy-core --test storage -j 2 -- --nocapture
cargo test --locked -p archy-cli --test desktop_readers -j 2 --no-run
for run in $(seq 1 100); do output=$(/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps/desktop_readers-44961eb718d73bb7 --test-threads=6 2>&1); status=$?; if [ "$status" -ne 0 ]; then printf 'FAILED run=%s\n%s\n' "$run" "$output"; exit "$status"; fi; done; printf '100/100 six-thread desktop runs passed\n'
cargo clippy --locked -p archy-core --all-targets -j 2 -- -D warnings
git diff --check
```

The prior raw-fork source is included in `docs/plans/ci-desktop-lock-review.md`. It was recompiled and run against the revised library:

```sh
sed -n '/^use std::fs;$/,/^RS$/p' docs/plans/ci-desktop-lock-review.md | sed '$d' | RUSTUP_AUTO_INSTALL=0 rustc --edition=2024 -L dependency=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps --extern archy_core=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps/libarchy_core-b31ade73cb4dc09f.rlib -o /home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/fork-guard-probe-round2 - && /home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/fork-guard-probe-round2
```

The retained no-proc probe has SHA-256 `24d2fdf8cfdc0495e25bf88b2674e2954a055043ba87aeeb9de7470976220fff`. Its own source is at the path below:

```sh
RUSTUP_AUTO_INSTALL=0 rustc --edition=2024 -L dependency=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps --extern archy_core=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps/libarchy_core-b31ade73cb4dc09f.rlib -o /home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/no-proc-probe /home/butahlecoq/work/archy/.tools/targets/ci-desktop-lock/no-proc-probe.rs && ARCHY_NOPROC_ROOT=$(mktemp -d /tmp/archy-noproc-rereview.XXXXXX) unshare --user --map-root-user --mount --fork sh -c 'mount -t tmpfs tmpfs /proc && exec /home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/no-proc-probe'
```

The separate identity-lost-after-acquisition probe and exact commands were:

```sh
RUSTUP_AUTO_INSTALL=0 rustc --edition=2024 -L dependency=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps --extern archy_core=/home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/debug/deps/libarchy_core-b31ade73cb4dc09f.rlib -o /home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/proc-lost-probe - <<'RS'
use std::{fs,io::{Read,Write},os::unix::net::UnixStream};
unsafe extern "C" { fn fork()->i32; fn waitpid(p:i32,s:*mut i32,o:i32)->i32; fn mount(src:*const i8,dst:*const i8,ty:*const i8,flags:usize,data:*const std::ffi::c_void)->i32; fn _exit(s:i32)->!; }
fn main() {
 let root=std::env::temp_dir().join(format!("archy-proc-lost-{}",std::process::id())); fs::create_dir(&root).unwrap(); fs::write(root.join("project.json"),b"source").unwrap();
 let store=archy_core::ProjectStore::open(&root.join("project.json")).unwrap(); let guard=store.lock().unwrap(); assert_eq!(guard.revision().unwrap(),0);
 let (mut parent,mut child)=UnixStream::pair().unwrap(); let pid=unsafe{fork()}; assert!(pid>=0);
 if pid==0 { drop(parent); let mut b=[0]; child.read_exact(&mut b).unwrap(); unsafe{_exit(0)} }
 drop(child); assert_eq!(unsafe{mount(c"tmpfs".as_ptr(),c"/proc".as_ptr(),c"tmpfs".as_ptr(),0,std::ptr::null())},0); assert!(fs::metadata("/proc/thread-self/ns/pid").is_err());
 let err=guard.revision().unwrap_err(); drop(guard); let conflict=store.lock().err().unwrap(); parent.write_all(b"1").unwrap(); let mut st=0; assert_eq!(unsafe{waitpid(pid,&mut st,0)},pid); assert_eq!(st,0); let after=store.lock().is_ok();
 println!("identity_lost_error={:?} inherited_conflict={:?} acquired_after_child_exit={after}",err.code,conflict.code); assert_eq!(err.code,archy_core::ErrorCode::PermissionDenied); assert_eq!(conflict.code,archy_core::ErrorCode::RevisionConflict); assert!(after); fs::remove_dir_all(root).unwrap();
}
RS
unshare --user --map-root-user --mount --fork /home/butahlecoq/work/archy/.tools/targets/ci-lock-independent/proc-lost-probe
```

No product source, tests, CI, settings, main checkout, or author target were modified by this rereview.
