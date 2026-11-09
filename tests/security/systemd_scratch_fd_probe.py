#!/usr/bin/env python3
"""Bounded user-service scratch FD and cgroup proof; fixture only."""

import array
import errno
import hashlib
import json
import os
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

sys.dont_write_bytecode = True
import scratch_fd_probe as kernel


SYSTEMD_RUN = "/usr/bin/systemd-run"
SYSTEMCTL = "/usr/bin/systemctl"
PYTHON = "/usr/bin/python3"
CGROUP = Path("/sys/fs/cgroup")
MEMORY = 256 * 1024 * 1024
TASKS = 16
DEADLINE = 30
PAYLOAD = b"archy-systemd-private-scratch\n"


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def run_fixed(command, timeout=4):
    result = subprocess.run(
        command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, timeout=timeout,
    )
    require(len(result.stdout) <= 8192 and len(result.stderr) <= 8192, "command output bound exceeded")
    require(result.returncode == 0, f"command exit {result.returncode}: {result.stderr[:512]!r}")
    return result.stdout.decode("utf-8", "replace")


def properties(unit):
    keys = [
        "LoadState", "ActiveState", "ControlGroup", "MainPID", "MemoryMax",
        "MemorySwapMax", "TasksMax", "KillMode", "RuntimeMaxUSec", "TimeoutStopUSec",
    ]
    output = run_fixed([SYSTEMCTL, "--user", "show", unit, *[f"-p{key}" for key in keys]])
    return dict(line.split("=", 1) for line in output.splitlines() if "=" in line)


def cgroup_path(value):
    require(value.startswith("/user.slice/") and ".." not in value.split("/"), "unsafe cgroup path")
    path = CGROUP / value.lstrip("/")
    require(path.is_dir(), "service cgroup absent")
    return path


def mount_record(pid, mountpoint):
    with open(f"/proc/{pid}/mountinfo", encoding="utf-8") as stream:
        for line in stream:
            left, separator, right = line.partition(" - ")
            if separator and left.split()[4] == str(mountpoint):
                return left.split()[5], right.split()[0]
    raise AssertionError("service tmpfs mount absent")


def service(mountpoint):
    path = Path(mountpoint)
    channel = socket.socket(fileno=0)
    channel.settimeout(8)
    try:
        fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        try:
            fs = os.fstatvfs(fd)
            require(0 < fs.f_blocks * fs.f_frsize <= kernel.SIZE, "service byte quota absent")
            require(0 < fs.f_files <= kernel.INODES, "service inode quota absent")
            info = {
                "pid": os.getpid(),
                "st_dev": os.fstat(fd).st_dev,
                "st_ino": os.fstat(fd).st_ino,
                "byte_capacity": fs.f_blocks * fs.f_frsize,
                "inode_capacity": fs.f_files,
            }
            frame = json.dumps(info, sort_keys=True).encode("ascii")
            sent = channel.sendmsg(
                [frame], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array("i", [fd]))]
            )
            require(sent == len(frame), "short service FD frame")
            require(channel.recv(8) == b"GO", "missing bounded start token")
            output = os.open("payload.bin", os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600, dir_fd=fd)
            try:
                require(os.write(output, PAYLOAD) == len(PAYLOAD), "short fixture write")
            finally:
                os.close(output)
        finally:
            os.close(fd)
    finally:
        channel.close()


