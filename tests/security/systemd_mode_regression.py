#!/usr/bin/env python3
"""Prove the disposable service rejects an insecure mount before its start token."""

import json
import shutil
import subprocess
import tempfile
from pathlib import Path


source = Path(__file__).resolve().parent
with tempfile.TemporaryDirectory(prefix="archy-a09-mode-regression-", dir="/var/tmp") as temporary:
    root = Path(temporary)
    shutil.copyfile(source / "scratch_fd_probe.py", root / "scratch_fd_probe.py")
    script = (source / "systemd_scratch_fd_probe.py").read_text(encoding="utf-8")
    assert script.count("mode=0700") == 1
    assert script.count('parent_socket.sendall(b"GO")') == 1
    script = script.replace("mode=0700", "mode=0777")
    script = script.replace(
        'parent_socket.sendall(b"GO")',
        'raise AssertionError("unexpected start token")',
    )
    (root / "systemd_scratch_fd_probe.py").write_text(script, encoding="utf-8")
    result = subprocess.run(
        ["/usr/bin/python3", "-B", str(root / "systemd_scratch_fd_probe.py")],
        capture_output=True, text=True, timeout=35,
    )
    assert result.returncode == 1, (result.returncode, result.stdout, result.stderr)
    assert not result.stdout, result.stdout
    evidence = json.loads(result.stderr)
    assert evidence == {
        "status": "unavailable",
        "reason": "scratch directory private mode must be 0700",
    }, evidence
    print(json.dumps({"status": "pass", "mode": "0777", "start_token": "not_reached"}))
