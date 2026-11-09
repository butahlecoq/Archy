# A14/A16 zbus SCM_RIGHTS close-on-exec correction

30 October 2026. Scope: `.worktrees/a14-a16-bus-confirm-20261030-unique`.
This is an unstaged security-review candidate. No deployment, approval path,
push, child agent, or other worktree edit was made.

## Cause and correction

The independent production availability review found that vendored zbus called
`recvmsg` with `RecvFlags::empty()`. A real Unix socket regression failed on
that source: a received `/dev/null` descriptor was visible in an exec child.
The receiving process's `F_GETFD` also lacked `FD_CLOEXEC`. The authority
seqpacket receive path already used `MSG_CMSG_CLOEXEC`; the zbus Unix path did
not.

Vendored zbus now requests `RecvFlags::CMSG_CLOEXEC` at every Unix ancillary
receive on Linux, Android, FreeBSD, NetBSD, OpenBSD, and DragonFly. This sets
close-on-exec while the kernel installs SCM_RIGHTS and avoids a gap in which a
concurrent spawn could inherit them. A kernel that rejects the flag returns an
error without a non-atomic retry. On Unix targets without this atomic receive
flag, `fd_recvmsg` returns `Unsupported` before calling `recvmsg`; the service
cannot safely accept rights there. This is a fail-closed portability limit.
Windows uses its existing non-Unix receive path. The 16-descriptor per-receive
and cumulative frame limits, `MSG_CTRUNC` rejection, and 64 KiB service frame
limit remain in place.

## Evidence

The new vendored regression was first compiled against the vulnerable source
in an isolated copy. It failed with `received FD crossed exec`, showing
`/dev/null` in the child. After the correction, three vendored real-socket
tests passed: single and repeated SCM_RIGHTS receives; 1, 2, and 16 rights
accepted; 17 and 32 rejected; and no matching sent memfd left open on either
rejection. The 32-right case exercises control truncation. The named memfd
assertion avoids process-wide FD-count races with parallel tests.

The private-bus boundary test now checks the actual bus `ProcessFD` for
`FD_CLOEXEC` and probes it across a real `/bin/sh` exec. That test also runs
the actual snapshot worker through the served `Review` method. The shell
probe establishes the inherited FD table at exec; it does not inspect the
worker process while it is running.

Commands run from this worktree, all exit 0 after the fix:

```sh
TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/zbus-ancillary-target-20261030 CARGO_BUILD_JOBS=2 cargo +1.99.1 test --offline --manifest-path /home/butahlecoq/work/archy/.tools/zbus-cloexec-test-20261030/Cargo.toml --lib ancillary_bound_tests -- --nocapture
TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/a14-a16-availability-target-20261030-unique CARGO_BUILD_JOBS=2 cargo +1.99.1 test --locked --offline -p archy-authority --test human_bus_boundary --test zbus_receive_bound --test bus_recovery --test protected_project --lib --bin archy-human-busd -- --nocapture
TMPDIR=/home/butahlecoq/work/archy CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/a14-a16-availability-target-20261030-unique CARGO_BUILD_JOBS=2 cargo +1.99.1 check --locked --offline -p archy-authority --bin archy-human-busd
cargo +1.99.1 fmt --all -- --check
rustfmt +1.99.1 --edition 2024 --check authority/vendor/zbus-5.19.0/src/connection/socket/unix.rs
git diff --check
sha256sum -c docs/plans/a14-a16-bus-cloexec-source.sha256
```

The vendored test copy differs only in Cargo test scaffolding: it adds an
empty `[workspace]` and omits the unavailable `codspeed-criterion-compat`
development dependency. Its tested `src/connection/socket/unix.rs` was copied
byte-for-byte from this candidate before the final vendor run. No production
dependency was changed for this test.

The new source freeze is `a14-a16-bus-cloexec-source.sha256` for the vendor
receive source, patch note, and private-bus test. Older freeze manifests remain
historical inputs and are superseded for these files. Independent review
should verify the atomic flag on each supported receive path, fail-closed
unsupported-platform behavior, error cleanup, the ProcessFD boundary, and
that 64 KiB/16 FD limits still hold. Native installed policy, distinct UID
behavior, production supervision, and approval authorization remain outside
this correction and unapproved.