def parent():
    require(all(Path(tool).is_file() for tool in (SYSTEMD_RUN, SYSTEMCTL, PYTHON)), "fixed tool absent")
    start = time.monotonic()
    baseline_fds = len(os.listdir("/proc/self/fd"))
    source_inventory = set(os.listdir(Path(__file__).parent))
    unit = f"archy-scratch-fd-{uuid.uuid4().hex}.service"
    with tempfile.TemporaryDirectory(prefix="archy-systemd-fd-") as temporary:
        root = Path(temporary)
        mountpoint = root / "scratch"
        mountpoint.mkdir(mode=0o700)
        outside = root / "outside"
        outside.write_bytes(b"outside-unchanged\n")
        host_dev = mountpoint.stat().st_dev
        parent_socket, service_socket = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
        parent_socket.settimeout(8)
        mount_property = (
            f"TemporaryFileSystem={mountpoint}:size={kernel.SIZE},nr_inodes={kernel.INODES},"
            "mode=0700,nosuid,nodev,noexec"
        )
        command = [
            SYSTEMD_RUN, "--user", "--pipe", "--wait", "--collect",
            "--expand-environment=no", "--quiet", f"--unit={unit}",
            f"--property={mount_property}",
            "--property=RuntimeMaxSec=20s", f"--property=MemoryMax={MEMORY}",
            "--property=MemorySwapMax=0", f"--property=TasksMax={TASKS}",
            "--property=KillMode=control-group", "--property=TimeoutStopSec=2s",
            "--", PYTHON, "-B", str(Path(__file__).resolve()), "--internal-service", str(mountpoint),
        ]
        process = subprocess.Popen(
            command, stdin=service_socket, stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE, start_new_session=True,
        )
        service_socket.close()
        descriptor = None
        try:
            try:
                descriptor, info = kernel.receive_one_directory(parent_socket)
            except (OSError, AssertionError, ValueError) as exc:
                raise AssertionError(f"systemd-run control socket unavailable: {exc}") from exc
            require(time.monotonic() - start < DEADLINE, "probe deadline before verification")
            unit_state = properties(unit)
            require(unit_state.get("LoadState") == "loaded", "unit not loaded")
            require(unit_state.get("ActiveState") == "active", "service not active before token")
            require(unit_state.get("MainPID") == str(info["pid"]), "service PID mismatch")
            require(unit_state.get("MemoryMax") == str(MEMORY), "MemoryMax property mismatch")
            require(unit_state.get("MemorySwapMax") == "0", "MemorySwapMax property mismatch")
            require(unit_state.get("TasksMax") == str(TASKS), "TasksMax property mismatch")
            require(unit_state.get("KillMode") == "control-group", "KillMode property mismatch")
            require(unit_state.get("RuntimeMaxUSec") == "20s", "RuntimeMaxSec property mismatch")
            require(unit_state.get("TimeoutStopUSec") == "2s", "stop grace property mismatch")
            group = cgroup_path(unit_state["ControlGroup"])
            require((group / "memory.max").read_text().strip() == str(MEMORY), "effective memory.max mismatch")
            require((group / "memory.swap.max").read_text().strip() == "0", "effective memory.swap.max mismatch")
            require((group / "pids.max").read_text().strip() == str(TASKS), "effective pids.max mismatch")
            require("populated 1" in (group / "cgroup.events").read_text(), "service cgroup not populated")
            options, kind = mount_record(info["pid"], mountpoint)
            require(kind == "tmpfs", "service mount is not tmpfs")
            require({"nosuid", "nodev", "noexec"} <= set(options.split(",")), "tmpfs mount flags missing")
            metadata = os.fstat(descriptor)
            require(stat.S_ISDIR(metadata.st_mode), "transferred FD not a directory")
            require(metadata.st_uid == os.getuid(), "scratch directory owner mismatch")
            require(stat.S_IMODE(metadata.st_mode) == 0o700, "scratch directory private mode must be 0700")
            require((metadata.st_dev, metadata.st_ino) == (info["st_dev"], info["st_ino"]), "FD identity mismatch")
            fs = os.fstatvfs(descriptor)
            require(fs.f_blocks * fs.f_frsize == info["byte_capacity"] == kernel.SIZE, "retained byte quota mismatch")
            require(fs.f_files == info["inode_capacity"] == kernel.INODES, "retained inode quota mismatch")
            require(os.listdir(descriptor) == [], "fixture wrote before start token")
            parent_socket.sendall(b"GO")
            _, errors = process.communicate(timeout=max(0.1, DEADLINE - (time.monotonic() - start) - 5))
            require(len(errors) <= 8192, "systemd-run output bound exceeded")
            require(process.returncode == 0, f"systemd-run exit {process.returncode}: {errors[:512]!r}")
            after = properties(unit)
            require(after.get("ActiveState") == "inactive", "service did not become inactive")
            cgroup_exit = "absent"
            if group.exists():
                require("populated 0" in (group / "cgroup.events").read_text(), "service cgroup still populated")
                cgroup_exit = "empty"
            file_fd = os.open("payload.bin", os.O_RDONLY | os.O_CLOEXEC, dir_fd=descriptor)
            try:
                data = os.read(file_fd, len(PAYLOAD) + 1)
            finally:
                os.close(file_fd)
            require(data == PAYLOAD, "retained FD payload mismatch")
            require(os.listdir(descriptor) == ["payload.bin"], "retained inventory mismatch")
            require(mountpoint.stat().st_dev == host_dev and list(mountpoint.iterdir()) == [], "host backing changed")
            require(outside.read_bytes() == b"outside-unchanged\n", "outside sentinel changed")
            os.close(descriptor)
            closed = descriptor
            descriptor = None
            try:
                os.fstat(closed)
            except OSError as exc:
                require(exc.errno == errno.EBADF, "retained FD did not close")
            else:
                raise AssertionError("retained FD remained open")
            evidence = {
                "status": "pass", "unit": unit, "byte_capacity": fs.f_blocks * fs.f_frsize,
                "inode_capacity": fs.f_files, "memory_max": MEMORY, "memory_swap_max": 0,
                "pids_max": TASKS, "service_inactive": True, "cgroup_empty": True,
                "cgroup_exit": cgroup_exit, "unit_load_state_after": after.get("LoadState"),
                "control_group": unit_state["ControlGroup"], "mount_type": kind,
                "mount_flags": sorted({"nosuid", "nodev", "noexec"} & set(options.split(","))),
                "scratch_uid": metadata.st_uid, "scratch_mode": format(stat.S_IMODE(metadata.st_mode), "04o"),
                "runtime_max": unit_state["RuntimeMaxUSec"],
                "stop_grace": unit_state["TimeoutStopUSec"],
                "kill_mode": unit_state["KillMode"],
                "payload_sha256": hashlib.sha256(data).hexdigest(),
            }
        finally:
            parent_socket.close()
            if descriptor is not None:
                os.close(descriptor)
            try:
                subprocess.run(
                    [SYSTEMCTL, "--user", "stop", unit], stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3,
                )
            except subprocess.TimeoutExpired:
                pass
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                try:
                    process.communicate(timeout=2)
                except subprocess.TimeoutExpired:
                    process.stderr.close()
                    process.wait(timeout=2)
    require(len(os.listdir("/proc/self/fd")) == baseline_fds, "parent FD leaked")
    require(set(os.listdir(Path(__file__).parent)) == source_inventory, "source directory changed")
    require(time.monotonic() - start <= DEADLINE, "whole probe deadline exceeded")
    print(json.dumps(evidence, sort_keys=True))


if __name__ == "__main__":
    try:
        if len(sys.argv) == 3 and sys.argv[1] == "--internal-service":
            service(sys.argv[2])
        elif len(sys.argv) == 1:
            parent()
        else:
            raise AssertionError("usage: systemd_scratch_fd_probe.py")
    except (AssertionError, OSError, subprocess.TimeoutExpired) as exc:
        print(json.dumps({"status": "unavailable", "reason": str(exc)[:512]}), file=sys.stderr)
        raise SystemExit(1) from exc
