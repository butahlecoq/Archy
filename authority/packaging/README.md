# Target package fragments; do not install on the development host

These files are reviewable inputs for an Arch/Omarchy package, not an enabled
deployment. The existing `packaging/arch/package.py` stages only the core CLI; it
does not include these units or the authority binaries. A future package must
include their locked runtime dependency notices before distribution.
The system administrator must install immutable/root-owned
executables, create the two service accounts with sysusers, enroll exactly one
human UID in `/etc/archy/authority-human-uid` (decimal UID and newline, regular
root-owned file, mode 0644 or stricter), create the `archy-review` group via
sysusers, and add only the enrolled human to that group. The agent and broker
must not be members. Refresh the human login session after group enrollment,
then inspect effective unit properties
and UID/group/session reachability in a disposable target VM. The entire
coding agent, including its shell and tool execution, must run as
`archy-agent`; launching only the MCP adapter under that UID is insufficient.

The human socket is group-restricted (0660); the broker also denies all but
the enrolled UID before reading a packet. Its single method reads a proposal
by exact SHA-256. Eight fixed human workers serve this endpoint; each accepted
peer has checked one-second socket timeouts and clients have checked two-second
timeouts. A finite worker pool and kernel backlog can still saturate. Verify
the effective socket mode, membership, backlog behavior, and hostile-client
availability in a native target VM. The systemd service has no final approval API, and no final
decision should be enabled by installing these files.

At startup the daemon creates a mode-0700 `journal` child of its
broker-owned `StateDirectory`. It verifies the journal chain and refuses to
start if any unknown, malformed, changed or non-private entry is present.
Correctly named broker-owned private temporary files are removed and synced
under the journal lock after a pre-rename crash. This
is a storage primitive only: no decision is written, served or projected.
The two seqpacket endpoints deny the reserved decision opcode. Do not expose
the journal append API to either socket or interpret opaque records as
approval. The native package must verify effective state directory ownership
and persistence across service restart.

The service expects two systemd descriptors named `agent:human` at fds 3 and 4.
This ordering and `Sockets=` semantics require a native systemd VM check.
The daemon verifies socket type, listening state and bound filesystem path;
package acceptance must also confirm their ownership and mode. No host
installation, account creation or policy change was performed in this slice.
