"""Small ELF64 program-header fixtures for package admission tests."""

import struct


def gate_elf(*, dynamic=False):
    data = bytearray(120)
    data[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<H", data, 16, 3)
    struct.pack_into("<H", data, 18, 62)
    struct.pack_into("<I", data, 20, 1)
    struct.pack_into("<Q", data, 24, 64)
    struct.pack_into("<Q", data, 32, 64)
    struct.pack_into("<HHH", data, 52, 64, 56, 1)
    struct.pack_into("<I", data, 64, 3 if dynamic else 1)
    struct.pack_into("<I", data, 68, 5)
    struct.pack_into("<QQQQQQ", data, 72, 0, 0, 0, len(data), len(data), 1)
    return bytes(data)
