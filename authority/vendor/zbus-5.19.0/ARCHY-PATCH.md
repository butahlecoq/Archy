# Local zbus 5.19.0 receive bound

This directory is the crates.io zbus 5.19.0 source with a narrow receive-size patch.
The original `.crate` archive SHA-256 is
`5db4be7c075cb421e4b7ee645541604239bd243ba7c357511f4ff3a74b555907`,
matching the original `Cargo.lock` registry checksum. The upstream package
declares MIT; its unchanged `LICENSE` SHA-256 is
`260e4ff3fc493695880c90774283eb1e3d018f898fabe655bbd52dd2cbd2fcad`.
Keep the license with any distributed copy. The package and its normal
dependency notices still require inventory before staging `archy-human-busd`.

The local changes add `Builder::max_message_size`, carry that value through
client Hello authentication and the socket reader, and reject a complete
message length derived from the 16-byte primary header before `Vec::resize`
or body reads. The limit is per connection, defaults to the upstream 128 MiB,
and is clamped to that upstream ceiling. A length overflow is rejected. The
service sets 64 KiB. No daemon-wide bus policy is changed.

For review, compare this directory against the unpacked archive with
`diff -ru --exclude=ARCHY-PATCH.md upstream/zbus-5.19.0 authority/vendor/zbus-5.19.0`.
The seven modified upstream files are `src/connection/builder.rs`,
`src/connection/handshake/{client,mod}.rs`, `src/connection/mod.rs`,
`src/connection/socket/{channel,mod}.rs`, and `src/connection/socket_reader.rs`.
The exact unified diff is `docs/plans/a14-a16-zbus-receive-bound.patch`.
The candidate file hashes are in
`docs/plans/a14-a16-production-receive-bound-source.sha256`.

The subsequent availability candidate additionally bounds Unix ancillary
SCM_RIGHTS reception to 16 descriptors per `recvmsg` and per message, counts
descriptors explicitly, and rejects `MSG_CTRUNC`. Those changes are in
`src/utils.rs`, `src/connection/socket/unix.rs`, and
`src/connection/socket/mod.rs`.
The subsequent close-on-exec correction uses the kernel's
`MSG_CMSG_CLOEXEC` on Linux, Android, and BSD systems where rustix exposes it.
An unsupported Unix platform returns `Unsupported` before `recvmsg`, since
setting `FD_CLOEXEC` after receiving rights would race a concurrent fork or
spawn. A kernel that rejects the flag propagates its error without retrying
without the flag. Real socket tests cover exec inheritance, descriptor counts,
truncation, and cleanup on rejection.
This affects every zbus connection in this package; the only workspace user
is the authority bus service, which needs one ProcessFD from the bus. Target
compatibility and separate security review remain required. The later file
hashes are frozen in `docs/plans/a14-a16-production-availability-source.sha256`;
the earlier receive-bound patch and hash manifest remain historical evidence.

The human-bus availability follow-on keeps the 64 KiB accepted-frame allocation
limit while discarding a complete oversized frame in 4 KiB reads. Its header
length must fit the D-Bus 128 MiB protocol ceiling; each partial header or
frame has a two-second receive deadline under the default `async-io` runtime.
Discarded ancillary FDs are closed without dispatch, and a short delay follows
each discarded frame. Invalid framing, ancillary truncation, timeout, or a
length above the protocol ceiling still closes the connection. The previous
patch and manifests remain historical; the new exact source state is recorded
in the availability follow-on handoff.
