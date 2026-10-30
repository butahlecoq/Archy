# A14/A16 human bus oversized-frame availability follow-on

30 October 2026. Source base: `7af8be00ae5872053cb60bf98c7191a089cc5014`.
This work was done in a new isolated worktree; the base integration worktree and
the author checkout were not changed.

## Plan and basis

The private-bus review showed that a 70 KiB enrolled-sender method call closes
the shared 64 KiB-limited zbus service connection. Repeated calls can spend its
finite retry and supervisor start budgets. Preserve the accepted-frame limit,
but consume a *complete* oversized message before reading the next one. The
[D-Bus message format](https://dbus.freedesktop.org/doc/dbus-specification.html)
defines its total length from the 16-byte primary header and caps a message at
128 MiB. The [dbus-daemon limits](https://dbus.freedesktop.org/doc/dbus-daemon.1.html)
include per-message bytes and Unix FDs; the effective installed values remain
a native policy gate.

The vendored zbus receive path now skips an oversized frame using a 4096-byte
stack buffer without constructing a `Message` or dispatching it. Its absolute
work bound is the D-Bus 128 MiB protocol ceiling. The default `async-io` build
gives a partial header or frame two seconds to complete; the idle wait for a
new header remains open. The loop yields every 16 chunks and waits five
milliseconds after each discarded frame. Received SCM_RIGHTS values from a
discarded frame are closed; accepted messages still use the existing 16-FD
per-message cap and `MSG_CMSG_CLOEXEC` handling. Impossible lengths, protocol
version mismatch, EOF, receive timeout and truncated ancillary data remain
fatal because the next frame position cannot be trusted. No new dependency was
added. The vendored zbus MIT license remains in place.

## Behavioral evidence

- Before the receive change, `oversized_frame_with_fd_is_discarded_before_next_message`
  failed with `ExcessData`; after it, the test reads the following valid message,
  observes reads no larger than 4096 bytes, and observes the discarded FD closed.
- Seven receive-bound tests pass. They cover the exact 64 KiB allocation bound,
  impossible or malformed primary headers, malformed fields, the accepted-frame
  16-FD cap, a partial primary header deadline, and a partial oversized-frame
  deadline. The low-level partial-frame tests use a synthetic read half.
- The real disposable private D-Bus test passes. On one actual served service
  connection it sends three sequential 70 KiB calls, a 70 KiB call with
  SCM_RIGHTS, and a 20-call 70 KiB burst, then obtains valid Review replies.
  It checks that the service unique name and connection survive. A raw sender
  completes D-Bus Hello, holds a partial 70 KiB Review for 2.5 seconds, then
  completes it; valid Reviews succeed during and after the stall. The private
  bus disconnects a raw malformed post-Hello sender while valid Review still
  works. This test also retains mock session denial and Confirm denial. Logind
  and the installed policy are not exercised.
- `cargo +1.99.1 test -p archy-authority --locked --offline` passed all authority
  unit and integration targets, including the real private-bus recovery test.
  The three CLI compatibility suites (`golden_artifacts`, `accepted_worker`,
  `desktop_readers`) passed 17/17 tests. All builds used
  `TMPDIR=/home/butahlecoq/work/archy`,
  `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/a14-a16-bus-availability-target-20261030-codex`,
  and `CARGO_BUILD_JOBS=2` on ext4.
- `sha256sum -c docs/plans/a14-a16-bus-availability-source.sha256`,
  `git diff --check`, and scoped `rustfmt --check` passed. The manifest fixes the
  four changed source/test/vendor-note files; this handoff is outside it to
  avoid a self-referential hash.

## Limits and disposition

The tested 70 KiB closure path is removed. A sender can still fill the bus
queue or keep sending large complete messages, delaying valid callers. The
per-frame byte ceiling, two-second deadline and yield points bound local
discard work, but they do not establish sustained availability or sender-level
fairness. If the real bus forwards a partially delivered message or allows
more than 16 FDs in one message, a timeout or ancillary truncation can still
close the shared connection. The private bus uses a 16-FD per-message ingress
limit; effective system-bus limits and native sustained-abuse behavior need
separate validation. The non-`async-io` build does not provide the asynchronous
receive timeout and was not used for this service test.

An independent security review of this follow-on has not yet been performed;
the earlier review applies to the base commit, not this change.

This is source and private-bus evidence only. The proposed unit and policy
were not installed or exercised on a native target. `Confirm` remains denied;
there is no trusted exact-byte display, fresh human authorization, durable
decision, or approval claim. Root witness integration and the prior review's
other deployment gates remain open. No push or host service action was made.
