# Independent A09 supervisor review, round 3

Date: 2026-10-29. Worktree: `.worktrees/a09-production-supervisor`. Scope: frozen A09 adapter candidate, handoff, and both prior reports. I did not change source, other worktrees, installed files, commits, or remote state. Probe artifacts and Cargo output used the existing `.tools/targets/a09-production-supervisor` or the fresh SSD target `.tools/targets/a09-production-supervisor-review-round3`; no Cargo target was under `/tmp`.

## Verdict

**Round 1 P1 false success and round 2 P3 malformed-event false success are closed on this snapshot.** The P2 no-observed-cgroup case remains explicitly unverified and fails closed. I found no new false teardown success in the probed cases. This is a code-level rereview, **not production or Omarchy acceptance**: the packaged gate is absent, so `ServiceSession::start` through release/finish, gate identity, worker execution, cancellation, OOM, and whole-job teardown were not exercised. Keep the A09 production gate open until those paths run on the target with an audited gate and an identity-bound cleanup proof for the no-observed-group case.

## Frozen identity

`git status --short` before review showed modified `lib.rs` and untracked `service.rs`, handoff, and prior reports. SHA-256 at review start and after checks:

```text
663059a07782b0b8e82e3ecee43464fc43ffb25b1f9d92689442c26bf3f9518e  crates/archy-adapters/src/service.rs
955e36333ae1bfb0bf81f2ad36ceff89a69d94d8a636854abd2a2d4f7a941e49  crates/archy-adapters/src/lib.rs
cef81aaa9bc7434aa1d49bd16eaa3604dd956e8d388cb275c868ee321f0d81de  crates/archy-adapters/src/isolation.rs
b1e4aa6c8071f4ed55f3776d937c862baf2e2ce69fb67c223fe32bb3b9179251  crates/archy-adapters/src/process.rs
8fc03ae3778cddb4c75436f7ffacdb4f82830765de2171e38b2268741bcf14f7  crates/archy-adapters/tests/isolation.rs
96823c6529a2f7f044cedc70910b323fc7695a1348be1e171ac653f6634c93d7  crates/archy-adapters/tests/systemd_receiver.rs
5804fc71df012661499d1a5c5c10065d79a089c3e5034db6c1bf2824068ced38  tests/security/systemd_rust_receiver_helper.py
13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40  Cargo.lock
```

All match the round 3 freeze in the handoff. The `service.rs` hash differs from round 2 as expected; supporting frozen hashes match both prior reports.

## Independent reproductions

I extracted the **unchanged** `cgroup_populated` and `cgroup_stopped` function bodies from this `service.rs` into `.tools/targets/a09-production-supervisor-review-round3/probe.rs`; only the error type and constructor were substituted (`ArchyError = String`, `unavailable(s) = s`). `rustc --edition=2024 .../probe.rs -o .../probe` and `.../probe .../a09-production-supervisor-review-round3` both exited 0. The probe directory contains the exact runnable cases. This tests the real decision bodies with synthetic files, not a kernel that emitted malformed events.

| Input or state | Result from current functions |
| --- | --- |
| `populated 1\nfrozen 0\n` | `Ok(false)` |
| `populated 0\nfrozen 0\n` | `Ok(true)` |
| `future-field 3\npopulated 0\nfrozen 0\n` | `Ok(true)`; an additional well-formed flat key is allowed |
| `populated 0\npopulated 1\n` | error (duplicate), closing round 2 P3 |
| duplicate unknown key; malformed extra line; extra value token; missing `populated` | errors |
| missing newline; empty file; blank extra line; tab separator | errors |
| invalid `frozen` or `populated` state; invalid UTF-8; over 1024 bytes | errors |
| group exists, `cgroup.events` missing | error |
| group removed, event open and group lookup both `NotFound` | `Ok(true)` |
| parent mode 000, event file still `populated 1\n` | `File::open` = `PermissionDenied`, `Path::exists()` = false, **old predicate = true**, current `cgroup_stopped` = error |

