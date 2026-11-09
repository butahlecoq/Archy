# Independent A09 production service-control review

Date: 2026-10-29. Scope: uncommitted `crates/archy-adapters/src/service.rs` and its `lib.rs` export in `.worktrees/a09-production-supervisor`, with the existing isolation/process APIs and the A09 handoff/plan. **Verdict: changes requested.** I changed no product code, fixtures, commits, or remote state. The only worktree write is this review. Disposable build output stayed under `/home/butahlecoq/work/archy/.tools/targets/a09-production-supervisor-review`; probes used small temporary data and disposable user services.

## Frozen source and intent

The handoff names base commit `a5a61c8e565d50928a7ccbf950ac061691eccf04`; `git rev-parse HEAD` matched. `git status --short` before review showed only modified adapter `lib.rs` and untracked `service.rs` and A09 handoff. The handoff provides **no expected hash for the new `service.rs`**. The observed review snapshot was:

```text
42522214e14349e8aaf2426c7dd7dd5bce3368fd2f39dfcb20a279dc12d678a1  crates/archy-adapters/src/service.rs
955e36333ae1bfb0bf81f2ad36ceff89a69d94d8a636854abd2a2d4f7a941e49  crates/archy-adapters/src/lib.rs
cef81aaa9bc7434aa1d49bd16eaa3604dd956e8d388cb275c868ee321f0d81de  crates/archy-adapters/src/isolation.rs
b1e4aa6c8071f4ed55f3776d937c862baf2e2ce69fb67c223fe32bb3b9179251  crates/archy-adapters/src/process.rs
8fc03ae3778cddb4c75436f7ffacdb4f82830765de2171e38b2268741bcf14f7  crates/archy-adapters/tests/isolation.rs
96823c6529a2f7f044cedc70910b323fc7695a1348be1e171ac653f6634c93d7  crates/archy-adapters/tests/systemd_receiver.rs
5804fc71df012661499d1a5c5c10065d79a089c3e5034db6c1bf2824068ced38  tests/security/systemd_rust_receiver_helper.py
13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40  Cargo.lock
```

The isolation, process, fixture, and lock values match their earlier frozen handoffs. The new service and export hashes identify this review snapshot; they cannot be compared to an A09 handoff hash. The intended boundary is a fixed gated transient user service, with effective identity, cgroup limits, and quota-backed scratch proved before a start token, followed by verified whole-unit teardown before scratch release. The gate is absent, so this review cannot establish an end-to-end controller or containment result.

## Verified findings

### P1: unreadable cgroup can be accepted as removed during teardown

`service.rs:538-539` converts every `cgroup.events` read failure to `None`, then calls `Path::exists()`. Rust's `exists()` returns `false` when lookup is denied. `teardown_verified()` at line 373 accepts `ActiveState=inactive`, `events=None`, `group_exists=false`. Thus a permission or I/O failure can produce the same success result as a removed cgroup. `stop()` then reaps the wrapper, sets `verified_stopped=true`, and `finish()` can release scratch. This violates the promised fail-closed teardown evidence. It does not require a malicious unit; a loss of cgroup visibility is enough.

I reproduced the exact Rust predicate with a disposable mode-000 parent under the separate target. The probe created `unit.service/cgroup.events` containing `populated 1`, removed traversal permission on the parent, and ran the same `read_to_string(...).ok()`/`Path::exists()`/`teardown_verified` expression. Output:

```text
rustc_rc 0
probe_rc 0 read=Err(PermissionDenied) exists=false teardown_verified=true
```

Exact Rust probe body, compiled with `rustc -o /home/butahlecoq/work/archy/.tools/targets/a09-production-supervisor-review/teardown-permission-probe -` and run with its sole argument set to the disposable `unit.service` path after its parent was changed to mode 000:

```rust
use std::path::Path;
fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let p = Path::new(&arg);
    let read = std::fs::read_to_string(p.join("cgroup.events"));
    let observation = read.as_ref().map(|x| x.trim().to_owned()).map_err(|e| e.kind());
    let exists = p.exists();
    let verified = "inactive" == "inactive" && match read.ok() {
        Some(t) => t.lines().any(|x| x == "populated 0"),
        None => !exists,
    };
    println!("read={observation:?} exists={exists} teardown_verified={verified}");
}
```

The setup wrote `populated 1\n` to `unit.service/cgroup.events`; the temporary file, directories, and executable were removed after the probe. Expected: `PermissionDenied` must leave teardown unverified. This is a predicate reproduction, not an observed cgroup permission failure in a real service.

### P2: a collected unit with no saved cgroup path makes cleanup fail despite confirmed inactivity

During `start()`, `session.group` remains empty until **all** pre-token checks finish (`service.rs:459-469`). On a receive, property, PID, socket, or mount failure, `start()` calls `stop()` with an empty group. `stop()` then requires `ControlGroup` to pass `cgroup_path()` (`service.rs:533-535`), even after `--collect` has removed the transient unit. A collected unit reports an empty `ControlGroup`; the call errors before `teardown_verified()` can examine the inactive state. This masks the original failure as uncertain teardown and leaves the private mountpoint directories behind. The same path can occur when the service exits before verification captures its group.

