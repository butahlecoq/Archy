"""Exercise real bind mounts in a disposable unprivileged mount namespace."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
from unittest.mock import patch

root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('runtime_manifest_mount', root / 'packaging/arch/runtime_manifest.py')
manifest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(manifest)
base = Path(sys.argv[1])
subprocess.run(['mount', '--make-rprivate', '/'], check=True)


def bind(source, target):
    subprocess.run(['mount', '--bind', str(source), str(target)], check=True)


def unmount(target):
    subprocess.run(['umount', str(target)], check=True)


def must_refuse(operation, label):
    try:
        operation()
    except (OSError, ValueError):
        return
    raise AssertionError(label + ' accepted a bind mount')


source = base / 'source'
parent = source / 'workers/ifc'
parent.mkdir(parents=True)
(parent / 'worker.py').write_bytes(b'reviewed')
external = base / 'external'
external.mkdir()
(external / 'worker.py').write_bytes(b'unreviewed')
original_directory = manifest.AnchoredRoot.directory
mounted = False


def swap_source(instance, parts, *args, **kwargs):
    global mounted
    if not mounted and tuple(parts) == ('workers', 'ifc'):
        bind(external, parent)
        mounted = True
    return original_directory(instance, parts, *args, **kwargs)

try:
    with patch.object(manifest.AnchoredRoot, 'directory', swap_source):
        must_refuse(lambda: manifest.read_regular(source, 'workers/ifc/worker.py'), 'source parent')
finally:
    if mounted:
        unmount(parent)
assert mounted

stage = base / 'stage'
parent = stage / 'safe'
parent.mkdir(parents=True)
external_stage = base / 'external-stage'
external_stage.mkdir()
mounted = False


def swap_stage(instance, parts, *args, **kwargs):
    global mounted
    if not mounted and tuple(parts) == ('safe',):
        bind(external_stage, parent)
        mounted = True
    return original_directory(instance, parts, *args, **kwargs)

try:
    with manifest.AnchoredRoot(stage) as dest, patch.object(manifest.AnchoredRoot, 'directory', swap_stage):
        must_refuse(lambda: dest.write_bytes('safe/worker.py', b'PACKAGED BYTES'), 'stage parent')
finally:
    if mounted:
        unmount(parent)
assert mounted and not (external_stage / 'worker.py').exists()

file_root = base / 'file-root'
file_root.mkdir()
(file_root / 'worker.py').write_bytes(b'reviewed')
external_file = base / 'external-file'
external_file.write_bytes(b'unreviewed')
bind(external_file, file_root / 'worker.py')
try:
    must_refuse(lambda: manifest.read_regular(file_root, 'worker.py'), 'source file')
finally:
    unmount(file_root / 'worker.py')

original_verify_parent = manifest.AnchoredRoot.verify_parent
mounted = False


def swap_file_during_verification(instance, parts, pinned):
    global mounted
    if not mounted and tuple(parts) == ():
        bind(file_root / 'worker.py', file_root / 'worker.py')
        mounted = True
    return original_verify_parent(instance, parts, pinned)


try:
    with manifest.AnchoredRoot(file_root) as anchored, patch.object(
            manifest.AnchoredRoot, 'verify_parent', swap_file_during_verification):
        must_refuse(lambda: anchored.read_bytes('worker.py'), 'file mount during final verification')
finally:
    if mounted:
        unmount(file_root / 'worker.py')
assert mounted

allowed_root = base / 'allowed-root'
allowed_root.mkdir()
bind(external, allowed_root)
try:
    with manifest.AnchoredRoot(allowed_root) as anchored:
        assert anchored.read_bytes('worker.py') == b'unreviewed'
finally:
    unmount(allowed_root)

with manifest.AnchoredRoot(source) as anchored:
    bind(source, source)
    try:
        must_refuse(anchored.verify_root, 'root mount replacement')
    finally:
        unmount(source)
print('source, staging, file and root replacement refused; initial root mount accepted')
