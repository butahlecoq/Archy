#!/usr/bin/env python3
"""Stage the fixed backend payload from checksum-locked inputs in an Arch build root."""
import argparse
import email
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from runtime_manifest import AnchoredRoot, capture_tree

PREFIX = Path('usr/lib/archy/runtime')
PYTHON_PREFIX = PREFIX / 'python'
BLENDER_PREFIX = PREFIX / 'blender'
MAX_FILE = 512 * 1024 * 1024
UPSTREAM_PINS = {
    'blender': '0d2b8bb2efe97e6c8eb57de6b7322f8800e1984e3125b60decca5f6771e4773e',
    'node': '88fd1ce767091fd8d4a99fdb2356e98c819f93f3b1f8663853a2dee9b438068a',
    'web_ifc': '4c88d9a07ebf5e1e646255edbed02ae2d8cd3a64ff6c3ea52e383525f16d4a26',
    'bwrap': '33516b9cd1fa6432307ad71f23765fa30604b0a8d769dac0d2ba054e85aeec83',
}


def pinned_bytes(path, expected):
    path = Path(os.path.abspath(path))
    with AnchoredRoot(path.parent) as root:
        data = root.read_bytes(path.name, MAX_FILE)
    if not re.fullmatch('[0-9a-f]{64}', expected) or hashlib.sha256(data).hexdigest() != expected:
        raise ValueError(f'locked input mismatch: {path}')
    return data


def member_name(name):
    p = PurePosixPath(name)
    if p.is_absolute() or any(x in ('', '.', '..') for x in name.split('/')):
        raise ValueError(f'unsafe archive member: {name}')
    return p


def copy_tree(source, stage, destination):
    with AnchoredRoot(source) as source_root:
        for relative in source_root.files():
            data, mode = source_root.read_with_mode(relative, MAX_FILE)
            stage.write_bytes(destination / relative, data, 0o755 if mode & 0o111 else 0o644)


def extract_blender(data, stage, dest):
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:xz') as tf:
        members = tf.getmembers()
        root = 'blender-4.5.15-linux-x64'
        names = set()
        for m in members:
            p = member_name(m.name.rstrip('/'))
            if p.parts[0] != root or not (m.isfile() or m.isdir() or m.issym()) or m.name in names or m.size > MAX_FILE:
                raise ValueError(f'unsafe Blender archive entry: {m.name}')
            names.add(m.name)

        def copy_member(member, relative, mode):
            with tf.extractfile(member) as source, stage.create_file(dest / relative, mode) as target:
                shutil.copyfileobj(source, target)
            if relative.parts[0] == 'license':
                notice = Path('usr/share/licenses/archy-runtime/blender') / relative.relative_to('license')
            elif str(relative) == 'copyright.txt':
                notice = Path('usr/share/licenses/archy-runtime/blender-copyright.txt')
            else:
                return
            with tf.extractfile(member) as source, stage.create_file(notice) as target:
                shutil.copyfileobj(source, target)

        for m in members:
            if not m.isfile():
                continue
            rel = member_name(m.name).relative_to(root)
            copy_member(m, rel, 0o755 if m.mode & 0o111 else 0o644)
        for m in members:
            if not m.issym():
                continue
            rel = member_name(m.name).relative_to(root)
            parent = PurePosixPath(m.name).parent
            target = member_name(str(parent / m.linkname))
            # Resolve symlink chains through the archive's own member map only.
            for _ in range(16):
                source_member = tf.getmember(str(target))
                if source_member.isfile():
                    break
                if not source_member.issym():
                    raise ValueError('Blender link is not a file')
                target = member_name(str(PurePosixPath(source_member.name).parent / source_member.linkname))
            else:
                raise ValueError('Blender link cycle')
            if target.parts[0] != root:
                raise ValueError('Blender link escapes root')
            copy_member(source_member, rel, 0o644)


