#!/usr/bin/env python3
"""Exercise ELF system dependency coverage in a disposable Arch container."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import struct
import sys

if os.geteuid() != 0 or not Path('/.dockerenv').exists():
    raise SystemExit('requires disposable root container')
root = Path('/usr/lib/archy/runtime')
root.mkdir(parents=True, exist_ok=False)
gate = root.parent / 'archy-service-gate'
shutil.copyfile(sys.argv[1], gate)
gate.chmod(0o755)
loader = Path('/usr/lib/ld-linux-x86-64.so.2')
entry_names = ('bin/bwrap', 'blender/blender', 'python/bin/python3.12', 'node/bin/node')
for name in entry_names:
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile('/usr/bin/python3.14', path)
    path.chmod(0o755)
for name in ('workers/blender/worker.py', 'workers/ifc/archy_ifc_worker/worker.py',
             'ifc-verifier/reader.cjs', 'ifc-verifier/verify-stdin.cjs',
             'ifc-verifier/step-scope.cjs', 'ifc-verifier/node_modules/web-ifc/web-ifc-api-node.js',
             'ifc-verifier/node_modules/web-ifc/web-ifc-node.wasm',
             'ifc-verifier/node_modules/web-ifc/package.json'):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b'fixture\n')
    path.chmod(0o644)

work = Path('/tmp/elf-coverage')
work.mkdir()
(work / 'dep.c').write_text('int archy_dependency(void) { return 9; }\n')
(work / 'module.c').write_text('extern int archy_dependency(void); int archy_module(void) { return archy_dependency(); }\n')
dep = Path('/usr/lib/libarchy-coverage-probe.so')
subprocess.run(['/usr/bin/cc', '-fPIC', '-shared', '-Wl,-soname,libarchy-coverage-probe.so',
                '-o', str(dep), str(work / 'dep.c')], check=True)
module = root / 'python/lib/python3.12/site-packages/coverage.so'
module.parent.mkdir(parents=True)
subprocess.run(['/usr/bin/cc', '-fPIC', '-shared', '-o', str(module), str(work / 'module.c'),
                '-L/usr/lib', '-larchy-coverage-probe'], check=True)
module.chmod(0o755)
# A second real module uses an $ORIGIN private library as Blender does.
local_dep = module.parent / 'libarchy-origin-probe.so'
subprocess.run(['/usr/bin/cc', '-fPIC', '-shared', '-Wl,-soname,libarchy-origin-probe.so',
                '-o', str(local_dep), str(work / 'dep.c')], check=True)
origin_module = module.parent / 'origin.so'
subprocess.run(['/usr/bin/cc', '-fPIC', '-shared', '-o', str(origin_module),
                str(work / 'module.c'), '-L' + str(module.parent), '-larchy-origin-probe',
                '-Wl,-rpath,$ORIGIN'], check=True)

# A sibling extension is eligible through its own explicit RUNPATH.
preloaded = module.parent / 'libarchy-preloaded-probe.so'
subprocess.run(['/usr/bin/cc', '-fPIC', '-shared', '-Wl,-soname,libarchy-preloaded-probe.so',
                '-o', str(preloaded), str(work / 'dep.c')], check=True)
preload_user = module.parent / 'preloaded-user.so'
subprocess.run(['/usr/bin/cc', '-fPIC', '-shared', '-o', str(preload_user),
                str(work / 'module.c'), '-L' + str(module.parent), '-larchy-preloaded-probe',
                '-Wl,-rpath,$ORIGIN'], check=True)

def row(path):
    return {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'mode': path.stat().st_mode & 0o7777}

libraries = {loader.resolve()}
for executable in [*(root / n for n in entry_names), module, origin_module]:
    result = subprocess.run([str(loader), '--list', '--inhibit-cache', str(executable)],
                            capture_output=True, text=True, check=True)
    for line in result.stdout.splitlines():
        words = line.split()
        for word in words:
            if word.startswith('/'):
                path = Path(word).resolve(strict=True)
                if not path.is_relative_to(root):
                    libraries.add(path)
files = [row(p) for p in sorted(root.rglob('*')) if p.is_file()]
system = [row(p) for p in sorted(libraries)]
if len(sys.argv) > 2:
    sys.path.insert(0, str(Path(sys.argv[2]) / 'packaging/arch'))
    from runtime_manifest import AnchoredRoot
    from runtime import system_closure
    with AnchoredRoot('/') as stage:
        generated = system_closure(stage)
    assert {str(loader), str(dep)} <= {r['path'] for r in generated}
    assert {r['path'] for r in system} <= {r['path'] for r in generated}
    system = generated
    print('static generator covered real ELF closure', flush=True)
    entry = root / 'blender/blender'
    held = work / 'held-blender'
    entry.rename(held)
    try:
        with AnchoredRoot('/') as stage:
            try:
                system_closure(stage)
            except ValueError as error:
                assert 'entry point missing' in str(error), str(error)
            else:
                raise AssertionError('generator accepted missing fixed entry point')
    finally:
        held.rename(entry)
    print('static generator rejected missing fixed entry point', flush=True)
inventory = root / 'inventory.json'

def audit(rows, expected, label):
    inventory.write_text(json.dumps({'schema_version': 2, 'files': files, 'system_files': rows}))
    result = subprocess.run([str(gate), '--audit-runtime=ifc'], capture_output=True, text=True)
    if (result.returncode == 0) != expected:
        raise AssertionError(f'{label}: expected success={expected}, got {result.returncode}: {result.stderr}')
    print(label, 'accepted' if expected else 'rejected', flush=True)
    return result.stderr

audit(system, True, 'matching real ELF closure')
audit([], False, 'empty system inventory')
audit([r for r in system if r['path'] != str(dep)], False, 'omitted extension dependency with nonempty inventory')
audit([r for r in system if r['path'] != str(loader)], False, 'omitted dynamic loader')
python_lib = next(r['path'] for r in system if Path(r['path']).name.startswith('libpython3.14.so'))
audit([r for r in system if r['path'] != python_lib], False, 'omitted bwrap prelaunch dependency')

hwcaps = Path('/usr/lib/glibc-hwcaps/test-archy')
hwcaps.mkdir(parents=True)
alternate = hwcaps / dep.name
shutil.copyfile(dep, alternate)
audit(system, False, 'uninventoried hwcaps alternative')
audit(sorted(system + [row(alternate)], key=lambda r: r['path']), True, 'inventoried inactive hwcaps plus baseline')
# A made-up level is ignored by the real loader. It cannot be the only
# candidate that makes an otherwise missing DT_NEEDED count as resolved.
dep.rename(work / 'held-default-dep')
try:
    inactive_rows = sorted([r for r in system if r['path'] != str(dep)] + [row(alternate)], key=lambda r: r['path'])
    loader_check = subprocess.run([str(loader), '--list', '--inhibit-cache', str(module)],
                                  capture_output=True, text=True)
    assert loader_check.returncode != 0 and dep.name in loader_check.stderr
    if len(sys.argv) > 3 and sys.argv[3] == 'hwcaps-generator':
        with AnchoredRoot('/') as stage:
            try:
                system_closure(stage)
            except ValueError as error:
                assert dep.name in str(error) and 'ordinary search directories' in str(error), str(error)
            else:
                raise AssertionError('generator accepted inactive hwcaps-only dependency')
        print('static generator rejected inactive hwcaps-only dependency', flush=True)
    else:
        error = audit(inactive_rows, False, 'inactive hwcaps-only dependency')
        assert 'ordinary search directories' in error and dep.name in error, error
finally:
    (work / 'held-default-dep').rename(dep)
alternate.unlink()
hwcaps.rmdir()
hwcaps.parent.rmdir()

preload = Path('/etc/ld.so.preload')
assert not preload.exists()
preload.write_bytes(b'')
audit(system, False, 'loader preload file')
preload.unlink()

# A library in an unrelated directory is not a valid search path.
unrelated = root / 'blender/unrelated'
unrelated.mkdir(parents=True)
extra_lib = unrelated / 'libarchy-unrelated-probe.so'
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-Wl,-soname,libarchy-unrelated-probe.so',
                '-o', str(extra_lib), str(work / 'dep.c')], check=True)
bad_module = module.parent / 'unrelated-user.so'
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-o', str(bad_module), str(work / 'module.c'),
                '-L' + str(unrelated), '-larchy-unrelated-probe'], check=True)
loader_check = subprocess.run([str(loader), '--list', '--inhibit-cache', str(bad_module)],
                              capture_output=True, text=True)
assert loader_check.returncode != 0 and 'libarchy-unrelated-probe.so' in loader_check.stderr
files.extend((row(extra_lib), row(bad_module)))
files.sort(key=lambda x: x['path'])
if len(sys.argv) > 3 and sys.argv[3] == 'generator-first':
    with AnchoredRoot('/') as stage:
        try:
            system_closure(stage)
        except ValueError as error:
            assert 'libarchy-unrelated-probe.so' in str(error), str(error)
        else:
            raise AssertionError('generator accepted unrelated search directory')
    print('static generator rejected unrelated search directory', flush=True)
else:
    audit(system, False, 'unrelated search directory')
bad_module.unlink()
extra_lib.unlink()
unrelated.rmdir()
files = [r for r in files if r['path'] not in (str(bad_module), str(extra_lib))]

# NODEFLIB excludes default library directories even when a matching file exists.
nodefault = module.parent / 'nodefault.so'
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-Wl,-z,nodefaultlib', '-o', str(nodefault),
                str(work / 'module.c'), '-L/usr/lib', '-larchy-coverage-probe'], check=True)
loader_check = subprocess.run([str(loader), '--list', '--inhibit-cache', str(nodefault)],
                              capture_output=True, text=True)
assert loader_check.returncode != 0 and 'libarchy-coverage-probe.so' in loader_check.stderr
files.append(row(nodefault))
files.sort(key=lambda x: x['path'])
if len(sys.argv) > 3 and sys.argv[3] == 'nodefault-generator':
    with AnchoredRoot('/') as stage:
        try:
            system_closure(stage)
        except ValueError as error:
            assert 'NODEFLIB' in str(error), str(error)
        else:
            raise AssertionError('generator accepted NODEFLIB without modeling its search')
    print('static generator rejected unsupported NODEFLIB', flush=True)
else:
    error = audit(system, False, 'unsupported NODEFLIB')
    assert 'NODEFLIB' in error, error
nodefault.unlink()
files = [r for r in files if r['path'] != str(nodefault)]

# glibc can inherit DT_RPATH from the loader's parent; validate even the
# alternate system library that can override the requester's default.
parent_dir = Path('/usr/lib/archy-parent-rpath-fixture')
parent_dir.mkdir()
parent_base = Path('/usr/lib/libarchy-parent-probe.so')
parent_alternate = parent_dir / parent_base.name
for lib in (parent_base, parent_alternate):
    subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-Wl,-soname,libarchy-parent-probe.so',
                    '-o', str(lib), str(work / 'dep.c')], check=True)
child = module.parent / 'parent-child.so'
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-Wl,-soname,parent-child.so',
                '-o', str(child), str(work / 'module.c'), '-L/usr/lib', '-larchy-parent-probe'], check=True)
parent = module.parent / 'parent-rpath.so'
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-Wl,--disable-new-dtags',
                '-Wl,-rpath,$ORIGIN:/usr/lib/archy-parent-rpath-fixture',
                '-o', str(parent), str(work / 'module.c'), '-L' + str(module.parent),
                '-l:parent-child.so'], check=True)
listing = subprocess.run([str(loader), '--list', '--inhibit-cache', str(parent)],
                         capture_output=True, text=True, check=True)
assert str(parent_alternate) in listing.stdout, listing.stdout
files.extend((row(child), row(parent)))
files.sort(key=lambda x: x['path'])
parent_system = sorted(system + [row(parent_base)], key=lambda r: r['path'])
audit(parent_system, False, 'unlisted inherited RPATH alternative')
with AnchoredRoot('/') as stage:
    generated_with_parent = system_closure(stage)
assert str(parent_alternate) in {r['path'] for r in generated_with_parent}
parent_system.append(row(parent_alternate))
parent_system.sort(key=lambda r: r['path'])
audit(parent_system, True, 'listed inherited RPATH alternative')
held_parent_base = work / 'held-parent-base'
parent_base.rename(held_parent_base)
try:
    listing = subprocess.run([str(loader), '--list', '--inhibit-cache', str(parent)],
                             capture_output=True, text=True, check=True)
    assert str(parent_alternate) in listing.stdout
    without_base = [r for r in parent_system if r['path'] != str(parent_base)]
    audit(without_base, False, 'ancestor-only resolution conservatively refused')
    with AnchoredRoot('/') as stage:
        try:
            system_closure(stage)
        except ValueError as error:
            assert 'ordinary search directories' in str(error), str(error)
        else:
            raise AssertionError('generator accepted ancestor-only resolution as universal')
finally:
    held_parent_base.rename(parent_base)
# A requester RUNPATH prevents use of its parent's legacy RPATH in this case.
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-Wl,-soname,parent-child.so',
                '-Wl,-rpath,/usr/lib', '-o', str(child), str(work / 'module.c'),
                '-L/usr/lib', '-larchy-parent-probe'], check=True)
listing = subprocess.run([str(loader), '--list', '--inhibit-cache', str(parent)],
                         capture_output=True, text=True, check=True)
assert str(parent_alternate) not in listing.stdout
files = [r for r in files if r['path'] != str(child)] + [row(child)]
files.sort(key=lambda x: x['path'])
audit([r for r in parent_system if r['path'] != str(parent_alternate)], True,
      'requester RUNPATH excludes inherited RPATH')
child.unlink()
parent.unlink()
parent_base.unlink()
parent_alternate.unlink()
parent_dir.rmdir()
files = [r for r in files if r['path'] not in (str(child), str(parent))]

# Corrupt PT_DYNAMIC's virtual address while leaving its file offset intact.
malformed = module.parent / 'mapping-corrupt.so'
shutil.copyfile(module, malformed)
data = bytearray(malformed.read_bytes())
phoff = struct.unpack_from('<Q', data, 32)[0]
phsize, phnum = struct.unpack_from('<HH', data, 54)
assert phsize == 56
for i in range(phnum):
    off = phoff + i * phsize
    if struct.unpack_from('<I', data, off)[0] == 2:
        virtual = struct.unpack_from('<Q', data, off + 16)[0]
        struct.pack_into('<Q', data, off + 16, virtual + 1)
        break
else:
    raise AssertionError('fixture has no PT_DYNAMIC')
malformed.write_bytes(data)
files.append(row(malformed))
files.sort(key=lambda x: x['path'])
error = audit(system, False, 'mismatched PT_DYNAMIC mapping')
assert 'dynamic segment mapping' in error, error
malformed.unlink()
files = [r for r in files if r['path'] != str(malformed)]

(work / 'audit.c').write_text('#include <fcntl.h>\n#include <unistd.h>\n'
    '__attribute__((constructor)) static void init(void) { int f = open("/tmp/archy-audit-constructor", O_CREAT|O_WRONLY, 0600); if (f >= 0) close(f); }\n'
    'int la_version(unsigned int x) { return x; }\n')
audit_lib = work / 'audit.so'
subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-o', str(audit_lib), str(work / 'audit.c')], check=True)
evil = root / 'python/lib/python3.12/site-packages/audit-entry'
(work / 'main.c').write_text('int main(void) { return 0; }\n')
subprocess.run(['/usr/bin/cc', '-o', str(evil), str(work / 'main.c'),
                '-Wl,-audit,' + str(audit_lib)], check=True)
files.append(row(evil))
files.sort(key=lambda x: x['path'])
audit(system, False, 'ELF audit hook')
assert not Path('/tmp/archy-audit-constructor').exists(), 'audit executed untrusted ELF code'
if len(sys.argv) > 2:
    with AnchoredRoot('/') as stage:
        try:
            system_closure(stage)
        except ValueError as error:
            assert 'unsupported ELF audit' in str(error), str(error)
        else:
            raise AssertionError('generator accepted ELF audit hook')
    assert not Path('/tmp/archy-audit-constructor').exists(), 'generator executed untrusted ELF code'
    print('static generator rejected ELF audit hook', flush=True)
print('PASS ELF closure probe')
