#!/usr/bin/env python3
"""Attempt a bind-mount swap while the real closure generator reads a payload."""
from contextlib import contextmanager
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile

sys.path.insert(0, sys.argv[1] + '/packaging/arch')
from runtime_manifest import AnchoredRoot
from runtime import system_closure

subprocess.run(['/usr/bin/mount', '--make-rprivate', '/'], check=True)
with tempfile.TemporaryDirectory(prefix='archy-closure-mount-') as tmp:
    base = Path(tmp)
    stage_path = base / 'stage'
    package = stage_path / 'usr/lib/archy/runtime/python/lib'
    package.mkdir(parents=True)
    for name in ('bin/bwrap', 'blender/blender', 'python/bin/python3.12', 'node/bin/node'):
        entry = stage_path / 'usr/lib/archy/runtime' / name
        entry.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile('/usr/bin/python3.14', entry)
    external = base / 'external'
    external.mkdir()
    dep_c = base / 'dep.c'
    mod_c = base / 'module.c'
    dep_c.write_text('int closure_dep(void) { return 1; }\n')
    mod_c.write_text('extern int closure_dep(void); int module(void) { return closure_dep(); }\n')
    dep = package / 'libclosureprobe.so'
    module = package / 'module.so'
    subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-Wl,-soname,libclosureprobe.so',
                    '-o', str(dep), str(dep_c)], check=True)
    subprocess.run(['/usr/bin/cc', '-shared', '-fPIC', '-o', str(module), str(mod_c),
                    '-L' + str(package), '-lclosureprobe', '-Wl,-rpath,$ORIGIN'], check=True)
    external_dep = external / dep.name
    external_dep.write_bytes(b'unreviewed external bytes')
    target = str(dep.relative_to(stage_path))
    count = 0
    mounted = False
    with AnchoredRoot(stage_path) as stage:
        original = stage.open_read

        @contextmanager
        def swapped(relative):
            global count, mounted
            if str(relative) == target:
                count += 1
                if count == 2:
                    subprocess.run(['/usr/bin/mount', '--bind', str(external), str(package)], check=True)
                    mounted = True
            with original(relative) as result:
                yield result

        stage.open_read = swapped
        try:
            try:
                system_closure(stage)
            except (OSError, ValueError):
                if not mounted:
                    raise
            else:
                raise AssertionError('closure generator accepted a bind-mounted dependency parent')
        finally:
            if mounted:
                subprocess.run(['/usr/bin/umount', str(package)], check=True)
    assert mounted and external_dep.read_bytes() == b'unreviewed external bytes'
print('PASS: closure generator denied dependency parent bind-mount swap')
