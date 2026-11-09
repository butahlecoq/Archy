# A09 controller integration handoff

Date: 2026-10-29. New isolated worktree: `.worktrees/a09-controller-integration-20261029`, detached at published `origin/feat/release-0.1` commit `d356133b7311bb05104837be6a356ee3574fe581`. No commit or push.

## Plan and integration

1. Verify the A09 handoff and independent round 3 review freeze against the original source worktree.
2. Carry only the reviewed `service.rs` and its Linux module/export in `lib.rs` onto the published base. Preserve published receiver, process, isolation, lockfile, and all other changes.
3. Run focused adapter tests, the disposable WSL receiver, formatting, strict Clippy, and patch checks. Keep the production gate open.

The source files were copied byte for byte from the reviewed A09 snapshot. The five published commits after the A09 source base changed no adapter files or `Cargo.lock`. The supporting receiver, process, isolation, test fixture, and lockfile SHA-256 values on `d356133` equal the round 3 freeze. The adapter package compiles with the latest published core and eval crates. The patch contains only `crates/archy-adapters/src/lib.rs` and new `crates/archy-adapters/src/service.rs`; it adds no dependencies or caller-supplied executable/policy authority.

## Reproducible source patch

`docs/plans/a09-controller-integration.patch` is a plain Git patch based on `d356133`. From a clean checkout at that commit:

```sh
git apply --check docs/plans/a09-controller-integration.patch
git apply docs/plans/a09-controller-integration.patch
```

The patch itself is an untracked handoff artifact in this worktree, so copy it to that clean checkout first. It has no documentation changes or changes from other worktrees. `git apply --check --cached` passed against the untouched `d356133` index; `git apply --check --reverse` passed against the integrated worktree.

## Frozen source identity

```text
663059a07782b0b8e82e3ecee43464fc43ffb25b1f9d92689442c26bf3f9518e  crates/archy-adapters/src/service.rs
955e36333ae1bfb0bf81f2ad36ceff89a69d94d8a636854abd2a2d4f7a941e49  crates/archy-adapters/src/lib.rs
cef81aaa9bc7434aa1d49bd16eaa3604dd956e8d388cb275c868ee321f0d81de  crates/archy-adapters/src/isolation.rs
b1e4aa6c8071f4ed55f3776d937c862baf2e2ce69fb67c223fe32bb3b9179251  crates/archy-adapters/src/process.rs
8fc03ae3778cddb4c75436f7ffacdb4f82830765de2171e38b2268741bcf14f7  crates/archy-adapters/tests/isolation.rs
96823c6529a2f7f044cedc70910b323fc7695a1348be1e171ac653f6634c93d7  crates/archy-adapters/tests/systemd_receiver.rs
5804fc71df012661499d1a5c5c10065d79a089c3e5034db6c1bf2824068ced38  tests/security/systemd_rust_receiver_helper.py
13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40  Cargo.lock
b12dcc8c0f34a0923969beedfe2e2231dd5bc22ab1d997151ba243a3b8d60d71  docs/plans/a09-controller-integration.patch
```

## Validation

All Cargo commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-controller-integration-20261029`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, and `RUSTUP_AUTO_INSTALL=0`.

- `cargo test -p archy-adapters --locked --offline`: exit 0; 23 library, 6 artifact, 4 isolation, and 8 protocol tests passed; pinned IFC and live receiver fixtures ignored in this invocation.
- `cargo test -p archy-adapters --test systemd_receiver --locked --offline -- --ignored --exact transient_user_service_passes_real_fd_to_public_receiver --nocapture`: exit 0. A disposable WSL user service accepted the private tmpfs FD, refused unsafe mode and regular file FDs before GO, and reported inactive/collected units, absent cgroups, and unchanged host sentinel.
- `cargo fmt -p archy-adapters -- --check`: exit 0.
- `cargo clippy -p archy-adapters --all-targets --locked --offline -- -D warnings`: exit 0.
- `git diff --check`, patch forward-index check, patch reverse-worktree check: exit 0.
- `/usr/lib/archy/archy-service-gate` is absent on this host.

## Review status and open production gate

The independent round 3 review applies to the exact frozen source above. It closed the permission-denied and malformed-event false teardown successes. The no-observed-cgroup path remains explicitly unverified and fails closed. This integration has a compatibility assessment, not a new independent source review.

**A09 remains open.** The packaged fixed gate is absent. `ServiceSession::release` sends `GO` and then drops its control socket; the gate's required EOF behavior and a typed request/runtime binding are not established here. The live disposable receiver fixture exercises FD transfer, not `ServiceSession` worker execution. No end-to-end worker run, gate identity, cancellation/OOM/manager-loss teardown, output capture, publication, or Omarchy acceptance is claimed. The separate fixed gate author owns gate work; this integration does not edit it. Before A09 closure, audit that gate and test the full service path on the target, including GO/EOF semantics and identity-bound cleanup when a collected unit exposes no cgroup path.