def extract_regular_archive(data, archive_root, stage, destination, selected=None, notice=None):
    found = set()
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:*') as tf:
        seen = set()
        for member in tf.getmembers():
            rel = member_name(member.name.rstrip('/'))
            if rel.parts[0] != archive_root or member.name in seen:
                raise ValueError(f'unsafe archive entry: {member.name}')
            seen.add(member.name)
            target_rel = rel.relative_to(archive_root)
            if member.issym() and selected is not None and target_rel.as_posix() not in selected:
                continue
            if not (member.isfile() or member.isdir()) or member.size > MAX_FILE:
                raise ValueError(f'unsafe archive entry: {member.name}')
            if not member.isfile():
                continue
            if selected is not None and target_rel.as_posix() not in selected:
                continue
            with tf.extractfile(member) as source, stage.create_file(destination / target_rel, 0o755 if member.mode & 0o111 else 0o644) as target:
                shutil.copyfileobj(source, target)
            if notice is not None and target_rel.as_posix() == notice[0]:
                with tf.extractfile(member) as source, stage.create_file(notice[1]) as target:
                    shutil.copyfileobj(source, target)
            found.add(target_rel.as_posix())
    if selected is not None and found != selected:
        raise ValueError('archive lacks selected runtime file')
    if notice is not None and notice[0] not in found:
        raise ValueError('archive lacks required notice')


def install_wheels(wheelhouse, lock, stage, site, notice_root=None):
    pinned = {}
    active = None
    for line in lock.decode('utf-8').splitlines():
        match = re.match(r'^([a-z0-9-]+)==([0-9][^\s\\]*)', line)
        if match:
            active = match.group(1)
            if active in pinned:
                raise ValueError('duplicate wheel lock entry')
            pinned[active] = (match.group(2), set())
        for value in re.findall(r'--hash=sha256:([0-9a-f]{64})', line):
            if active is None:
                raise ValueError('orphan wheel hash')
            pinned[active][1].add(value)
    seen = set()
    def wheel_inputs():
        with AnchoredRoot(wheelhouse) as source:
            wheels = sorted(name for name in os.listdir(source.fd) if name.endswith('.whl'))
            if len(wheels) != 15:
                raise ValueError('expected exact 15-wheel IFC closure')
            for name in wheels:
                yield name, source.read_bytes(name, MAX_FILE)
    for wheel, data in wheel_inputs():
        name, version = wheel.split('-')[:2]
        canonical = name.lower().replace('_', '-')
        if canonical in seen or canonical not in pinned or pinned[canonical][0] != version or hashlib.sha256(data).hexdigest() not in pinned[canonical][1]:
            raise ValueError(f'wheel absent from hash lock: {wheel}')
        seen.add(canonical)
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            if z.testzip() is not None:
                raise ValueError('corrupt wheel')
            names = set(z.namelist())
            metadata = [path for path in names if path.endswith('.dist-info/METADATA')]
            if len(metadata) != 1:
                raise ValueError(f'wheel metadata closure: {wheel}')
            dist = metadata[0].rsplit('/', 1)[0]
            fields = email.message_from_bytes(z.read(metadata[0]))
            declared_name = fields.get('Name', '').lower().replace('_', '-')
            if declared_name != canonical or fields.get('Version') != version:
                raise ValueError(f'wheel metadata identity: {wheel}')
            license_files = fields.get_all('License-File', [])
            if not license_files and canonical != 'ifcopenshell':
                raise ValueError(f'wheel lacks declared license text: {wheel}')
            for license_file in license_files:
                rel = member_name(license_file)
                candidates = (dist + '/licenses/' + str(rel), dist + '/' + str(rel))
                origin = next((path for path in candidates if path in names), None)
                if origin is None:
                    raise ValueError(f'wheel license text missing: {wheel}: {license_file}')
                if notice_root is not None:
                    with z.open(origin) as source, stage.create_file(notice_root / f'{canonical}-{version}' / rel) as output:
                        shutil.copyfileobj(source, output)
            if notice_root is not None:
                with z.open(metadata[0]) as source, stage.create_file(notice_root / f'{canonical}-{version}' / 'METADATA') as output:
                    shutil.copyfileobj(source, output)
            for info in z.infolist():
                if info.is_dir():
                    continue
                rel = member_name(info.filename)
                if '.data' in rel.parts or info.file_size > MAX_FILE or stat.S_ISLNK(info.external_attr >> 16):
                    raise ValueError(f'unsupported wheel member: {rel}')
                with z.open(info) as source, stage.create_file(site / rel, 0o755 if (info.external_attr >> 16) & 0o111 else 0o644) as target:
                    shutil.copyfileobj(source, target)
    if seen != set(pinned):
        raise ValueError('wheel lock closure mismatch')




