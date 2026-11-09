#!/usr/bin/env python3
"""Exercise the actual IFC verifier and A09 audit in a disposable container only."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

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
build = Path('/tmp/archy-elf-probe-build')
build.mkdir()
(build / 'marker.c').write_text('#include <sys/stat.h>\n#include <unistd.h>\nint main(void) { mkdir("/tmp/archy-untrusted-bwrap-ran", 0700); write(1, "{}", 2); return 0; }\n')
subprocess.run(['/usr/bin/cc', '-o', str(bwrap), str(build / 'marker.c')], check=True)
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
system = Path('/usr/lib/libarchy-recovery-system-fixture.so')
(build / 'dep.c').write_text('int recovery_dep(void) { return 1; }\n')
(build / 'module.c').write_text('extern int recovery_dep(void); int recovery_module(void) { return recovery_dep(); }\n')
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-Wl,-soname,libarchy-recovery-system-fixture.so', '-o', str(system), str(build / 'dep.c')], check=True)
module = runtime / 'python/lib/python3.12/site-packages/recovery.so'
module.parent.mkdir(parents=True)
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-o', str(module), str(build / 'module.c'), '-L/usr/lib', '-larchy-recovery-system-fixture'], check=True)


def row(path):
    return {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'mode': path.stat().st_mode & 0o7777}


files = [row(path) for path in sorted(runtime.rglob('*')) if path.is_file()]
with AnchoredRoot('/') as stage:
    systems = system_closure(stage)
(runtime / 'inventory.json').write_text(json.dumps({'schema_version': 2, 'files': files,
                                                    'system_files': systems}))

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
caller('valid_inventory_reaches_fixture_bwrap')
assert marker.exists()
marker.rmdir()
print('control: matching inventory reached synthetic bwrap')
for label, path in (('node', node), ('bwrap', bwrap), ('listed system dependency', system)):
    original = path.read_bytes()
    path.write_bytes(original + b'\nchanged\n')
    run([str(gate), '--audit-runtime=ifc'], expect_success=False)
    caller('invalid_inventory_stops_verifier_before_bwrap')
    assert not marker.exists(), label
    path.write_bytes(original)
    print('refused tampered', label)
run([str(gate), '--audit-runtime=ifc'])
inventory_path = runtime / 'inventory.json'
original_inventory = inventory_path.read_bytes()
omitted = json.loads(original_inventory)
omitted['system_files'] = []
inventory_path.write_text(json.dumps(omitted))
run([str(gate), '--audit-runtime=ifc'], expect_success=False)
caller('invalid_inventory_stops_verifier_before_bwrap')
assert not marker.exists()
inventory_path.write_bytes(original_inventory)
print('refused empty system_files before verifier execution')
inventory_path.write_bytes(b'{invalid')
caller('invalid_inventory_stops_verifier_before_bwrap')
assert not marker.exists()
inventory_path.write_bytes(original_inventory)
print('refused malformed inventory')
inventory_path.chmod(0o666)
caller('invalid_inventory_stops_verifier_before_bwrap')
assert not marker.exists()
inventory_path.chmod(0o644)
print('refused writable inventory')
inventory_path.rename(inventory_path.with_name('held-inventory'))
caller('invalid_inventory_stops_verifier_before_bwrap')
assert not marker.exists()
print('refused missing inventory')
inventory_path.with_name('held-inventory').rename(inventory_path)
for mode in (0o775, 0o644):
    gate.chmod(mode)
    caller('invalid_inventory_stops_verifier_before_bwrap')
    assert not marker.exists()
    print('refused gate mode', oct(mode))
gate.chmod(0o755)
os.chown(gate, 65534, 65534)
caller('invalid_inventory_stops_verifier_before_bwrap')
assert not marker.exists()
os.chown(gate, 0, 0)
print('refused non-root gate owner')
gate_parent = gate.parent
gate_parent.chmod(0o775)
caller('invalid_inventory_stops_verifier_before_bwrap')
assert not marker.exists()
gate_parent.chmod(0o755)
print('refused writable gate parent')
held = gate.with_name('held-gate')
gate.rename(held)
gate.symlink_to(held.name)
caller('invalid_inventory_stops_verifier_before_bwrap')
assert not marker.exists()
gate.unlink()
print('refused gate symlink')
caller('invalid_inventory_stops_verifier_before_bwrap')
assert not marker.exists()
print('refused missing gate')
print('PASS: actual caller and gate refuse altered runtime before verifier execution')
