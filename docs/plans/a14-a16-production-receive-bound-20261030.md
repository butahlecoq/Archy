# A14/A16 production receive-bound candidate

30 October 2026. Isolated worktree:
`.worktrees/a14-a16-bus-confirm-20261030-unique`. This remains an unstaged,
fail-closed review candidate. `Confirm` and both seqpacket decision routes deny.
No system-wide daemon policy is changed.

## Bound and source

The exact crates.io zbus 5.19.0 archive is SHA-256
`5db4be7c075cb421e4b7ee645541604239bd243ba7c357511f4ff3a74b555907`,
the prior lockfile checksum. Its MIT `LICENSE` is retained unchanged at SHA-256
`260e4ff3fc493695880c90774283eb1e3d018f898fabe655bbd52dd2cbd2fcad`.
`[patch.crates-io]` selects the local copy and the new lockfile resolves zbus
to that path. `authority/vendor/zbus-5.19.0/ARCHY-PATCH.md` identifies the
seven upstream files changed and the source comparison command. The vendored
source diff is frozen as `a14-a16-zbus-receive-bound.patch`. The source files
and patch are frozen in `a14-a16-production-receive-bound-source.sha256` alongside
the service, tests, and prior candidate files.

The local `Builder::max_message_size` defaults to zbus's original 128 MiB and
is clamped to it. The setting is per connection. The production human service
sets 65,536 bytes before authentication, while other connections keep their
default. The value is passed to the bus Hello reply and the normal socket
reader. Both paths calculate complete frame length from the 16-byte primary
header with checked addition and reject excess **before** the body buffer
`resize` and body read. The service exits with an error when the connection
closes, so a future supervisor can restart it; no supervisor is installed by
this candidate. A caller can force that exit with one oversize message, which
is an availability limit requiring policy and restart testing on target.

The [upstream zbus builder](https://docs.rs/zbus/5.19.0/zbus/connection/struct.Builder.html)
has `max_queued` but no receive-byte setter. The
[upstream read path](https://docs.rs/zbus/5.19.0/src/zbus/connection/socket/mod.rs.html)
uses the primary header length before resizing; its upstream ceiling is
128 MiB. The [D-Bus daemon limits](https://dbus.freedesktop.org/doc/dbus-daemon.1.html)
are broker configuration, so the private-bus configuration alone does not
protect the production system-bus service.

## Reproducible checks

Run from this worktree on ext4:

```sh
findmnt -T /home/butahlecoq/work/archy -no TARGET,FSTYPE
sha256sum -c docs/plans/a14-a16-production-receive-bound-source.sha256
TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/target cargo +1.99.1 test --locked --offline -p archy-authority --test zbus_receive_bound --test human_bus_boundary -- --nocapture
TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/target cargo +1.99.1 check --locked --offline -p archy-authority --bin archy-human-busd
cargo +1.99.1 fmt --all -- --check
git diff --check
```

The header-only adversarial test feeds an advertised 70 KiB body, a maximum
header-fields length, and invalid endian. It asserts exactly one 16-byte read
and no body read. The real disposable private bus accepts 2 MiB frames while
the service connection accepts 64 KiB. A 60 KiB call reaches the handler and
is denied there; a 70 KiB call yields a transport error, closes the service
connection, and adds no mock logind lookup. Earlier valid calls return the
exact protected snapshot and digest. The same test retains bus-assigned
unique-name, ProcessFD, UID, active/local/seated mock-session, and denied
`Confirm` checks.

Final focused run on ext4 with the command above: **2 tests passed, 0 failed**
(`human_bus_boundary` and `zbus_receive_bound`). The locked offline service
binary check exited 0. Formatting and `git diff --check` exited 0.

## Remaining review and deployment gates

- The snapshot worker uses a thread that cannot be cancelled. Four stuck
  workers retain all four slots indefinitely. The three-second response
  deadline and four-slot concurrency bound are not execution-time limits.
  A separate process worker with strict IPC, kill/reap semantics, privilege
  handling, and protected-store isolation would need its own design and tests.
- The private bus uses mock logind. Native distinct-UID session and policy,
  bus restart, privileged bind-mount replacement, target memory pressure,
  and service restart behavior are unproven.
- The human bus binary remains absent from package staging and installed
  units. Its separate normal dependency closure and notices, including zbus,
  need inventory and hashing before distribution. Independent review and a
  disposable target recheck are required before activation. `Confirm` and
  seqpacket decisions remain disabled.
