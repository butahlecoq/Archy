# A09 scratch FD kernel probe handoff

Date: 2026-10-29. Owned files: `tests/security/scratch_fd_probe.py` and this handoff only. This is a disposable WSL filesystem proof for the candidate at the end of [worker-supervisor-plan.md](worker-supervisor-plan.md). It does not launch Blender, IFC, a production worker, systemd service or bwrap; no product source, dependency, lockfile, commit, push or agent was changed.

## Fixture and decision

The script has no public fixture/tool override. It uses fixed `/usr/bin/unshare`, `/usr/bin/mount` and `/usr/bin/python3` with Python standard library only. The parent creates a temporary host directory and an outside sentinel. A child in a new user and private mount namespace mounts tmpfs at a child-only path with `size=65536,nr_inodes=8,mode=0700,nosuid,nodev,noexec`. The child checks its mountinfo type/options and `statvfs` capacities, separately drives byte and inode allocation to real `ENOSPC`, removes those exhaustion files, and writes one small payload.

The child sends one directory FD and a bounded evidence frame over a Unix `SOCK_SEQPACKET` socket using `SCM_RIGHTS`. The parent requires exactly one directory FD, `MSG_CMSG_CLOEXEC`, no truncation or unexpected ancillary type; a real two-FD message is rejected and its received FDs are closed. After the namespace process exits, the parent checks the retained directory identity/capacities, lists only `payload.bin`, opens it descriptor-relative and compares its bytes and SHA-256. It verifies the host backing directory is empty and on its original filesystem, the outside sentinel is unchanged, closes the retained FD, observes `EBADF`, and checks its own FD count returned to baseline. The temporary host directory is then removed. Each subprocess has a fixed timeout, and the unshare process group is killed and reaped on failure.

## Exact WSL evidence

| Command | Result |
| --- | --- |
| `/usr/bin/python3 tests/security/scratch_fd_probe.py` | Exit 0. JSON: `byte_capacity=65536`, `byte_enospc_after_bytes=65536`, `inode_capacity=8`, `inode_enospc_after_files=7`, `payload_sha256=2f5efc92e89ef48ea36433d29011baf16f63cd84c4e90c769534e31bee93ee14`, `status=pass`. `st_dev=87`, `st_ino=1` on this run; device numbers are run-specific. |
| `/usr/bin/python3 -c 'import ast, pathlib; ast.parse(pathlib.Path("tests/security/scratch_fd_probe.py").read_text())'` | Exit 0. |

The first live run passed with the same measured byte/inode caps. The added two-FD negative check first exited 1 because its test harness incorrectly used a tuple as a context manager; that harness error was fixed, then the full probe passed. The final run above includes the extra-descriptor rejection, post-close host check and parent FD count check. Runtime: Python 3.14.4, util-linux `unshare`/`mount` 2.41.3, WSL2 kernel `6.6.87.2-microsoft-standard-WSL2`. Final script SHA-256: `94be127b1039b3dadc1a1b2e888414bdc96a69641b252182b237fbcfac14d423`.

## Limits and next gate

This proves that this WSL kernel enforces the selected nonzero tmpfs byte and inode limits in a disposable namespace and permits reading through a retained directory FD after the namespace process exits. Closing the last parent FD is observed as `EBADF` and no host mount or backing file remains; the test does not directly measure kernel memory reclamation. The evidence frame comes from this trusted fixture child, not an authenticated production service. A future production handoff must validate descriptor count/type/ownership/filesystem identity and effective capacity independently, prove the actual systemd user-service control-FD path and cgroup teardown, cap every writable mount, and verify bwrap/network policy. Production quota or worker readiness, actual Omarchy behavior, and publication authority are not established here.