Disposable user-service reproduction (no installed gate or host file write):

```python
import subprocess, uuid
unit = 'archy-a09-review-' + uuid.uuid4().hex + '.service'
run = subprocess.run(['/usr/bin/systemd-run', '--user', '--wait', '--collect', '--quiet', '--unit='+unit, '--', '/usr/bin/true'], capture_output=True, text=True, timeout=15)
show = subprocess.run(['/usr/bin/systemctl', '--user', 'show', unit, '-pLoadState', '-pActiveState', '-pControlGroup'], capture_output=True, text=True, timeout=5)
print(run.returncode, show.returncode, repr(show.stdout))
```

Observed: `0 0 'LoadState=not-found\nActiveState=inactive\nControlGroup=\n'`. `cgroup_path(unit, "")` necessarily returns `unsafe or mismatched service cgroup path`; the service unit's disappearance is the expected `--collect` behavior. The direct service session was not run because `/usr/lib/archy/archy-service-gate` is absent. A safe fix needs an identity-bound way to prove the collected unit and prior cgroup are gone, without treating an unreadable group as absent.

## Checks and limits

| Check | Actual result | Limit |
| --- | --- | --- |
| `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-production-supervisor-review CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 cargo test -p archy-adapters --locked` | Exit 0: 21 library, 6 artifact, 4 isolation, 8 protocol tests passed; IFC and real systemd tests ignored. Process tests exercised bounded output, timeout, cancellation, TERM-resistant child, direct-child reaping, and escaped-session pipe holder. | `process.rs` is fixture-only and is not the service controller. Static service tests do not exercise `start`, `stop`, or `finish` with a gate. |
| Same Cargo environment, `cargo test -p archy-adapters --test systemd_receiver --locked --offline -- --ignored --exact transient_user_service_passes_real_fd_to_public_receiver --nocapture` | Exit 0. Private 65,536-byte/8-inode tmpfs FD accepted; unsafe mode and regular file refused before GO; each disposable unit inactive/collected, cgroup absent, host sentinel unchanged. | This tests the receiver fixture and service FD forwarding, not the A09 gate, service identity checks, or teardown implementation. |
| Fixed `systemd-run --user --pipe --wait --collect --quiet --service-type=exec --expand-environment=no --unit=<UUID> -- /usr/bin/printf '%s\n' '$(printf injected);${HOME};*;--property=MemoryMax=infinity'` with a cleared client environment and fixed `XDG_RUNTIME_DIR` | Exit 0; stdout exactly `$(printf injected);${HOME};*;--property=MemoryMax=infinity\n`. | Confirms this installed systemd 259 treats those argv bytes literally in the disposable probe. The A09 caller exposes only `BackendKind`; its unit/path are generated, command argv is an array, and client environment is cleared. Manager-to-gate environment and gate behavior remain untested. |
| Missing gate probe (`test -e /usr/lib/archy/archy-service-gate`) and existing `missing_packaged_gate_fails_closed_before_service_launch` test | Gate absent; test passed with `BackendUnavailable`. | No direct A09 service, worker, bwrap, request binding, cancellation/timeout, OOM, or whole-cgroup teardown can be claimed. |

Source review found no writable host-directory fallback in `ServiceSession`: the mountpoint is created empty and a received descriptor must pass owned mode-0700 tmpfs byte/inode/flag checks, then match the service mount identity before GO. `release()` sends only after these checks; `finish()` calls `stop()` before returning the retained descriptor. `systemctl` output is capped at 8,192 bytes per command and manager commands use deadlines. These are code-path observations, not production proofs. The manager can inherit an environment distinct from the cleared `systemd-run` client environment; the absent gate must enforce its own environment and bwrap policy. The service API currently carries neither typed request bytes nor audited runtime digest/run identity, as the handoff says.

The user manager reported `running`; `systemctl --version` reported systemd 259 (259.5-0ubuntu3.4). This WSL host had about 520 MiB free on `/tmp` during review. No Omarchy target, packaged gate, worker, namespace denial matrix, disk/inode exhaustion at the A09 128 MiB/256-inode limits, or end-to-end publication was tested. Neither finding changes the stated absence of controller/containment and Omarchy acceptance evidence.

## Response snapshot pending independent rereview

The implementation response is documented in `a09-production-supervisor-handoff.md`. The current `service.rs` SHA-256 is `416ba8d7d3f20bacb4522e29c5a7ec452bf0ec28ec13046464e8af90e6261d4a`; `lib.rs` remains `955e36333ae1bfb0bf81f2ad36ceff89a69d94d8a636854abd2a2d4f7a941e49`. This section records the implementer's response and does **not** replace the independent review verdict. A separate reviewer must inspect the revised snapshot before that verdict can be closed.
