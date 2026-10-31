#!/usr/bin/env python3
"""Opt-in disposable service check of the fixed gate's pre-launch protocol."""

import array
import json
import os
from pathlib import Path
import shutil
import socket
import stat
import subprocess
import sys
import uuid


def one_case(binary, send_request, *, extra=None, keep_open=False):
    job = uuid.uuid4().hex
    root = Path("/tmp") / f"archy-job-{job}"
    root.mkdir(mode=0o700)
    unit = f"archy-job-{job}.service"
    receiver, sender = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
    env = {"LC_ALL": "C", "XDG_RUNTIME_DIR": f"/run/user/{os.getuid()}"}
    child = None
    try:
        sentinel = root / "outside.txt"
        sentinel.write_text("unchanged\n")
        scratch = root / "scratch"
        scratch.mkdir(mode=0o700)
        cmd = ["/usr/bin/systemd-run", "--user", "--pipe", "--wait", "--collect",
               "--service-type=exec", "--expand-environment=no", "--quiet", f"--unit={unit}",
               f"--property=TemporaryFileSystem={scratch}:size=134217728,nr_inodes=256,mode=0700,nosuid,nodev,noexec",
               "--property=RuntimeMaxSec=20s", "--property=MemoryMax=2147483648",
               "--property=MemorySwapMax=0", "--property=CPUQuota=100%",
               "--property=TasksMax=64", "--property=KillMode=control-group",
               "--property=TimeoutStopSec=2s", "--property=SendSIGKILL=yes",
               "--property=LimitCORE=0", "--", str(Path(binary).resolve()),
               "--backend=blender", f"--scratch={scratch}"]
        child = subprocess.Popen(cmd, stdin=sender, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, env=env)
        sender.close()
        receiver.settimeout(10)
        frame, ancillary, flags, _ = receiver.recvmsg(64, socket.CMSG_SPACE(4))
        if frame != b"ARCHY-SCRATCH-1" or flags != 0:
            _, stderr = child.communicate(timeout=10)
            raise AssertionError((frame, flags, stderr))
        assert len(ancillary) == 1 and ancillary[0][:2] == (socket.SOL_SOCKET, socket.SCM_RIGHTS)
        descriptors = array.array("i")
        descriptors.frombytes(ancillary[0][2])
        assert len(descriptors) == 1
        fd = descriptors[0]
        try:
            info = os.fstat(fd)
            quota = os.fstatvfs(fd)
            assert stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
            assert info.st_mode & 0o7777 == 0o700
            assert 0 < quota.f_blocks * quota.f_frsize <= 134217728
            assert 0 < quota.f_files <= 256
            receiver.send(b"GO")
            if send_request:
                request = {"schema_version": "blender-worker-v1", "operation": "build_massing",
                           "request_id": "r1", "project_id": "p1", "variant_id": "variant-abc",
                           "run_id": "run1", "output_dir": "output", "params": {}, "site": {},
                           "input_digest": "x", "preset_id": "smoke-cpu-v1"}
                receiver.send(b"ARCHY-REQUEST-1\n" + json.dumps(request).encode())
                if extra is not None:
                    receiver.send(extra)
        finally:
            os.close(fd)
        if not keep_open:
            receiver.close()
        _, stderr = child.communicate(timeout=30)
        assert child.returncode != 0, "gate falsely reported success"
        if keep_open:
            expected = b"request channel EOF:"
        elif extra is not None:
            expected = b"trailing request packet"
        elif send_request:
            expected = b"runtime inventory:"
        else:
            expected = b"missing or oversized typed request frame"
        assert expected in stderr, stderr
        state = subprocess.run(["/usr/bin/systemctl", "--user", "show", unit,
                                "-pActiveState", "-pLoadState"], env=env,
                               capture_output=True, text=True, timeout=5, check=True).stdout
        assert "ActiveState=inactive" in state and "LoadState=not-found" in state, state
        assert sentinel.read_text() == "unchanged\n"
        print(f"{'open peer' if keep_open else 'extra packet' if extra is not None else 'request' if send_request else 'GO-only'}: bounded FD, unavailable, collected, host unchanged")
    finally:
        receiver.close()
        sender.close()
        subprocess.run(["/usr/bin/systemctl", "--user", "stop", unit], env=env,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
        if child is not None and child.poll() is None:
            child.kill()
            child.wait(timeout=5)
        shutil.rmtree(root)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: service_gate_unavailable.py /path/to/archy-service-gate")
    one_case(sys.argv[1], False)
    one_case(sys.argv[1], True)
    one_case(sys.argv[1], True, extra=b"EXTRA-FRAME")
    one_case(sys.argv[1], True, extra=b"")
    one_case(sys.argv[1], True, keep_open=True)
