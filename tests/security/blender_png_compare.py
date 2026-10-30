#!/usr/bin/env python3
"""Compare two 8-bit RGBA PNGs by decoded pixels, independent of metadata."""
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib


def decode(path):
    data = Path(path).read_bytes()
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('bad PNG signature')
    offset = 8
    chunks = []
    compressed = bytearray()
    dimensions = None
    finished = False
    while offset < len(data):
        length, kind = struct.unpack_from('>I4s', data, offset)
        start = offset + 8
        payload = data[start:start + length]
        crc = struct.unpack_from('>I', data, start + length)[0]
        if zlib.crc32(kind + payload) & 0xffffffff != crc:
            raise ValueError('bad PNG CRC')
        chunks.append(kind.decode('ascii'))
        if kind == b'IHDR':
            width, height, depth, color, compression, filtering, interlace = struct.unpack('>IIBBBBB', payload)
            if (depth, color, compression, filtering, interlace) != (8, 6, 0, 0, 0):
                raise ValueError('expected noninterlaced 8-bit RGBA PNG')
            dimensions = (width, height)
        elif kind == b'IDAT':
            compressed.extend(payload)
        elif kind == b'IEND':
            finished = True
            if start + length + 4 != len(data):
                raise ValueError('trailing PNG bytes')
        offset = start + length + 4
    if not finished or dimensions is None:
        raise ValueError('missing PNG chunks')
    width, height = dimensions
    encoded = zlib.decompress(compressed)
    stride = width * 4
    if len(encoded) != (stride + 1) * height:
        raise ValueError('unexpected PNG scanline bytes')
    prior = bytearray(stride)
    pixels = bytearray()
    for y in range(height):
        start = y * (stride + 1)
        mode = encoded[start]
        raw = encoded[start + 1:start + 1 + stride]
        row = bytearray(stride)
        for x, value in enumerate(raw):
            left = row[x-4] if x >= 4 else 0
            up = prior[x]
            upper_left = prior[x-4] if x >= 4 else 0
            if mode == 0: predictor = 0
            elif mode == 1: predictor = left
            elif mode == 2: predictor = up
            elif mode == 3: predictor = (left + up) // 2
            elif mode == 4:
                p = left + up - upper_left
                distances = (abs(p-left), abs(p-up), abs(p-upper_left))
                predictor = (left, up, upper_left)[distances.index(min(distances))]
            else: raise ValueError('invalid PNG filter')
            row[x] = (value + predictor) & 255
        pixels.extend(row)
        prior = row
    return dimensions, pixels, chunks, hashlib.sha256(data).hexdigest()


def main():
    left, right = map(Path, sys.argv[1:])
    ld, lp, lc, lh = decode(left)
    rd, rp, rc, rh = decode(right)
    if ld != rd:
        raise ValueError('dimensions differ')
    deltas = [abs(a-b) for a,b in zip(lp, rp)]
    print(json.dumps({'dimensions':ld, 'maximum_channel_difference':max(deltas, default=0),
                      'changed_channels':sum(x != 0 for x in deltas),
                      'left_png_sha256':lh, 'right_png_sha256':rh,
                      'left_chunks':lc, 'right_chunks':rc}, sort_keys=True))
    if any(deltas):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
