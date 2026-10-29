# A16 broker slice

## Human decision follow-on (unavailable)

The broker now reserves `OP_DECIDE` and denies it on **both** seqpacket
channels, including the enrolled human's read channel. The human client
remains a read-only display of an untrusted proposal. No caller can cause a
selection, withdrawal, approved projection, or final-export status change.

`journal::Journal` is a broker-private append-only *storage primitive*. At
startup the service creates and verifies its mode-0700 `journal` directory
inside the broker-owned state directory. Opaque entries use a previous-hash
chain and sequential create-only names. A process mutex and Linux `flock`
serialize writers, exact idempotency keys replay the same record, a changed
payload conflicts, and a synced temporary file is atomically renamed before
the directory is synced. A failed post-rename sync leaves an uncertain commit;
the next append first syncs the directory and then scans/replays the chain.
Startup takes the journal lock and removes only correctly named, private
broker-owned temporary files left before rename, then syncs the directory.
Unknown, malformed, changed, non-private, or out-of-sequence entries block
further use. The daemon **does not call** the journal append method.
No opaque entry is interpreted as a decision. This protects against accidental
or agent-supplied `DecisionRecord` projection; it does not authenticate a human.
The journal has no external monotonic anchor, so someone who gains broker UID
or root and deletes a valid tail can cause rollback. Native deployment must
make that threat explicit and add an independently protected anchor if rollback
by a privileged local actor is in scope.

The pending design uses a separate **system-bus** review/confirm method. A
trusted installed human client should obtain a broker-created snapshot from
the protected project store, render its exact bytes and digest, then send an
affirmative action on the same bus connection. The service must use the
message's unique sender name, query the bus for host UID/PID credentials,
tie the PID to a live active local logind session without PID reuse, and
reject the agent UID. It must bind the snapshot and digest to that sender and
session with expiry and invalidation on disconnect/restart. After the action,
it must request a fresh Polkit `auth_self` challenge for that *unique bus name*
and reject immediate authorization, retention/temporary grants, cancellation,
session changes and policy changes. A generic Polkit YES is insufficient:
the service must bind the returned result to the displayed snapshot and
recheck all authority facts under the final commit lock. No Polkit response
can be invented by a unit test. The installed PAM/authentication agent and
target policy must be exercised by a real human in a disposable VM.

Before the journal can carry decisions, the broker must own a protected
source-of-truth project history. It must fetch the exact proposal and all
input, normalized, variant, evaluation and comparison artifacts from that
history; prove inclusion in a verified run and head/revision; recompute the
evaluation and comparison; reject changed, FAIL/WARN/UNKNOWN or stale
evidence; and recheck the head while holding the project commit lock. The
current `ProjectStore` and A14 `claimed_checks_passed_against` only verify
agent-writable history and claims, so neither is an authority source. A02/A04
versioned history work must be integrated independently, without copying its
schema or CLI changes into this slice. A published preapproval IFC remains
`DRAFT`; selection would append an event rather than rewrite history.

