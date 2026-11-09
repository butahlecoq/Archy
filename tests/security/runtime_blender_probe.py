#!/usr/bin/env python3
"""Exercise the packaged Blender worker at its final path in a disposable Arch image."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess

if not Path('/.dockerenv').exists():
    raise SystemExit('requires disposable container')

job = Path('/tmp/archy-runtime-blender-probe')
job.mkdir()
request = job / 'request.json'
shutil.copyfile('/blender-request.json', request)
response = job / 'response.json'
command = [
    '/usr/lib/archy/runtime/blender/blender', '--background', '--factory-startup',
    '--disable-autoexec', '--offline-mode', '--python-exit-code', '20', '--threads', '1',
    '--python', '/usr/lib/archy/runtime/workers/blender/worker.py', '--',
    '--request', str(request), '--response', str(response),
]
result = subprocess.run(command, cwd=job, capture_output=True, text=True, timeout=240,
                        env={**os.environ, 'HOME': '/tmp'})
if result.returncode:
    raise AssertionError(f'Blender exit {result.returncode}: {result.stderr[-1000:]} {result.stdout[-1000:]}')
report = json.loads(response.read_text())
assert report['status'] == 'ok', report
assert {item['path'] for item in report['artifacts']} == {
    'scene.blend', 'model.glb', 'view-top.png', 'view-front.png', 'view-iso.png'
}
for item in report['artifacts']:
    data = (job / item['path']).read_bytes()
    assert len(data) == item['size_bytes']
    assert hashlib.sha256(data).hexdigest() == item['sha256']
assert (job / 'model.glb').read_bytes()[:4] == b'glTF'
for view in ('top', 'front', 'iso'):
    data = (job / f'view-{view}.png').read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    assert struct.unpack('>II', data[16:24]) == (512, 512)
print('final-path CPU Blender worker: GLB and three PNGs with matching artifact digests')
