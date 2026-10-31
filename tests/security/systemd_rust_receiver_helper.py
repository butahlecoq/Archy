#!/usr/bin/env python3
"""Fixed transient-service helper for the Rust scratch receiver fixture."""

import array
import os
import socket
import sys


if len(sys.argv) != 3 or sys.argv[2] not in ("directory", "regular"):
    raise SystemExit("invalid fixture invocation")

channel = socket.socket(fileno=0)
channel.settimeout(12)
path = sys.argv[1] if sys.argv[2] == "directory" else "/dev/null"
flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NONBLOCK
if sys.argv[2] == "directory":
    flags |= os.O_DIRECTORY
descriptor = os.open(path, flags)
try:
    marker = b"ARCHY-SCRATCH-1"
    if channel.sendmsg([marker], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array("i", [descriptor]))]) != len(marker):
        raise RuntimeError("short fixture descriptor frame")
    token = channel.recv(8)
    if token == b"":
        if channel.send(b"NO-GO") != 5:
            raise RuntimeError("short no-token acknowledgement")
    elif token == b"GO" and sys.argv[2] == "directory":
        output = os.open("payload.bin", os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600, dir_fd=descriptor)
        try:
            payload = b"archy-rust-service-receiver\n"
            if os.write(output, payload) != len(payload):
                raise RuntimeError("short fixture payload")
        finally:
            os.close(output)
    else:
        raise RuntimeError("invalid fixture token")
finally:
    os.close(descriptor)
    channel.close()