Official interface basis: [D-Bus unique names and credentials](https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-get-connection-credentials),
[logind session objects](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html),
[Polkit subject and authorization result](https://polkit.pages.freedesktop.org/polkit/eggdbus-interface-org.freedesktop.PolicyKit1.Authority.html),
[systemd service sandboxing](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html).
The reusable Rust [zbus](https://docs.rs/zbus/latest/zbus/) service/client
library and [pkcheck](https://polkit.pages.freedesktop.org/polkit/pkcheck.1.html)
were assessed; `pkcheck` is a command wrapper and cannot bind the required
displayed snapshot. Neither is added as a dependency until the native bus and
policy integration can be built and tested. No arbitrary command runner is
introduced.

The authority service is a system service under `archy-broker`. Its agent
seqpacket endpoint accepts only bounded A14 `Proposal` records from the
configured `archy-agent` host UID. The service copies each valid proposal into
its own private store using a content-addressed, create-only file and syncs it
before acknowledging. A separate review endpoint serves those stored bytes to
the enrolled human host UID. The installed review client prints the exact A14
claim and its SHA-256, clearly marked as unverified, and offers no selection
action. The agent endpoint has no read or decision operation.

The agent has one listener worker; the human channel has exactly eight workers
sharing its listener, with no broker-side connection queue or per-peer thread.
Four silent human peers therefore leave workers for a queued valid read. Accepted
connections have checked one-second receive and send timeouts; clients have
checked two-second timeouts and a two-second bounded nonblocking connect. The
package restricts the human socket to the `archy-review` group (mode 0660),
whose only non-root member must be the enrolled human. The broker still checks
the exact enrolled UID with `SO_PEERCRED` before receiving a packet. Eight
stalled authenticated peers can occupy all workers for one timeout interval;
more peers, a full finite kernel backlog, scheduler delays, or a blocked
filesystem operation can still make a request miss its client deadline. No
OS-level latency guarantee follows from these settings. The private store admits at most 1024 entries, including abandoned
temporary files. Since a submitted proposal is smaller than 64 KiB, broker
created proposal bytes are bounded below 64 MiB. At capacity, new submissions
are denied while reads and matching replays remain available. A replay syncs
the directory before success, including after a previous rename whose
directory sync failed. A filesystem operation itself has no kernel-enforced
wall-clock deadline; deployment must still provision and monitor the state
filesystem.

The systemd socket units provide two fixed listener descriptors. The service
validates their names, socket type and local address before accepting traffic.
It reads `SO_PEERCRED` from each accepted connection, rejects file descriptors
and truncated packets, and never trusts UID/actor fields on the wire. Installed
account names and a root-owned enrollment file determine identities. The
broker refuses to start as root or with overlapping agent, broker, or human
UIDs. Existing A14 `DecisionRecord` stays an untrusted data type; this slice
does not project any approved state.

The review socket and CLI are an interim read-only channel. Final selection
requires a system-bus service method bound to a unique sender, logind local
session verification, non-cached polkit challenge after a displayed snapshot,
and a protected single-writer decision store with evidence recomputation.
Those are not implemented here, so there is no acceptance path on WSL or on
an installed system. The client's read access alone is not human consent.
`SO_PEERCRED` captures the connecting peer, so passing an already connected
socket descriptor can delegate this read access. The listener name check also
does not prove the current filesystem entry points at that socket. Root-owned
runtime directory and effective socket ownership remain native deployment
gates, not verified properties of this worktree.

The crate is a member of the root Cargo workspace and uses its locked
dependencies. This integration changes the root Cargo manifest and lockfile;
the A14 claim contract, CLI, MCP, schema checker and package staging code
remain as published in `f1e2477`.

## Dependencies and upstream basis

`archy-core` supplies the published A14 untrusted proposal contract. `libc`
provides Linux `SO_PEERCRED`, `recvmsg`, `openat`, and `renameat2` without an
extra transport runtime. `serde`/`serde_json` parse that contract, and `sha2`
names protected proposal copies. These are already pinned in the release
workspace; the nested crate pins the same versions. No polkit, D-Bus or
systemd library was added because this slice has no final approval method.

The next slice should implement the human confirmation interface on the
system bus. [D-Bus connection credentials](https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-get-connection-credentials)
identify a unique bus sender; [logind `GetSessionByPID`](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html)
can locate its session. [Polkit's authorization API](https://www.freedesktop.org/software/polkit/docs/latest/PolkitAuthority.html)
says interaction belongs after a user action, and [the result API](https://www.freedesktop.org/software/polkit/docs/latest/PolkitAuthorizationResult.html)
distinguishes challenge, cached/temporary grants and retention. A generic
polkit success does not bind the exact design. [systemd socket units](https://www.freedesktop.org/software/systemd/man/latest/systemd.socket.html)
support `ListenSequentialPacket` and socket ownership; [activation descriptors](https://www.freedesktop.org/software/systemd/man/latest/sd_listen_fds.html)
are passed with `LISTEN_FDS` and `LISTEN_FDNAMES`. Native installed behavior
must be checked before enabling the unit.

## Exact target gates for final selection

1. In a disposable Arch/Omarchy VM, install fixed root-owned binaries and
   policy; verify effective systemd unit, socket paths, host UIDs/groups,
   account non-login status, private state ownership and agent confinement.
   The agent's whole execution tree must be under the agent UID with no human
   home/session, privileged manager, broker store or unapproved network access.
2. Implement a separate system-bus confirmation method. Derive the caller's
   unique sender and host UID from the bus, resolve a live active local logind
   session without PID reuse ambiguity, and reject the agent before any polkit
   dialog. Tie one bounded review snapshot and digest to that connection and
   session, expiring it on disconnect, policy change, timeout or restart.
3. The trusted, administrator-installed human client must render the exact
   broker snapshot and require an explicit affirmative selection. A fresh
   non-cached polkit `auth_self` challenge must follow that selection. Deny
   immediate YES, retention, canceled/absent/inactive sessions, policy changes
   and mismatched sender/digest. Test with the target PAM and auth agent;
   authentication alone does not prove what was displayed.
4. Replace proposal-only storage with a single broker-owned authoritative
   project store. Validate A14's claimed evidence against immutable history,
   recompute evaluation and comparison, recheck head/revision under the commit
   lock, and durably record exactly one event with actor and time derived in
   the broker. Test crash points, replay, rollback, changed evidence and
   uncertain commit. Existing agent-writable records cannot become trusted.
5. Run separate-agent negative probes and human-positive review on that VM,
   including forged actor, forwarded connection, cached polkit grant, stale
   snapshot, service restart and protected-store tampering. Until every gate
   passes, final approval remains unavailable and exports stay DRAFT.
