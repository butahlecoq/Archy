#!/usr/bin/env python3
"""Read historical system-library evidence from the pinned disposable Arch image.

Run inside the image with `python3 - /evidence/inventory.json < this_file`;
no package or host changes.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

NAMES = (
    'libSM.so.6', 'libICE.so.6', 'libX11.so.6', 'libxcb.so.1',
    'libXau.so.6', 'libXdmcp.so.6', 'libuuid.so.1', 'libc.so.6',
    'ld-linux-x86-64.so.2',
)


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def brackets(output, tag):
    return sorted(set(re.findall(r'\(' + tag + r'\).*?\[([^]]+)\]', output)))


inventory = None
if len(sys.argv) == 2:
    inventory_bytes = Path(sys.argv[1]).read_bytes()
    inventory = json.loads(inventory_bytes)
    expected = {row['path']: row for row in inventory['system_files']}
    if len(expected) != len(inventory['system_files']):
        raise ValueError('duplicate historical system path')
    aliases = [Path(path) for path in sorted(expected)]
else:
    aliases = [Path('/usr/lib') / name for name in NAMES]

rows = []
for alias in aliases:
    resolved = alias.resolve(strict=True)
    if not resolved.is_relative_to('/usr/lib') or not resolved.is_file():
        raise ValueError(f'unexpected system path: {alias}')
    data = resolved.read_bytes()
    dynamic = command('/usr/bin/readelf', '-dW', str(resolved))
    versions = command('/usr/bin/readelf', '--version-info', '-W', str(resolved))
    owner = command('/usr/bin/pacman', '-Qoq', str(resolved))
    package = command('/usr/bin/pacman', '-Q', owner)
    requirements = {}
    current = None
    for line in versions.splitlines():
        match = re.search(r'\bFile: (\S+)', line)
        if match:
            current = match.group(1)
            requirements[current] = []
        match = re.search(r'\bName: (\S+)', line)
        if match and current is not None:
            requirements[current].append(match.group(1))
    requirements = {name: sorted(set(values)) for name, values in requirements.items()}
    digest = hashlib.sha256(data).hexdigest()
    mode = oct(resolved.stat().st_mode & 0o777)
    if inventory is not None:
        original = expected[str(alias)]
        if resolved != alias or digest != original['sha256'] or int(mode, 8) != original['mode']:
            raise ValueError(f'historical inventory differs from image: {alias}')
    rows.append({
        'alias': str(alias), 'resolved_path': str(resolved),
        'sha256': digest, 'mode': mode,
        'owner': owner, 'package_version': package.removeprefix(owner + ' '),
        'soname': brackets(dynamic, 'SONAME'), 'needed': brackets(dynamic, 'NEEDED'),
        'rpath': brackets(dynamic, 'RPATH'), 'runpath': brackets(dynamic, 'RUNPATH'),
        'version_requirements': requirements,
    })
result = {'schema_version': 1, 'scope': 'historical system_files' if inventory else 'bounded X11 chain, libc and loader', 'libraries': rows}
if inventory is not None:
    result['historical_inventory_sha256'] = hashlib.sha256(inventory_bytes).hexdigest()
    result['historical_system_file_count'] = len(expected)
print(json.dumps(result, indent=2, sort_keys=True))
