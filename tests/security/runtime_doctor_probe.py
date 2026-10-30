#!/usr/bin/env python3
"""Check package ownership diagnostics in a disposable root Arch container."""
import json
import os
from pathlib import Path
import subprocess
import sys

if os.geteuid() != 0 or not Path('/.dockerenv').exists():
    raise SystemExit('requires disposable root container')


def pacman(*args):
    result = subprocess.run(['/usr/bin/pacman', *args], capture_output=True, text=True, timeout=600)
    if result.returncode:
        raise SystemExit(f'pacman failed: {result.stderr[-500:]}')


def doctor():
    result = subprocess.run([sys.argv[1], 'doctor', '--json'], capture_output=True, text=True,
                            timeout=30, env={'PATH': '/untrusted/path'})
    if result.returncode not in (0, 1):
        raise SystemExit(f'doctor failed: {result.stderr[-500:]}')
    report = json.loads(result.stdout)
    return next(check for check in report['checks'] if check['id'] == 'tools.bwrap')


pacman('-U', '--noconfirm', '/bubblewrap.pkg.tar.zst')
pacman('-Q', 'bubblewrap')
before = doctor()
assert before['status'] == 'missing', before
pacman('-U', '--noconfirm', '/runtime.pkg.tar.zst')
installed = doctor()
assert installed['status'] == 'deferred' and 'ownership observed' in installed['reason'], installed
assert 'archy-runtime' in str(installed['evidence']), installed
path = Path('/usr/lib/archy/runtime/bin/bwrap')
path.chmod(0o775)
unsafe = doctor()
assert unsafe['status'] == 'unavailable' and 'immutable' in unsafe['reason'], unsafe
print('doctor: system bubblewrap alone missing; private package deferred; mutable private path refused')