def system_closure(stage):
    """Resolve each dynamic ELF's direct search paths without executing payload."""
    loader_env = {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}
    payload_root = (stage.path / PREFIX).resolve(strict=True)
    system = {Path('/usr/lib/ld-linux-x86-64.so.2').resolve(strict=True)}
    inspected = set()
    records = {}

    def inspect(fd, origin):
        if os.pread(fd, 4, 0) != b'\x7fELF':
            return None
        fd_path = '/proc/self/fd/' + str(fd)
        result = subprocess.run(['/usr/bin/readelf', '-dW', fd_path], text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True,
                                env=loader_env, pass_fds=(fd,))
        if 'Dynamic section at offset' not in result.stdout:
            return None
        needs = set()
        search_values = {}
        for line in result.stdout.splitlines():
            if '(FLAGS_1)' in line and 'NODEFLIB' in line:
                raise ValueError(f'unsupported ELF NODEFLIB: {origin}')
            if re.search(r'\((?:AUDIT|DEPAUDIT|FILTER|AUXILIARY)\)', line):
                raise ValueError(f'unsupported ELF audit or filter: {origin}')
            if '(NEEDED)' in line:
                match = re.search(r'\[([^]]+)\]', line)
                if not match:
                    raise ValueError('invalid ELF needed entry')
                name = match.group(1)
                if '/' in name or '$' in name or name in ('.', '..'):
                    raise ValueError(f'unsupported ELF dependency name: {name}')
                needs.add(name)
            for tag in ('RPATH', 'RUNPATH'):
                if f'({tag})' not in line:
                    continue
                if tag in search_values:
                    raise ValueError(f'duplicate ELF {tag}')
                match = re.search(r'\[([^]]*)\]', line)
                if not match:
                    raise ValueError('invalid ELF search path')
                paths = set()
                for item in match.group(1).split(':'):
                    expanded = item.replace('${ORIGIN}', str(origin)).replace('$ORIGIN', str(origin))
                    if not expanded or '$' in expanded or not Path(expanded).is_absolute():
                        raise ValueError(f'unsupported ELF search path: {item}')
                    path = Path(os.path.normpath(expanded))
                    final_root = Path('/') / PREFIX
                    if path == final_root or path.is_relative_to(final_root):
                        path = payload_root / path.relative_to(final_root)
                    try:
                        path = path.resolve(strict=True)
                    except FileNotFoundError:
                        continue
                    if not (path == payload_root or path.is_relative_to(payload_root) or
                            path == Path('/usr/lib') or path.is_relative_to('/usr/lib')):
                        raise ValueError(f'ELF search path outside package/system: {path}')
                    if not path.is_dir():
                        raise ValueError(f'ELF search path is not a directory: {path}')
                    paths.add(path)
                search_values[tag] = paths
        has_runpath = 'RUNPATH' in search_values
        search = search_values.get('RUNPATH', search_values.get('RPATH', set()))
        inherited = set() if has_runpath else search_values.get('RPATH', set())
        return needs, search, inherited, has_runpath

    entry_points = {str(PREFIX / item) for item in
                    ('bin/bwrap', 'blender/blender', 'python/bin/python3.12', 'node/bin/node')}
    seen_entries = set()
    for relative in stage.files(PREFIX):
        with stage.open_read(relative) as (fd, _):
            info = inspect(fd, (stage.path / relative).parent)
            if info is not None:
                records[stage.path / relative] = info
            if relative in entry_points:
                if info is None:
                    raise ValueError(f'fixed runtime entry is not dynamic ELF: {relative}')
                seen_entries.add(relative)
    if seen_entries != entry_points:
        raise ValueError('fixed runtime ELF entry point missing')

    def expanded_dirs(directories):
        candidates = set(directories)
        def add_hwcaps(anchor, parts, directory):
            try:
                fd = anchor.directory((*parts, 'glibc-hwcaps'))
            except FileNotFoundError:
                return
            try:
                with os.scandir(fd) as scan:
                    names = sorted(entry.name for entry in scan)
                for name in names:
                    child_parts = (*parts, 'glibc-hwcaps', name)
                    child = anchor.directory(child_parts)
                    try:
                        anchor.verify_parent(child_parts, child)
                    finally:
                        os.close(child)
                    candidates.add(directory / 'glibc-hwcaps' / name)
                anchor.verify_parent((*parts, 'glibc-hwcaps'), fd)
            finally:
                os.close(fd)
        for directory in sorted(directories):
            if directory == payload_root or directory.is_relative_to(payload_root):
                add_hwcaps(stage, directory.relative_to(stage.path).parts, directory)
            else:
                with AnchoredRoot(directory) as source:
                    add_hwcaps(source, (), directory)
        return candidates

    while True:
        before = len(system)
        for path in sorted(system - inspected):
            with AnchoredRoot(path.parent) as root:
                with root.open_read(path.name) as (fd, _):
                    info = inspect(fd, path.parent)
                    if info is None:
                        raise ValueError(f'system dependency is not dynamic ELF: {path}')
                    records[path] = info
            inspected.add(path)
        inherited = set().union(*(record[2] for record in records.values()))
        for requester, (needs, search, _, has_runpath) in sorted(records.items()):
            # hwcaps levels are CPU-dependent; only an ordinary search directory
            # proves availability on every supported CPU. Audit alternatives too.
            own_base = search | {Path('/usr/lib')}
            own = expanded_dirs(own_base)
            extra = set() if has_runpath else expanded_dirs(inherited)
            for name in sorted(needs):
                found_own = False
                for directory in sorted(own | extra):
                    if directory == payload_root or directory.is_relative_to(payload_root):
                        relative = (directory / name).relative_to(stage.path)
                        try:
                            with stage.open_read(relative) as (fd, _):
                                if os.pread(fd, 4, 0) != b'\x7fELF':
                                    raise ValueError(f'packaged ELF dependency is not ELF: {relative}')
                        except FileNotFoundError:
                            continue
                        if directory in own_base:
                            found_own = True
                        continue
                    try:
                        path = (directory / name).resolve(strict=True)
                    except FileNotFoundError:
                        continue
                    if not path.is_file():
                        continue
                    if directory in own_base:
                        found_own = True
                    if not path.is_relative_to('/usr/lib'):
                        raise ValueError(f'external ELF outside /usr/lib: {path}')
                    system.add(path)
                if not found_own:
                    raise ValueError(f'ELF requester {requester} has no dependency in ordinary search directories: {name}')
        if before == len(system) and system <= inspected:
            break
    stage.verify_root()
    rows = []
    for path in sorted(system):
        with AnchoredRoot(path.parent) as source:
            data, mode = source.read_with_mode(path.name, MAX_FILE)
        rows.append(dict(path=str(path), sha256=hashlib.sha256(data).hexdigest(), mode=mode))
    return rows


