# A09 scratch private mode rereview, round 2

**Scoped verdict: PASS.** The round-one private-mode finding is addressed. The receiver checks the owner and exact `0700` permission and special bits on the received directory FD. The systemd probe checks the same properties before it sends `GO`.

I ran the following checks in `/home/butahlecoq/work/archy/.worktrees/a09-supervisor`:

- `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-review CARGO_BUILD_JOBS=2 cargo test --locked -p archy-adapters --test isolation` exited 0 with 4 tests passed.
- `/usr/bin/python3 -B tests/security/systemd_scratch_fd_probe.py` exited 0. The real transient user service passed an owned directory FD with UID 1000 and mode `0700`. The probe reported 65,536 bytes, eight inodes, the required mount flags, an inactive collected service, and an absent cgroup after exit. Its retained FD read succeeded.
- `/usr/bin/python3 -B tests/security/systemd_mode_regression.py` exited 0 with `{"status": "pass", "mode": "0777", "start_token": "not_reached"}`. A disposable copy requested `0777` and placed a failure sentinel at the `GO` send. The real service probe refused at the private-mode check before that sentinel.
- An independently compiled disposable Rust probe sent an owned directory FD through real `SCM_RIGHTS` messages. Mode `0700` passed the mode check and reached the filesystem check. Modes `0777`, `0701`, `04700`, `02700`, and `01700` each returned the private-mode refusal. The process FD count was unchanged after every case. The `0700` directory was on a non-tmpfs filesystem, so this confirms the mode check, not full quota acceptance.

The passing service probe checked its host backing directory, outside sentinel, source-directory inventory, and retained-FD close. After both service tests, no `archy-scratch-fd-*.service` units or matching temporary paths remained. This is scoped WSL evidence; it does not prove absence of every possible host-side effect.

The frozen source hashes matched before and after the checks:

```text
cef81aaa9bc7434aa1d49bd16eaa3604dd956e8d388cb275c868ee321f0d81de  crates/archy-adapters/src/isolation.rs
8fc03ae3778cddb4c75436f7ffacdb4f82830765de2171e38b2268741bcf14f7  crates/archy-adapters/tests/isolation.rs
99586a26df00fc2af7846c9399a51c9944869eef621c740f26358f3076f7b614  crates/archy-adapters/src/lib.rs
ba4552770e011dcf62b1d8b2cf2063ad5bc1ad22fa7ba167472e4fac34394929  crates/archy-adapters/Cargo.toml
13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40  Cargo.lock
94be127b1039b3dadc1a1b2e888414bdc96a69641b252182b237fbcfac14d423  tests/security/scratch_fd_probe.py
1ce64b1debd429dadebc135b0f33b26090e99314f0c38f2711e0901831549fe8  tests/security/systemd_scratch_fd_probe.py
6a2a002117e5e6668850770a53d2a9fc2ede86b5f87304839927343a61b5c3b6  tests/security/systemd_mode_regression.py
8765b5ce51e59d705334e6f9dadb862ea136d68dbab6cff9a2ca367f53a0a114  docs/plans/worker-isolation-handoff.md
```

The author reports the full adapter tests, package format, and strict clippy passing; I did not rerun those broader gates. A positive end-to-end Rust quota receiver on the transient service remains unverified. The fixed frame does not authenticate a service or job. Future integration must bind service identity and cgroup limits, prove writer teardown, and establish containment before capture. Actual Omarchy acceptance remains deferred.

Pstack's **Boundary Discipline** focused the review on the received FD and pre-token checks. **Prove It Works** led to real descriptor and user-service probes.
