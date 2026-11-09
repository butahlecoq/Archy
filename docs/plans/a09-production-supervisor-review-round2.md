# Independent A09 supervisor rereview, round 2

Date: 2026-10-29. Worktree: `.worktrees/a09-production-supervisor`. Base commit: `a5a61c8e565d50928a7ccbf950ac061691eccf04`. Scope: the refreshed A09 handoff, prior review, revised `service.rs`, adapter export, frozen supporting files, and disposable user-service behavior. This is an independent direct review; this session had no Sol delegation interface, so it is not represented as a Sol-authored review. No product code, fixtures, host installation, commit, or remote state was changed. Build and probe output used the separate SSD target `.tools/targets/a09-production-supervisor-review-round2` and temporary directories.

## Verdict

The prior P1 false teardown success is fixed for denied cgroup lookup. The prior P2 early-loss path now retains a validated cgroup path when one was observed; if no path was ever observed, `stop()` explicitly fails unverified. The latter is safe but is not a cleanup proof. One narrower P3 discrepancy remains: a readable `cgroup.events` containing `populated 0` plus contradictory or malformed lines is accepted. No A09 end-to-end or Omarchy acceptance claim is possible because the packaged gate is absent.

## Source identity

SHA-256 at the start and after validation:

```text
416ba8d7d3f20bacb4522e29c5a7ec452bf0ec28ec13046464e8af90e6261d4a  crates/archy-adapters/src/service.rs
955e36333ae1bfb0bf81f2ad36ceff89a69d94d8a636854abd2a2d4f7a941e49  crates/archy-adapters/src/lib.rs
cef81aaa9bc7434aa1d49bd16eaa3604dd956e8d388cb275c868ee321f0d81de  crates/archy-adapters/src/isolation.rs
b1e4aa6c8071f4ed55f3776d937c862baf2e2ce69fb67c223fe32bb3b9179251  crates/archy-adapters/src/process.rs
8fc03ae3778cddb4c75436f7ffacdb4f82830765de2171e38b2268741bcf14f7  crates/archy-adapters/tests/isolation.rs
96823c6529a2f7f044cedc70910b323fc7695a1348be1e171ac653f6634c93d7  crates/archy-adapters/tests/systemd_receiver.rs
5804fc71df012661499d1a5c5c10065d79a089c3e5034db6c1bf2824068ced38  tests/security/systemd_rust_receiver_helper.py
13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40  Cargo.lock
```

The `service.rs` and `lib.rs` values match the refreshed handoff; supporting values match the frozen first review.

## Reproductions and review

**P1 closed.** A disposable mode-000 parent containing `unit.service/cgroup.events` with `populated 1` reproduced the original syscall behavior: `read=Err(PermissionDenied)`, `Path::exists()=false`, and the old predicate said `old_verified=true`. A Rust probe using the revised `cgroup_stopped` decision returned `Err(PermissionDenied)`. With the group still present but its events file removed, it returned `Err(NotFound)`; after removing the group, it returned `Ok(true)`. The source at lines 378–402 uses `File::open`, distinguishes `NotFound` from other errors, and confirms group removal with `symlink_metadata`. The real unit test `denied_cgroup_lookup_never_verifies_teardown` passed. These are synthetic permission tests; no live cgroup was permission-denied.

**P2 safe residual.** `verify_active` validates `ControlGroup` and assigns `observed_group` before later property, PID, executable, socket, controller and mount checks (lines 282–285). If reception or property lookup fails before that assignment, `stop` reads `ControlGroup` once; an empty value returns `no observed cgroup path; teardown unverified` (lines 553–565). A disposable `systemd-run --user --wait --collect` service returned `LoadState=not-found`, `ActiveState=inactive`, and empty `ControlGroup` after stop. Thus collection alone still cannot prove a prior cgroup path. This is an explicit fail-closed residual, not a false success. An identity-bound independent proof would be needed to make this case a verified cleanup.

**P3, synthetic malformed evidence.** The exact `cgroup_stopped` function was extracted from this `service.rs` into a temporary Rust probe, with only `ArchyError` replaced by `String` to compile it independently. Results: `populated 1\nfrozen 0` -> `Ok(false)`; `populated 0\nfrozen 0` -> `Ok(true)`; `populated 0\npopulated 1` -> `Ok(true)`; `populated 0\nnot-an-event` -> `Ok(true)`; over 1024 bytes and invalid UTF-8 -> errors. The `.lines().any(|line| line == "populated 0")` check at lines 389–391 accepts contradictory or extra malformed text. This conflicts with the refreshed handoff's statement that malformed events do not prove teardown. Kernel-generated events in the live probe were well formed; this is a parser contract finding, not evidence of a real kernel producing contradictory events.

## Fixed policy and live teardown evidence

A disposable user service was started with the fixed A09 systemd properties, a private temporary mountpoint, and `/usr/bin/sleep 30` in place of the absent gate. The client used cleared environment plus `LC_ALL=C` and `XDG_RUNTIME_DIR`. While active, `systemctl show` reported `MemoryMax=2147483648`, `MemorySwapMax=0`, `TasksMax=64`, `KillMode=control-group`, `RuntimeMaxUSec=3min`, `TimeoutStopUSec=2s`, `SendSIGKILL=yes`, `LimitCORE=0`, and a concrete `ControlGroup`. Its cgroup files read `memory.max=2147483648`, `memory.swap.max=0`, `cpu.max=100000 100000`, `pids.max=64`, and `cgroup.events=populated 1\nfrozen 0`. `systemctl --user stop` returned 0; the `systemd-run --wait --collect` wrapper exited 0; the unit then reported `not-found`/`inactive`/empty `ControlGroup`, and the saved cgroup path and events file were absent. The disposable mountpoint was removed. This tests this WSL systemd 259 manager and fixed property values, not `ServiceSession::start`, its gate identity checks, or `ServiceSession::stop` with a real gate.

## Validation and limits

With `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-production-supervisor-review-round2`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, and `RUSTUP_AUTO_INSTALL=0`, `cargo test -p archy-adapters --locked --offline` exited 0: 22 library, 6 artifact, 4 isolation and 8 protocol tests passed; the IFC roundtrip and real receiver fixture were ignored. The separately invoked ignored receiver test passed: owned private tmpfs FD accepted, unsafe mode and regular file refused before GO, disposable units inactive/collected with cgroups absent, host unchanged. `cargo fmt -p archy-adapters -- --check` and `cargo clippy -p archy-adapters --all-targets --locked --offline -- -D warnings` both exited 0.

`/usr/lib/archy/archy-service-gate` was absent (`test -e` exit 1). The service controller's pre-token path, cancellation, timeout, TERM resistance, manager failure, OOM, scratch return, and whole-unit teardown therefore remain unexercised end to end. The process tests exercise `process.rs`, not this service controller. No Omarchy target, packaged gate, bwrap policy, worker request binding, or publication path was tested. The absent gate prevents an end-to-end production or containment verdict.
