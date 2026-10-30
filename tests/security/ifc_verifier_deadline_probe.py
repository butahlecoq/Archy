#!/usr/bin/env python3
"""Run the actual verifier with a descendant-held pipe in a disposable container."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

if os.geteuid() != 0 or not Path('/.dockerenv').exists():
    raise SystemExit('requires disposable root container')

gate_input, test_binary, verifier_input, web_input, source = map(Path, sys.argv[1:])
sys.path.insert(0, str(source / 'packaging/arch'))
from runtime_manifest import AnchoredRoot
from runtime import system_closure
runtime = Path('/usr/lib/archy/runtime')
runtime.mkdir(parents=True, exist_ok=False)
gate = Path('/usr/lib/archy/archy-service-gate')
shutil.copyfile(gate_input, gate)
gate.chmod(0o755)
marker = Path('/tmp/archy-untrusted-bwrap-ran')
required = ('blender/blender', 'workers/blender/worker.py', 'python/bin/python3.12',
            'workers/ifc/archy_ifc_worker/worker.py')
for name in required:
    path = runtime / name
    path.parent.mkdir(parents=True, exist_ok=True)
    if name in ('blender/blender', 'python/bin/python3.12'):
        shutil.copyfile('/usr/bin/python3.14', path)
        path.chmod(0o755)
    else:
        path.write_bytes(name.encode())
        path.chmod(0o644)
node = runtime / 'node/bin/node'
node.parent.mkdir(parents=True)
shutil.copyfile('/usr/bin/python3.14', node)
node.chmod(0o755)
bwrap = runtime / 'bin/bwrap'
bwrap.parent.mkdir(parents=True)
build = Path('/tmp/archy-deadline-build')
build.mkdir()
source_file = build / 'hold-pipe.c'
source_file.write_text('#include <sys/stat.h>\n#include <unistd.h>\n#include <stdlib.h>\n'
                       'int main(void) { mkdir("/tmp/archy-untrusted-bwrap-ran", 0700); '
                       'pid_t pid = fork(); if (pid < 0) return 2; '
                       'if (pid == 0) { sleep(23); _exit(0); } '
                       'write(1, "{}", 2); return 0; }\n')
subprocess.run(['/usr/bin/cc', '-o', str(bwrap), str(source_file)], check=True)
bwrap.chmod(0o755)
for name in ('reader.cjs', 'step-scope.cjs', 'verify-stdin.cjs'):
    target = runtime / 'ifc-verifier' / name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(verifier_input / name, target)
    target.chmod(0o644)
for name in ('web-ifc-api-node.js', 'web-ifc-node.wasm', 'package.json'):
    target = runtime / 'ifc-verifier/node_modules/web-ifc' / name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(web_input / name, target)
    target.chmod(0o644)


def row(path):
    return {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'mode': path.stat().st_mode & 0o7777}


files = [row(path) for path in sorted(runtime.rglob('*')) if path.is_file()]
with AnchoredRoot('/') as stage:
    system = system_closure(stage)
(runtime / 'inventory.json').write_text(json.dumps({'schema_version': 2, 'files': files,
                                                    'system_files': system}))

def run(command, expect_success=True):
    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
    if (result.returncode == 0) != expect_success:
        raise AssertionError(f'{command}: {result.returncode}\n{result.stdout}\n{result.stderr}')
    return result


def caller(test):
    result = run(['/usr/sbin/runuser', '-u', 'nobody', '--', str(test_binary), '--ignored', '--exact',
                  'ifc_verifier::runtime_trust_tests::' + test, '--nocapture'])
    if '1 passed' not in result.stdout:
        raise AssertionError(result.stdout)


run([str(gate), '--audit-runtime=ifc'])
started = time.monotonic()
caller('verifier_deadline_covers_descendant_held_pipe')
elapsed = time.monotonic() - started
assert marker.exists(), 'fixture bwrap was not reached'
print(f'verifier returned after {elapsed:.2f}s', flush=True)
assert 19 <= elapsed < 30, 'verifier deadline not enforced'
print("PASS: verifier deadline covers the descendant-held pipe")
