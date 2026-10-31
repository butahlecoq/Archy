#!/usr/bin/env python3
"""Disposable Linux namespace and tmpfs descriptor proof; never launches a worker."""

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
from pathlib import Path


UNSHARE = "/usr/bin/unshare"
MOUNT = "/usr/bin/mount"
PYTHON = "/usr/bin/python3"
SIZE = 64 * 1024
INODES = 8
PAYLOAD = b"archy-private-tmpfs-fd-proof\n"
DEADLINE = 20


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def mount_record(path):
    target = str(path)
    with open("/proc/self/mountinfo", encoding="utf-8") as stream:
        for line in stream:
            left, separator, right = line.partition(" - ")
            if separator and left.split()[4] == target:
                return left.split()[5], right.split()[0]
    raise AssertionError("tmpfs mount absent from child mountinfo")


def force_byte_enospc(path):
    used = 0
    try:
        with open(path / "byte-limit", "wb", buffering=0) as stream:
            for _ in range(1 + SIZE // 4096):
                stream.write(b"x" * 4096)
                used += 4096
    except OSError as exc:
        require(exc.errno == errno.ENOSPC, f"byte fill error: {exc}")
        require(used > 0, "byte cap is zero")
        return used
    finally:
        (path / "byte-limit").unlink(missing_ok=True)
    raise AssertionError("byte cap did not produce ENOSPC")


def force_inode_enospc(path):
    created = []
    try:
        for index in range(INODES + 2):
            entry = path / f"inode-{index}"
            try:
                entry.touch(exist_ok=False)
            except OSError as exc:
                require(exc.errno == errno.ENOSPC, f"inode fill error: {exc}")
                require(created, "inode cap is zero")
                return len(created)
            created.append(entry)
    finally:
        for entry in created:
            entry.unlink()
    raise AssertionError("inode cap did not produce ENOSPC")


def child(socket_number, mountpoint):
    path = Path(mountpoint)
    command = [
        MOUNT,
        "-t",
        "tmpfs",
        "-o",
        f"size={SIZE},nr_inodes={INODES},mode=0700,nosuid,nodev,noexec",
        "tmpfs",
        str(path),
    ]
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
    require(result.returncode == 0, f"private tmpfs mount failed: {result.stderr[:512]!r}")
    options, kind = mount_record(path)
    require(kind == "tmpfs", f"mounted filesystem is {kind}")
    require({"nosuid", "nodev", "noexec"} <= set(options.split(",")), "tmpfs flags missing")
    fs = os.statvfs(path)
    require(0 < fs.f_blocks * fs.f_frsize <= SIZE, "byte cap absent or larger than requested")
    require(0 < fs.f_files <= INODES, "inode cap absent or larger than requested")
    byte_used = force_byte_enospc(path)
    inode_used = force_inode_enospc(path)
    (path / "payload.bin").write_bytes(PAYLOAD)
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        info = {
            "byte_enospc_after_bytes": byte_used,
            "inode_enospc_after_files": inode_used,
            "st_dev": os.fstat(fd).st_dev,
            "st_ino": os.fstat(fd).st_ino,
            "byte_capacity": fs.f_blocks * fs.f_frsize,
            "inode_capacity": fs.f_files,
            "payload_sha256": hashlib.sha256(PAYLOAD).hexdigest(),
        }
        message = json.dumps(info, sort_keys=True).encode("ascii")
        with socket.socket(fileno=socket_number) as channel:
            sent = channel.sendmsg([message], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array("i", [fd]))])
            require(sent == len(message), "short descriptor message")
    finally:
        os.close(fd)


def receive_one_directory(channel):
    data, ancillary, flags, address = channel.recvmsg(
        4096, socket.CMSG_SPACE(8 * array.array("i").itemsize), socket.MSG_CMSG_CLOEXEC
    )
    received = []
    try:
        for level, kind, payload in ancillary:
            require(level == socket.SOL_SOCKET and kind == socket.SCM_RIGHTS, "unexpected ancillary type")
            require(len(payload) % array.array("i").itemsize == 0, "malformed descriptor payload")
            numbers = array.array("i")
            numbers.frombytes(payload)
            received.extend(numbers)
        require(not flags & (socket.MSG_CTRUNC | socket.MSG_TRUNC), "truncated descriptor message")
        require(address in (None, b""), "unexpected socket peer address")
        require(len(data) <= 4096 and len(received) == 1, "expected exactly one descriptor")
        require(received[0] >= 0, "invalid descriptor")
        mode = os.fstat(received[0]).st_mode
        require(stat.S_ISDIR(mode), "received descriptor is not a directory")
        require(not os.get_inheritable(received[0]), "received descriptor lacks CLOEXEC")
        info = json.loads(data)
        require(isinstance(info, dict), "invalid evidence frame")
        return received.pop(), info
    finally:
        for fd in received:
            os.close(fd)