The last case reproduces the round 1 false-success conditions and confirms the revised code propagates denial. `cgroup_stopped` treats only event-open `NotFound` plus group-lookup `NotFound` as removal. An unreadable group and an existing group with a missing event file do not pass. The parser validates every newline-terminated record, unique graphic key, single graphic value, and known `0`/`1` states. Both `verify_active` and `cgroup_stopped` call this parser. A well-formed unknown field may appear before or after `populated`; the probe covered before, and the adapter unit test covers after.

The two-read removal check has an unavoidable filesystem race: the group can disappear between event open and group lookup. The observed decision remains fail closed unless both lookups return `NotFound`. A live manager-state race was not fault-injected. `stop` also requires `ActiveState=inactive` and waits for the `systemd-run` wrapper after the cgroup check; it does not use an empty `ControlGroup` as success. If no group path was observed before the unit is collected, it returns `no observed cgroup path; teardown unverified`. A disposable service below reproduced the collected state (`not-found`, `inactive`, empty `ControlGroup`); it does not establish a safe cleanup proof for that branch.

## Adapter and disposable WSL checks

Environment for both Cargo invocations: `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-production-supervisor CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0`.

- `cargo test -p archy-adapters --locked --offline`: exit 0; 23 library, 6 artifact, 4 isolation, and 8 protocol tests passed; the IFC roundtrip and real receiver fixture were ignored in this invocation. This includes the denied lookup, malformed events, service socket endpoint, cgroup path, gate absence, and effective property tests.
- `cargo test -p archy-adapters --test systemd_receiver --locked --offline -- --ignored --exact transient_user_service_passes_real_fd_to_public_receiver --nocapture`: exit 0. The real disposable user service forwarded a private tmpfs FD accepted by the Rust receiver; unsafe mode and regular-file descriptors were refused before GO. All three units became inactive/collected, their cgroups were absent, and the fixture reported the host sentinel unchanged. This fixture is separate from `ServiceSession`.
- A fresh disposable `systemd-run --user --pipe --wait --collect --service-type=exec` service under an SSD probe mountpoint used the A09 unit properties with `/usr/bin/sleep 30` as its executable. While active, `systemctl show` returned `LoadState=loaded`, `ActiveState=active`, `MemoryMax=2147483648`, `MemorySwapMax=0`, `TasksMax=64`, `KillMode=control-group`, `RuntimeMaxUSec=3min`, `TimeoutStopUSec=2s`, `SendSIGKILL=yes`, `LimitCORE=0`, and a concrete group path. Its `memory.max`, `memory.swap.max`, `cpu.max`, `pids.max`, and `cgroup.events` read `2147483648`, `0`, `100000 100000`, `64`, and `populated 1\nfrozen 0\n`. `systemctl --user stop` and wrapper wait exited 0; afterward `show` returned `LoadState=not-found`, `ActiveState=inactive`, empty `ControlGroup`, and the saved group path was absent. The disposable mountpoint was removed.
- `test -e /usr/lib/archy/archy-service-gate`: exit 1. The existing unavailable-gate unit test passed. No substituted sleep or receiver fixture proves gate executable identity, control socket endpoint inside `ServiceSession`, or its `finish()` behavior.

## Source review and next gate

The public adapter export and fixed `BackendKind` API expose no caller-supplied executable or unit. `launch_argv` fixes the systemd options and gate path; `verify_properties` rejects mismatched identity/limit properties, `verify_active` checks MainPID cgroup membership, gate inode, **service** socket inode, cgroup limits and populated events before scratch mount identity/emptiness and GO. The supporting isolation, process, receiver, and lock hashes did not change. These are source observations and focused unit checks; the live service used `/usr/bin/sleep` because the gate is absent.

Before production approval, install and audit the packaged gate on the actual Omarchy target; run the complete pre-token path with typed request/runtime identity, and force cancellation, timeout, TERM-resistant descendants, manager disconnection, OOM, and teardown after group loss. Preserve an identity-bound proof for units collected before any cgroup path can be observed, or retain the current explicit unverified failure. Verify that scratch is returned and published only after whole-job teardown. The present tests cannot close those acceptance conditions.
