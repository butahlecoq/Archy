#!/usr/bin/env python3
"""Run only in a disposable root Arch container after pacman -U of the runtime."""
import json
import os
from pathlib import Path
import pwd
import shutil
import struct
import subprocess
import sys

if os.geteuid() != 0 or not Path('/.dockerenv').exists():
    raise SystemExit('requires disposable root container')

runtime = Path('/usr/lib/archy/runtime')
gate = Path('/usr/lib/archy/archy-service-gate')
shutil.copyfile(sys.argv[1], gate)
gate.chmod(0o755)

if sys.argv[2:] == ['--tamper-only']:
    before = subprocess.run([str(gate), '--audit-runtime=ifc'], capture_output=True, timeout=300)
    assert before.returncode == 0, before.stderr
    worker = runtime / 'workers/ifc/archy_ifc_worker/worker.py'
    worker.write_bytes(worker.read_bytes() + b'\n# tampered\n')
    after = subprocess.run([str(gate), '--audit-runtime=ifc'], capture_output=True, timeout=300)
    assert after.returncode != 0 and b'digest' in after.stderr, after.stderr
    print('installed runtime tamper audit: intact accepted; changed worker refused')
    raise SystemExit(0)


def run(*args, input=None):
    result = subprocess.run(args, input=input, capture_output=True, timeout=120,
                            env={**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'})
    if result.returncode:
        raise AssertionError(f'{args[0]} exit {result.returncode}: {result.stderr[:400]!r} {result.stdout[:400]!r}')
    return result.stdout

python = runtime / 'python/bin/python3.12'
node = runtime / 'node/bin/node'
blender = runtime / 'blender/blender'
run('pacman', '-Qkk', 'archy-runtime')
for backend in ('blender', 'ifc'):
    run(str(gate), '--audit-runtime=' + backend)
installed = subprocess.check_output(['pacman', '-Qi', 'archy-runtime'], text=True).splitlines()
depends_index = next(i for i, line in enumerate(installed) if line.startswith('Depends On'))
depends = [installed[depends_index].partition(':')[2]]
for line in installed[depends_index + 1:]:
    if line and not line[0].isspace():
        break
    depends.append(line)
declared = set(' '.join(depends).split())
inventory = json.loads((runtime / 'inventory.json').read_text())
owners = {subprocess.check_output(['pacman', '-Qqo', entry['path']], text=True).strip()
          for entry in inventory['system_files']}
assert owners <= declared, f'undeclared linked system libraries: {sorted(owners - declared)}'
assert b'3.12.16' in run(str(python), '-B', '-I', '-c',
    'import sys,_sqlite3,sqlite3,ifcopenshell,numpy,shapely,archy_ifc_worker; print(sys.version,sqlite3.sqlite_version,ifcopenshell.version,numpy.__version__,shapely.__version__)')
assert run(str(node), '--version').strip() == b'v22.22.2'
assert b'Blender 4.5.15' in run(str(blender), '--version')
assert b'bubblewrap 0.13.1' in run(str(runtime / 'bin/bwrap'), '--version')

project = Path('/tmp/archy-runtime-ifc-probe')
(project / 'runs/run-1').mkdir(parents=True)
request = project / 'request.json'
request.write_bytes(Path('/ifc-request.json').read_bytes())
nobody = pwd.getpwnam('nobody')
for path in (project, project / 'runs', project / 'runs/run-1', request):
    os.chown(path, nobody.pw_uid, nobody.pw_gid)
run('/usr/sbin/runuser', '-u', 'nobody', '--', str(python), '-B', '-I',
    '-m', 'archy_ifc_worker', '--project-root', str(project), '--request', str(request))
output = project / 'runs/run-1/ifc'
response = json.loads((output / 'response.json').read_text())
assert response['status'] == 'ok', response
assert response['validation']['fresh_process_reopen'] is True
assert response['validation']['express_validation']['exit_code'] == 0
assert 'No SQL support' not in json.dumps(response['validation'])
assert (output / 'model.ifc').is_file()
run('/usr/sbin/runuser', '-u', 'nobody', '--', 'python3', '-B', '/blender-probe.py')
for backend in ('blender', 'ifc'):
    run(str(gate), '--audit-runtime=' + backend)

fixture = Path('/fixture')
expected = next(case for case in json.loads((fixture / 'expected.json').read_text())['cases']
                if case['file'] == 'rectangle.ifc')
source = (fixture / 'source/rectangle.json').read_bytes()
model = (fixture / 'rectangle.ifc').read_bytes()
bridge = runtime / 'ifc-verifier/verify-stdin.cjs'

def frame(data):
    parts = [json.dumps(expected, separators=(',', ':')).encode(), source, data]
    return b''.join(struct.pack('>I', len(part)) for part in parts) + b''.join(parts)

report = json.loads(run(str(node), str(bridge), input=frame(model)))
assert report['status'] == 'ok' and report['engine'] == 'web-ifc'
bad = subprocess.run([str(node), str(bridge)], input=frame(model + b'<script>'),
                     capture_output=True, timeout=30)
assert bad.returncode != 0 and json.loads(bad.stdout)['code'] == 'ARTIFACT_INVALID'
worker = runtime / 'workers/ifc/archy_ifc_worker/worker.py'
worker.write_bytes(worker.read_bytes() + b'\n# tampered\n')
bad = subprocess.run([str(gate), '--audit-runtime=ifc'], capture_output=True, timeout=300)
assert bad.returncode != 0 and b'digest' in bad.stderr
print('installed runtime: pacman integrity, dependency closure, Python/Blender/Node/bwrap, native IFC worker and reader, positive gate audits, tamper refusal')
