# A21 doctor round2 independent harness rereview

Scope: corrected `tests/doctor/nonmutation_harness.py`, its four synthetic trace tests, and the updated doctor handoff. The passive-file bounds accepted in round1 were not reopened. No source edits, commits, or agents were used.

## Verdict

**Trace-attribution correction: ADDRESSED.** [`audit_traces`](../../tests/doctor/nonmutation_harness.py) starts each doctor PID segment only after a successful `execve` of the doctor binary (lines 39–48), then follows recorded `clone`/`clone3`/`fork`/`vfork` descendants (lines 49–58). It retains the executable allowlist and hostile shim rejection across all trace files (lines 34–38), and rejects network calls, writable opens, and filesystem mutation attempts in doctor/descendant segments (lines 59–63). This excludes Bubblewrap's pre-exec socket and uid/gid-map setup without exempting doctor or its children after exec.

The synthetic tests in [`test_nonmutation_trace.py`](../../tests/doctor/test_nonmutation_trace.py) cover the prior false positive, a post-exec doctor socket, a descendant writable open, and an unapproved exec. All four passed. I also independently ran the corrected audit over the seven retained `/tmp/archy-doctor-nonmutation-trace.*` files from the earlier false-failing run: it exited 0 and audited six doctor process/thread traces. The earlier PID 122030 log has Bubblewrap sockets on lines 1–2 and doctor `execve` on line 151; the corrected audit excludes only the former.

**Full nonmutation harness: coordinator-reported PASS, not independently rerun here.** The updated [`doctor-handoff.md:20-22`](doctor-handoff.md) records an exit-0 WSL run with workspace-local `strace`, six doctor traces across seven total, denied sandbox writes/network, and unchanged home/config/project inventories, hashes and modes. The harness still requires Bubblewrap's denial preflight (lines 89–101), parses the doctor JSON and deferred target acceptance (lines 107–113), and returns 77 when tracing is unavailable (lines 102–116). My reviewer sandbox previously failed Bubblewrap's NETLINK_ROUTE preflight, so I did not claim a local full-harness pass from that environment. The coordinator's new exit-0 evidence closes the round1 **WSL fixture nonmutation evidence** gate, subject to retaining its command/output as stated in the handoff.

## Independent checks run

- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/doctor -p test_nonmutation_trace.py -v`: exit 0, 4/4 passed.
- Corrected `audit_traces` invoked on the seven retained `/tmp/archy-doctor-nonmutation-trace.*` files: exit 0, six doctor process/thread traces audited.

No new harness finding was identified in this scoped rereview. Installed Omarchy doctor, package transactions, backends, live containment and desktop integration remain separate deferred target gates; this WSL fixture result does not close them.