def reject_extra_descriptors(root):
    sender, receiver = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
    try:
        first = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        second = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        try:
            sent = sender.sendmsg(
                [b"{}"],
                [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array("i", [first, second]))],
            )
            require(sent == 2, "short negative-test message")
            before = len(os.listdir("/proc/self/fd"))
            try:
                receive_one_directory(receiver)
            except AssertionError as exc:
                require(str(exc) == "expected exactly one descriptor", f"wrong rejection: {exc}")
            else:
                raise AssertionError("two descriptors were accepted")
            require(len(os.listdir("/proc/self/fd")) == before, "rejected descriptors leaked")
        finally:
            os.close(first)
            os.close(second)
    finally:
        sender.close()
        receiver.close()


def parent():
    require(all(Path(tool).is_file() for tool in (UNSHARE, MOUNT, PYTHON)), "fixed tool absent")
    baseline_fds = len(os.listdir("/proc/self/fd"))
    with tempfile.TemporaryDirectory(prefix="archy-scratch-fd-") as temporary:
        root = Path(temporary)
        mountpoint = root / "scratch"
        mountpoint.mkdir(mode=0o700)
        outside = root / "outside"
        outside.write_bytes(b"outside-unchanged\n")
        reject_extra_descriptors(root)
        host_dev = mountpoint.stat().st_dev
        parent_socket, child_socket = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
        parent_socket.settimeout(DEADLINE)
        command = [
            UNSHARE, "--user", "--map-root-user", "--mount", "--propagation", "private",
            "--fork", "--kill-child", "--", PYTHON, str(Path(__file__).resolve()),
            "--internal-child", str(child_socket.fileno()), str(mountpoint),
        ]
        process = subprocess.Popen(
            command, pass_fds=(child_socket.fileno(),), stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, start_new_session=True,
        )
        child_socket.close()
        descriptor = None
        try:
            try:
                descriptor, info = receive_one_directory(parent_socket)
            except (OSError, AssertionError) as exc:
                raise AssertionError(f"namespace descriptor handoff failed: {exc}") from exc
            try:
                _, errors = process.communicate(timeout=DEADLINE)
            except subprocess.TimeoutExpired as exc:
                raise AssertionError("namespace did not exit before deadline") from exc
            require(process.returncode == 0, f"namespace exited {process.returncode}: {errors[:512]!r}")
            metadata = os.fstat(descriptor)
            require(metadata.st_dev == info["st_dev"] and metadata.st_ino == info["st_ino"], "directory identity changed")
            fs = os.fstatvfs(descriptor)
            require(0 < fs.f_blocks * fs.f_frsize <= SIZE, "retained byte cap absent")
            require(0 < fs.f_files <= INODES, "retained inode cap absent")
            require(os.listdir(descriptor) == ["payload.bin"], "retained inventory differs")
            file_fd = os.open("payload.bin", os.O_RDONLY | os.O_CLOEXEC, dir_fd=descriptor)
            try:
                payload = os.read(file_fd, len(PAYLOAD) + 1)
            finally:
                os.close(file_fd)
            require(payload == PAYLOAD, "retained descriptor read differs")
            require(hashlib.sha256(payload).hexdigest() == info["payload_sha256"], "payload digest differs")
            require(info["byte_enospc_after_bytes"] > 0 and info["inode_enospc_after_files"] > 0, "exhaustion evidence absent")
            require(mountpoint.stat().st_dev == host_dev and list(mountpoint.iterdir()) == [], "host backing changed")
            require(outside.read_bytes() == b"outside-unchanged\n", "outside sentinel changed")
            os.close(descriptor)
            closed = descriptor
            descriptor = None
            try:
                os.fstat(closed)
            except OSError as exc:
                require(exc.errno == errno.EBADF, f"closed descriptor error: {exc}")
            else:
                raise AssertionError("descriptor remained open")
            require(mountpoint.stat().st_dev == host_dev and not os.path.ismount(mountpoint), "host mount changed after close")
        finally:
            parent_socket.close()
            if descriptor is not None:
                os.close(descriptor)
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                try:
                    process.communicate(timeout=2)
                except subprocess.TimeoutExpired:
                    process.stderr.close()
                    process.wait(timeout=2)
    require(len(os.listdir("/proc/self/fd")) == baseline_fds, "parent descriptor leaked")
    print(json.dumps({"status": "pass", **info}, sort_keys=True))


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "--internal-child":
        child(int(sys.argv[2]), sys.argv[3])
    elif len(sys.argv) == 1:
        parent()
    else:
        raise SystemExit("usage: scratch_fd_probe.py")