def captured_targets(entries):
    """Map each reviewed source byte sequence to every packaged destination."""
    package_licenses = Path('usr/share/licenses/archy-runtime')
    for name, data in sorted(entries.items()):
        if name.startswith('workers/blender/') or name.startswith('workers/ifc/'):
            relative = name.removeprefix('workers/')
            yield PREFIX / 'workers' / relative, data
            prefix = 'workers/ifc/archy_ifc_worker/'
            if name.startswith(prefix):
                yield PYTHON_PREFIX / 'lib/python3.12/site-packages/archy_ifc_worker' / name.removeprefix(prefix), data
            if name in ('workers/blender/LICENSE', 'workers/ifc/LICENSE'):
                yield package_licenses / (name.split('/')[1] + '-worker-LICENSE'), data
        elif name.startswith('packaging/arch/runtime-verifier/'):
            yield PREFIX / 'ifc-verifier' / name.rsplit('/', 1)[1], data
        elif name.startswith('packaging/arch/runtime-notices/') and not name.endswith('/sources.json'):
            leaf = name.rsplit('/', 1)[1]
            yield PREFIX / 'licenses' / leaf, data
            yield package_licenses / 'runtime' / leaf, data


def stage_captured_inputs(entries, stage):
    """Stage the exact source bytes already captured and reviewed by preflight."""
    for path, data in captured_targets(entries):
        stage.write_bytes(path, data)


def verify_captured_inputs(entries, stage):
    for path, expected in captured_targets(entries):
        data, mode = stage.read_with_mode(path, MAX_FILE)
        if data != expected or mode != 0o644:
            raise ValueError(f'staged reviewed input changed: {path}')


