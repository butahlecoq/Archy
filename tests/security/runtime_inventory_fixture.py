#!/usr/bin/env python3
"""Run only as root in a disposable container; exercise real gate audit policy."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

if os.geteuid() != 0 or not Path('/.dockerenv').exists():
    raise SystemExit('requires disposable root container')

root = Path('/usr/lib/archy/runtime')
root.mkdir(parents=True, exist_ok=True)
sys.path.insert(0, str(Path(sys.argv[2]) / 'packaging/arch'))
from runtime_manifest import AnchoredRoot
from runtime import system_closure
gate = Path('/usr/lib/archy/archy-service-gate')
shutil.copyfile(sys.argv[1], gate)
gate.chmod(0o755)
required = [
    'bin/bwrap', 'blender/blender', 'workers/blender/worker.py',
    'python/bin/python3.12', 'workers/ifc/archy_ifc_worker/worker.py',
    'node/bin/node', 'ifc-verifier/reader.cjs', 'ifc-verifier/verify-stdin.cjs',
    'ifc-verifier/step-scope.cjs',
    'ifc-verifier/node_modules/web-ifc/web-ifc-api-node.js',
    'ifc-verifier/node_modules/web-ifc/web-ifc-node.wasm',
    'ifc-verifier/node_modules/web-ifc/package.json',
]
for name in required:
    target = root / name
    target.parent.mkdir(parents=True, exist_ok=True)
    if name in ('bin/bwrap', 'blender/blender', 'python/bin/python3.12', 'node/bin/node'):
        shutil.copyfile('/usr/bin/python3.14', target)
        target.chmod(0o755)
    else:
        target.write_bytes(name.encode())
        target.chmod(0o644)


def snapshot():
    rows = []
    for path in sorted(root.rglob('*')):
        if path.is_file() and path.name != 'inventory.json':
            rows.append({'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                         'mode': path.stat().st_mode & 0o7777})
    with AnchoredRoot('/') as stage:
        systems = system_closure(stage)
    (root / 'inventory.json').write_text(json.dumps({'schema_version': 2, 'files': rows, 'system_files': systems}))


def audit(backend, success):
    result = subprocess.run([str(gate), '--audit-runtime=' + backend], capture_output=True, text=True)
    if (result.returncode == 0) != success:
        raise AssertionError(f'{backend} expected success={success}: {result.returncode}: {result.stderr}')
    return result.stderr

snapshot()
audit('blender', True)
audit('ifc', True)
path = root / 'workers/ifc/archy_ifc_worker/worker.py'
path.write_bytes(b'tampered')
assert 'digest or mode mismatch' in audit('ifc', False)
snapshot()
path.chmod(0o666)
assert 'mutable runtime payload' in audit('ifc', False)
path.chmod(0o644)
snapshot()
(root / 'extra').write_bytes(b'not listed')
assert 'payload differs' in audit('blender', False)
(root / 'extra').unlink()
snapshot()
node = root / 'node/bin/node'
contents = node.read_bytes()
node.unlink()
node.symlink_to('../../bin/bwrap')
assert 'mutable runtime payload' in audit('ifc', False)
node.unlink()
node.write_bytes(contents)
node.chmod(0o755)
os.chown(node, 65534, 65534)
assert 'mutable runtime payload' in audit('ifc', False)
os.chown(node, 0, 0)
snapshot()
node.unlink()
audit('ifc', False)
print('runtime audit: positive blender/ifc; digest, mode, extra, symlink, owner, and missing refusals passed')
