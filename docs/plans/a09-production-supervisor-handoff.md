# A09 production service control slice handoff

Date: 2026-10-29. Worktree: `.worktrees/a09-production-supervisor`; branch `codex/a09-production-supervisor`; base `a5a61c8e565d50928a7ccbf950ac061691eccf04` (`origin/feat/release-0.1` at creation). This is an uncommitted source slice awaiting independent Sol review. No install, commit, push, CLI, storage, worker, dependency, or other worktree changes.

## Short plan and scope

1. Reuse the reviewed scratch receiver and fixed user-service protocol. Add an adapter-owned, typed service session with no caller-provided executable, argv, unit, mount path, or policy.
2. Require trusted fixed systemd tools and a root-owned packaged gate; create a UUID-scoped private mountpoint and transient user service. Fail closed if the gate is absent.
3. Before sending the start token, verify effective unit properties, MainPID, the **service endpoint** of the control socket, executable inode, cgroup membership and effective memory/swap/CPU/tasks limits, and the received scratch mount identity and emptiness.
4. Bound manager commands, direct-child waiting, cancellation, stop/escalation, and teardown observation. Return the pinned scratch only after inactive unit, empty/removed cgroup and reaped wrapper are observed.

The fixed gate path is `/usr/lib/archy/archy-service-gate`; the gate is **not installed or implemented in this slice**. The session is a production-buildable control layer, but cannot run a real worker on this host. The backend enum only selects `blender` or `ifc`; a later packaged gate must enforce fixed worker argv, bwrap policy, immutable runtime inventory, typed request/input handling, and control-channel EOF behavior. The current service path gives no publication authority. No claim of complete containment follows from the unit tests.

## Review focus

- Confirm systemd's actual `RuntimeMaxUSec`, `LimitCORE`, `SendSIGKILL` and FD forwarding behavior on target Omarchy; the code refuses mismatches.
- Exercise a packaged gate and real transient service with the entire pre-token check, then force stop, TERM-resistant and escaped-session descendants, timeout, cancellation, manager disconnection, OOM and cgroup teardown. Static parser tests do not prove these paths.
- Confirm the user manager's `TemporaryFileSystem` and cgroup controller enforcement on target; missing capabilities must remain `BACKEND_UNAVAILABLE`.
- Bind a validated typed request and audited runtime digest to the gate. The current service session does not carry request bytes or run/revision identity.
- Check scratch capture and semantic readers only after whole-job writer teardown; storage publication stays caller-owned.

## Validation and residual gaps

All Cargo commands use `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-production-supervisor CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0`. Final `cargo fmt -p archy-adapters -- --check`, `cargo test -p archy-adapters --locked`, and `cargo clippy -p archy-adapters --all-targets --locked -- -D warnings` each exited 0. The test suite reported 21 library tests, 6 artifact tests, 4 isolation tests, and 8 protocol tests passed; one pinned IFC roundtrip and one real systemd receiver fixture were ignored; doc tests had zero cases. New unit tests cover fixed argv/policy, unavailable gate, property mismatches, cgroup path rejection, endpoint distinction, and teardown predicates. The existing receiver, artifact, process, and protocol tests also ran. The gate was absent at `/usr/lib/archy/archy-service-gate`; `/run/user/1000` was an owned mode-0700 directory. These are WSL development observations, not Omarchy acceptance.

The service gate, bwrap launch, all writable-mount policy, live service/cgroup teardown, backend request/response binding, output verification, and whole-job publication are unimplemented. Actual Omarchy acceptance is deferred. A process that can modify another process or unit under the same user is outside this slice's authentication boundary; independently audited gate installation and target tests are required before production use.

## Independent review response, 2026-10-29

The original review snapshot and findings are preserved in `a09-production-supervisor-review.md`. The revised source hash is `416ba8d7d3f20bacb4522e29c5a7ec452bf0ec28ec13046464e8af90e6261d4a` for `crates/archy-adapters/src/service.rs`; the `lib.rs` hash remains `955e36333ae1bfb0bf81f2ad36ceff89a69d94d8a636854abd2a2d4f7a941e49`. The frozen isolation, process, receiver fixture and lock hashes in that review remain unchanged.

P1: The pre-fix permission test failed as expected: an inaccessible parent produced `PermissionDenied`, `Path::exists()` returned false, and teardown was falsely verified. The stop loop now reads `cgroup.events` without discarding syscall errors. Only a `NotFound` opening `cgroup.events` followed by `NotFound` looking up the group proves removal. An existing group with missing events, unreadable path, malformed/oversize events, or `populated 1` does not prove teardown. A test exercises real permission denial; another exercises populated/empty events and removal between observations.

P2: A fresh disposable `systemd-run --user --wait --collect` service returned `LoadState=not-found`, `ActiveState=inactive`, and empty `ControlGroup` after completion. These properties do not identify a prior cgroup. The controller now retains a validated cgroup path as soon as it observes one, before later property, process, socket and mount checks. If reception or property lookup fails before any path is observed and the collected unit has an empty `ControlGroup`, `stop()` explicitly returns `no observed cgroup path; teardown unverified`, retains scratch, and does not claim cleanup. A safe success path for that remaining case requires an independent identity-bound cgroup or manager proof; none was established here.

Validation on this WSL user manager: offline adapter suite passed (22 library, 6 artifact, 4 isolation, 8 protocol; 2 ignored); strict Clippy passed; real ignored receiver fixture passed with disposable user services. A09 service end-to-end is still blocked by the absent packaged gate. A separate reviewer has not yet rereviewed this revised snapshot; the previous verdict remains open.

## Round-two P3 response and exact freeze, 2026-10-29

The independent direct rereview is preserved in `a09-production-supervisor-review-round2.md`. Its P1 and P2 findings remain as described above. P3 identified a parser contract failure: synthetic `cgroup.events` bytes with `populated 0` followed by a contradictory or malformed record were accepted as teardown evidence. The live disposable service produced well-formed `populated 1\nfrozen 0\n`; no malformed kernel output was observed. The [kernel cgroup v2 documentation](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html) defines `cgroup.events` as a read-only flat-keyed file with `populated` and `frozen` states. The parser now validates every newline-delimited key/value record, rejects duplicate keys and invalid known states, requires exactly one `populated` record, and permits additional well-formed fields. Both active and stopped checks use it. The prior `PermissionDenied` and unknown-cgroup fail-closed behavior remains in place; no fallback path was added.

The focused `malformed_cgroup_events_cannot_prove_teardown` test failed before the fix on `populated 0\npopulated 1\n`, then passed after the fix. It also checks malformed extra records, missing `populated`, invalid `frozen`, duplicate additional keys, an unterminated record, and an accepted additional field. With `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-production-supervisor`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, and `RUSTUP_AUTO_INSTALL=0`, `cargo test -p archy-adapters --locked --offline` exited 0 (23 library, 6 artifact, 4 isolation, 8 protocol passed; 2 ignored). The separately invoked ignored `systemd_receiver` test exited 0 against a real disposable user service. `cargo fmt -p archy-adapters -- --check`, `cargo clippy -p archy-adapters --all-targets --locked --offline -- -D warnings`, and `git diff --check` exited 0. The real receiver test exercises the shared user-service environment and FD transfer, not `ServiceSession` with the absent packaged gate.

Frozen SHA-256 values for this review candidate:

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

Independent Sol review remains pending. This session exposes no Sol delegation interface, and the task explicitly excludes child agents; the coordinating implementation cannot serve as its own independent review. No commit or push was made. The packaged gate and Omarchy end-to-end acceptance gaps described above remain.