def main():
    p = argparse.ArgumentParser()
    for key in ('dest', 'python-prefix', 'python-license', 'blender-archive', 'blender-sha256',
                'node-archive', 'node-sha256', 'web-ifc-archive', 'web-ifc-sha256',
                'wheelhouse', 'bwrap', 'bwrap-sha256', 'source'):
        p.add_argument('--' + key, required=True)
    a = p.parse_args()
    with AnchoredRoot(a.dest) as stage:
        if os.listdir(stage.fd):
            raise ValueError('destination must be an empty disposable package root')
        entries = capture_tree(a.source)
        if any(getattr(a, name + '_sha256') != expected for name, expected in UPSTREAM_PINS.items()):
            raise ValueError('upstream runtime pin mismatch')
        python_license = pinned_bytes(a.python_license, '3b2f81fe21d181c499c59a256c8e1968455d6689d269aa85373bfb6af41da3bf')
        bwrap = pinned_bytes(a.bwrap, a.bwrap_sha256)
        copy_tree(a.python_prefix, stage, PYTHON_PREFIX)
        blender = pinned_bytes(a.blender_archive, a.blender_sha256)
        extract_blender(blender, stage, BLENDER_PREFIX)
        del blender
        node = pinned_bytes(a.node_archive, a.node_sha256)
        extract_regular_archive(node, 'node-v22.22.2-linux-x64', stage, PREFIX / 'node',
                                {'bin/node', 'LICENSE'},
                                ('LICENSE', Path('usr/share/licenses/archy-runtime/node-LICENSE')))
        del node
        web_ifc = pinned_bytes(a.web_ifc_archive, a.web_ifc_sha256)
        extract_regular_archive(web_ifc, 'package', stage, PREFIX / 'ifc-verifier/node_modules/web-ifc',
                                notice=('LICENSE.md', Path('usr/share/licenses/archy-runtime/web-ifc-LICENSE.md')))
        del web_ifc
        stage_captured_inputs(entries, stage)
        install_wheels(a.wheelhouse, entries['workers/ifc/requirements.lock'], stage,
                       PYTHON_PREFIX / 'lib/python3.12/site-packages',
                       Path('usr/share/licenses/archy-runtime/wheels'))
        stage.write_bytes(PREFIX / 'licenses/cpython-LICENSE', python_license)
        stage.write_bytes('usr/share/licenses/archy-runtime/runtime/cpython-LICENSE', python_license)
        stage.write_bytes(PREFIX / 'bin/bwrap', bwrap, 0o755)
        verify_captured_inputs(entries, stage)
        required = ('python/bin/python3.12', 'blender/blender', 'bin/bwrap', 'node/bin/node',
                'ifc-verifier/reader.cjs', 'ifc-verifier/verify-stdin.cjs',
                'ifc-verifier/step-scope.cjs',
                'ifc-verifier/node_modules/web-ifc/web-ifc-api-node.js',
                'ifc-verifier/node_modules/web-ifc/web-ifc-node.wasm',
                'ifc-verifier/node_modules/web-ifc/package.json',
                'workers/blender/worker.py', 'workers/ifc/archy_ifc_worker/worker.py')
        paths = stage.files(PREFIX)
        if any(str(PREFIX / path) not in paths for path in required):
            raise ValueError('missing required runtime')
        rows = []
        for path in paths:
            data, mode = stage.read_with_mode(path, MAX_FILE)
            rows.append(dict(path='/' + path, sha256=hashlib.sha256(data).hexdigest(), mode=mode))
        rows.sort(key=lambda row: row['path'])
        system_files = system_closure(stage)
        verify_captured_inputs(entries, stage)
        inventory = (json.dumps(dict(schema_version=2, files=rows, system_files=system_files),
                                separators=(',', ':')) + '\n').encode()
        if len(inventory) > 16 * 1024 * 1024:
            raise ValueError('runtime inventory too large')
        stage.write_bytes(PREFIX / 'inventory.json', inventory)
        stage.verify_outputs()
        print(json.dumps(dict(files=len(rows), system_files=len(system_files),
                              inventory_sha256=hashlib.sha256(inventory).hexdigest())))

if __name__ == '__main__':
    main()
