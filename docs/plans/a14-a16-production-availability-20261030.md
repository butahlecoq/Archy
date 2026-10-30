# A14/A16 production availability follow-on candidate

30 October 2026. Worktree: `.worktrees/a14-a16-bus-confirm-20261030-unique`.
This is an unstaged review candidate. It retains the per-service 65,536-byte
zbus preallocation limit from the prior receive-bound candidate. It does not
install policy or units, stage a package, enable `Confirm`, or enable either
seqpacket decision route.

## Design and boundaries

- A closed system-bus connection consumes one incident from a four-connection
  process budget. Attempts are separated by 1, 2, and 4 seconds. Startup
  connection errors consume the same budget. After exhaustion the process exits
  nonzero. There is no persistent privileged reconnect loop. Each attempt
  reconstructs the service interface and its in-flight capacity. Caller
  identity and active local session are queried afresh for each review;
  no authenticated caller or approval lease is retained. The service now opens
  the actual `project/project.json` file, rather than the project directory.
- The synchronous protected snapshot runs in a child of the same broker UID,
  reached through `/proc/self/exe` in production. The parent passes the
  proposal digest, broker state path, cached source digest, and cached
  dev/inode/mount identity digest for the full protected path. The child
  reopens the broker-owned store and protected project, checks those identities,
  then uses the existing locked snapshot recomputation and final path/head
  checks. Its JSON reply is bounded to 1 MiB plus 4 KiB and checked against
  its digest. The parent kills and reaps the child at the three-second review
  deadline. It aborts if SIGKILL cannot be reaped within one second. A separate
  watchdog aborts if the parent orchestration thread itself stays stuck past
  the deadline plus one second. The review slot is released only after that
  thread ends. The child retains broker UID; native process sandboxing and
  these fail-stop cases need service supervision review.
- The vendored zbus Unix transport now reserves ancillary space for 16 rather
  than 1,024 FDs, rejects `MSG_CTRUNC`, explicitly rejects more than 16 received
  FDs even if control-message padding permits them, and caps their cumulative
  count over a complete frame. This changes the vendored crate globally;
  only the authority package uses zbus in this workspace. The authority's
  credentials reply needs one ProcessFD. Legitimate target-bus compatibility
  remains unproven.

## Checks and limits

From this worktree on ext4:

```sh
TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/a14-a16-availability-target-20261030-unique cargo +1.99.1 test --locked --offline -p archy-authority --test human_bus_boundary --test zbus_receive_bound --test bus_recovery --lib --bin archy-human-busd -- --nocapture
TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/a14-a16-availability-target-20261030-unique cargo +1.99.1 test --locked --offline -p archy-authority --test protected_project -- --nocapture
TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/a14-a16-availability-target-20261030-unique cargo +1.99.1 check --locked --offline -p archy-authority --bin archy-human-busd
cargo +1.99.1 fmt --all -- --check
git diff --check
sha256sum -c docs/plans/a14-a16-production-availability-source.sha256
```

The focused test run exited 0: 6 library, 1 binary, 1 bus restart,
1 human boundary, and 4 receive-bound tests passed. All 9 protected-project
integration tests also passed. The vendor socket test
exited 0 after the explicit count fix. The locked offline binary check,
formatting check, and `git diff --check` also exited 0.

- Focused authority tests on ext4 with short `TMPDIR` and a dedicated ext4
  target cover byte frames at exactly 65,536 and 65,537 bytes, malformed
  dynamic fields, cumulative 17-FD rejection, a sleeping child killed/reaped
  at deadline, held protected head lock denial, exact valid snapshot replay,
  oversized-frame connection closure followed by a new interface and fresh
  session check, finite retry delays, real private-bus daemon restart at the
  same socket with a new GUID for a probe service name, and denied `Confirm`.
- The vendored `fd_recvmsg` unit test sends real `SCM_RIGHTS` through a Unix
  socket: 16 FDs pass and 17 fail. It is run from an isolated ext4 copy because
  the patched crate is not a workspace member; that copy adds an empty
  `[workspace]` table and removes the unavailable `codspeed-criterion-compat`
  development dependency. The test
  initially exposed the control-message padding issue, then passed with the
  explicit count check. The final command was
  `TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/zbus-ancillary-target-20261030 cargo +1.99.1 test --offline --manifest-path /home/butahlecoq/work/archy/.tools/zbus-ancillary-test-20261030/Cargo.toml --lib ancillary_bound_tests -- --nocapture`.
  No production dependency was changed for this check.
- The private-bus fixture has one UID and mock logind. It does not prove real
  distinct-UID sender policy, actual system-bus restart behavior in the
  packaged service, native systemd supervision, kernel-stuck I/O recovery,
  malformed-parser CPU worst cases, target memory/FD pressure, installed
  notices, or package compatibility.
  These remain security and deployment blockers. A finite in-process budget
  deliberately stops serving after four incidents; operational recovery after
  exhaustion also needs a reviewed native policy.

The changed files and focused tests are frozen in
`docs/plans/a14-a16-production-availability-source.sha256` for separate
security review. The earlier receive-bound manifest remains historical and
is superseded for files changed here. No production PASS or `Confirm`
authorization follows from this candidate.
