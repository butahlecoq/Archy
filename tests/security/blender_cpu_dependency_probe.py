#!/usr/bin/env python3
"""Observe pinned Blender CPU worker loads in a network-disabled disposable container."""
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import zlib

if not Path('/.dockerenv').exists() or os.geteuid() == 0:
    raise SystemExit('requires an unprivileged disposable container')
inputs, output = map(Path, sys.argv[1:])
output.mkdir(parents=True, exist_ok=False)
blender = Path('/usr/lib/archy/runtime/blender/blender')
worker = Path('/usr/lib/archy/runtime/workers/blender/worker.py')
assert not Path('/dev/dri').exists() and not Path('/dev/kfd').exists()
assert not Path('/etc/ld.so.cache').exists() and not Path('/etc/ld.so.preload').exists()
for name in ('libXt.so.6', 'libamdhip64.so.6'):
    for base in ('/usr/lib', '/usr/local/lib', '/opt'):
        if Path(base).exists():
            assert not any(name in files for _, _, files in os.walk(base)), (base, name)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_glb(path):
    data = path.read_bytes()
    assert len(data) >= 28
    magic, version, total = struct.unpack_from('<4sII', data)
    assert magic == b'glTF' and version == 2 and total == len(data)
    offset = 12
    chunks = []
    while offset < len(data):
        assert offset + 8 <= len(data)
        length, kind = struct.unpack_from('<I4s', data, offset)
        offset += 8
        assert length % 4 == 0 and offset + length <= len(data)
        chunks.append((kind, data[offset:offset + length]))
        offset += length
    assert offset == len(data) and len(chunks) >= 2 and chunks[0][0] == b'JSON' and chunks[1][0] == b'BIN\0'
    model = json.loads(chunks[0][1])
    assert model['asset']['version'] == '2.0' and model.get('meshes') and model.get('nodes')
    assert model['buffers'][0]['byteLength'] <= len(chunks[1][1])
    return len(model['meshes'])


def check_png(path):
    data = path.read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    offset = 8
    encoded = bytearray()
    width = height = None
    done = False
    while offset < len(data):
        assert offset + 12 <= len(data)
        length = struct.unpack_from('>I', data, offset)[0]
        kind = data[offset + 4:offset + 8]
        start = offset + 8
        end = start + length
        assert end + 4 <= len(data)
        payload = data[start:end]
        crc = struct.unpack_from('>I', data, end)[0]
        assert crc == zlib.crc32(kind + payload) & 0xffffffff
        if kind == b'IHDR':
            width, height = struct.unpack_from('>II', payload)
        elif kind == b'IDAT':
            encoded.extend(payload)
        elif kind == b'IEND':
            done = True
            assert end + 4 == len(data)
        offset = end + 4
    assert done and (width, height) == (512, 512) and len(zlib.decompress(encoded)) > 512 * 512


observed = []
for index in range(3):
    job = output / f'variant-{index}'
    job.mkdir()
    request = inputs / f'{index}.json'
    response = job / 'response.json'
    env = {'HOME': str(job), 'TMPDIR': str(job), 'LC_ALL': 'C', 'TZ': 'UTC',
           'LD_DEBUG': 'libs,files'}
    cmd = [str(blender), '--background', '--factory-startup',
           '--disable-autoexec', '--offline-mode', '--python-exit-code', '20', '--threads', '1',
           '--python', str(worker), '--', '--request', str(request), '--response', str(response)]
    with (job / 'stdout.log').open('wb') as out, (job / 'loader-stderr.log').open('wb') as err:
        result = subprocess.run(cmd, cwd=job, env=env, stdin=subprocess.DEVNULL,
                                stdout=out, stderr=err, timeout=300)
    assert result.returncode == 0, (index, result.returncode, (job / 'loader-stderr.log').read_text(errors='replace')[-3000:])
    data = json.loads(response.read_text())
    assert data['status'] == 'ok' and data['preset']['device'] == 'CPU'
    assert data['preset']['denoising'] is False
    expected = {'scene.blend', 'model.glb', 'view-top.png', 'view-front.png', 'view-iso.png'}
    assert {r['path'] for r in data['artifacts']} == expected
    for item in data['artifacts']:
        artifact = job / item['path']
        assert artifact.stat().st_size == item['size_bytes'] and digest(artifact) == item['sha256']
    meshes = check_glb(job / 'model.glb')
    for view in ('top', 'front', 'iso'):
        check_png(job / f'view-{view}.png')
    trace = (job / 'loader-stderr.log').read_text(errors='replace')
    inits = [line.strip() for line in trace.splitlines() if 'calling init:' in line]
    actual = [line for line in inits if 'libXt.so' in line or 'libamdhip64' in line or 'device_hip' in line]
    observed.append({'variant_id': data['variant_id'], 'meshes': meshes,
                     'loaded_matching_libraries': actual,
                     'matching_trace_lines': [line.strip() for line in trace.splitlines()
                                              if 'libXt.so' in line or 'libamdhip64' in line or 'device_hip' in line],
                     'artifacts': {item['path']: item['sha256'] for item in data['artifacts']}})
    print(f'variant {index}: CPU GLB and three PNGs validated', flush=True)
(output / 'observation.json').write_text(json.dumps({
    'blender_sha256': digest(blender), 'worker_sha256': digest(worker),
    'absent_system_libraries': ['libXt.so.6', 'libamdhip64.so.6'],
    'limitations': 'Direct CPU worker and loader tracing only; no gate/service, target, GPU, arbitrary dlopen or human proof.',
    'variants': observed,
}, indent=2, sort_keys=True) + '\n')
print('PASS: three direct CPU worker cases; inspect loader trace and limitations', flush=True)
