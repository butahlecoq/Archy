#!/usr/bin/env python3
"""Validate reviewed runtime inputs before creating or consuming a snapshot."""
import argparse
from contextlib import contextmanager
import ctypes
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import tarfile

MANIFEST = 'packaging/arch/runtime-inputs.json'
GUARD = 'packaging/arch/runtime_manifest.py'
TRUSTED_REVIEWS = {'a12': frozenset(), 'distribution': frozenset()}
TOOLS = {'packaging/arch/' + name for name in
         ('runtime.py', 'runtime_manifest.py', 'runtime_recipe.py', 'runtime_snapshot.py')}
VERIFIERS = {'packaging/arch/runtime-verifier/' + name for name in
             ('reader.cjs', 'step-scope.cjs', 'verify-stdin.cjs')}
REQUIRED_WORKERS = {
    'workers/blender/LICENSE', 'workers/blender/worker.py', 'workers/blender/scene.py',
    'workers/blender/protocol.py', 'workers/ifc/LICENSE', 'workers/ifc/requirements.lock',
    *('workers/ifc/archy_ifc_worker/' + name for name in
      ('__init__.py', '__main__.py', 'geometry.py', 'hierarchy.py', 'properties.py',
       'protocol.py', 'verify.py', 'worker.py')),
}
REQUIRED_NOTICES = {'bubblewrap-COPYING', 'ifcopenshell-COPYING', 'ifcopenshell-COPYING.LESSER',
                    'ifc-mvd-LICENSE', 'ifc-simple-spf-LICENSE'}
NOTICE_DIR = 'packaging/arch/runtime-notices/'
MAX_FILE = 16 * 1024 * 1024
MAX_TOTAL = 64 * 1024 * 1024
MAX_FILES = 4096
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
RESOLVE_NO_XDEV = 0x01
RESOLVE_NO_MAGICLINKS = 0x02
RESOLVE_NO_SYMLINKS = 0x04
RESOLVE_BENEATH = 0x08


class OpenHow(ctypes.Structure):
    _fields_ = [('flags', ctypes.c_uint64), ('mode', ctypes.c_uint64),
                ('resolve', ctypes.c_uint64)]


def open_component(parent, name, flags, mode=0):
    """Linux openat2, fail closed when unavailable or crossing any mount."""
    if os.uname().machine != 'x86_64':
        raise OSError('runtime packaging requires x86_64 openat2')
    how = OpenHow(flags, mode, RESOLVE_NO_XDEV | RESOLVE_NO_MAGICLINKS |
                  RESOLVE_NO_SYMLINKS | RESOLVE_BENEATH)
    libc = ctypes.CDLL(None, use_errno=True)
    fd = libc.syscall(ctypes.c_long(437), ctypes.c_int(parent),
                      ctypes.c_char_p(os.fsencode(name)), ctypes.byref(how),
                      ctypes.c_size_t(ctypes.sizeof(how)))
    if fd < 0:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), name)
    return fd


def mount_id(fd):
    with open(f'/proc/self/fdinfo/{fd}', encoding='ascii') as info:
        for line in info:
            if line.startswith('mnt_id:'):
                return int(line.split(':', 1)[1])
    raise ValueError('mount identity unavailable')


def open_directory(path):
    absolute = Path(os.path.abspath(path))
    fd = os.open('/', DIR_FLAGS)
    try:
        for part in absolute.parts[1:]:
            child = os.open(part, DIR_FLAGS, dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except Exception:
        os.close(fd)
        raise


def same_inode(a, b):
    return (a.st_dev, a.st_ino) == (b.st_dev, b.st_ino)


class AnchoredRoot:
    """Operate below a directory held open without following symlinks."""

    def __init__(self, root):
        self.path = Path(os.path.abspath(root))
        self.fd = open_directory(self.path)
        self.writes = {}

    def __enter__(self):
        return self

    def __exit__(self, *_):
        os.close(self.fd)

    def directory(self, parts, create=False):
        fd = os.dup(self.fd)
        try:
            for part in parts:
                try:
                    child = open_component(fd, part, DIR_FLAGS)
                except FileNotFoundError:
                    if not create:
                        raise
                    os.mkdir(part, 0o755, dir_fd=fd)
                    child = open_component(fd, part, DIR_FLAGS)
                os.close(fd)
                fd = child
            return fd
        except Exception:
            os.close(fd)
            raise

    def verify_root(self):
        fd = open_directory(self.path)
        try:
            if not same_inode(os.fstat(fd), os.fstat(self.fd)) or mount_id(fd) != mount_id(self.fd):
                raise ValueError('root directory was replaced')
        finally:
            os.close(fd)

    def verify_parent(self, parts, pinned):
        self.verify_root()
        current = self.directory(parts)
        try:
            if not same_inode(os.fstat(current), os.fstat(pinned)) or mount_id(current) != mount_id(pinned):
                raise ValueError('parent directory was replaced')
        finally:
            os.close(current)

    def verify_entry(self, parent, name, pinned):
        current = open_component(parent, name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        try:
            old, new = os.fstat(pinned), os.fstat(current)
            if (not same_inode(old, new) or mount_id(pinned) != mount_id(current) or
                (old.st_size, old.st_mtime_ns, old.st_ctime_ns) !=
                    (new.st_size, new.st_mtime_ns, new.st_ctime_ns)):
                raise ValueError(f'file replaced: {name}')
        finally:
            os.close(current)

    def read_with_mode(self, relative, limit=MAX_FILE):
        parts = safe_name(str(relative)).parts
        parent = self.directory(parts[:-1])
        try:
            fd = open_component(parent, parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
            with os.fdopen(fd, 'rb') as stream:
                info = os.fstat(stream.fileno())
                if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
                    raise ValueError(f'invalid input file: {relative}')
                data = stream.read(info.st_size + 1)
                if len(data) > limit:
                    raise ValueError(f'input file exceeds limit: {relative}')
                final = os.fstat(stream.fileno())
                if (info.st_size, info.st_mtime_ns, info.st_ctime_ns) != (final.st_size, final.st_mtime_ns, final.st_ctime_ns) or len(data) != info.st_size:
                    raise ValueError(f'input file changed: {relative}')
                if not same_inode(info, os.stat(parts[-1], dir_fd=parent, follow_symlinks=False)):
                    raise ValueError(f'input file replaced: {relative}')
                self.verify_parent(parts[:-1], parent)
                self.verify_entry(parent, parts[-1], stream.fileno())
            return data, info.st_mode & 0o7777
        finally:
            os.close(parent)

    def read_bytes(self, relative, limit=MAX_FILE):
        return self.read_with_mode(relative, limit)[0]

    @contextmanager
    def open_read(self, relative):
        parts = safe_name(str(relative)).parts
        parent = self.directory(parts[:-1])
        fd = None
        try:
            fd = open_component(parent, parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
            initial = os.fstat(fd)
            if not stat.S_ISREG(initial.st_mode):
                raise ValueError(f'not a regular file: {relative}')
            yield fd, parent
            final = os.fstat(fd)
            current = os.stat(parts[-1], dir_fd=parent, follow_symlinks=False)
            if not same_inode(initial, current) or (initial.st_size, initial.st_mtime_ns, initial.st_ctime_ns) != (final.st_size, final.st_mtime_ns, final.st_ctime_ns):
                raise ValueError(f'input file changed: {relative}')
            self.verify_parent(parts[:-1], parent)
            self.verify_entry(parent, parts[-1], fd)
        finally:
            if fd is not None:
                os.close(fd)
            os.close(parent)

    def files(self, relative=''):
        paths = []

        def walk(parts):
            fd = self.directory(parts)
            try:
                with os.scandir(fd) as scan:
                    names = sorted(item.name for item in scan)
                for name in names:
                    info = os.stat(name, dir_fd=fd, follow_symlinks=False)
                    child = (*parts, name)
                    if stat.S_ISDIR(info.st_mode):
                        walk(child)
                    elif stat.S_ISREG(info.st_mode):
                        paths.append('/'.join(child))
                    else:
                        raise ValueError(f'unsafe tree entry: {"/".join(child)}')
                self.verify_parent(parts, fd)
            finally:
                os.close(fd)

        walk(safe_name(str(relative)).parts if relative else ())
        return paths

    @contextmanager
    def create_file(self, relative, mode=0o644):
        parts = safe_name(str(relative)).parts
        parent = self.directory(parts[:-1], create=True)
        created = None
        try:
            fd = open_component(parent, parts[-1], os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, mode)
            with os.fdopen(fd, 'wb') as stream:
                created = os.fstat(stream.fileno())
                digest = hashlib.sha256()

                class Writer:
                    def write(self, data):
                        count = stream.write(data)
                        if count != len(data):
                            raise ValueError('short output write')
                        digest.update(data)
                        return count

                    def __getattr__(self, name):
                        return getattr(stream, name)

                yield Writer()
                stream.flush()
                os.fchmod(stream.fileno(), mode)
                self.verify_parent(parts[:-1], parent)
                if not same_inode(created, os.stat(parts[-1], dir_fd=parent, follow_symlinks=False)):
                    raise ValueError(f'output file replaced: {relative}')
                self.verify_entry(parent, parts[-1], stream.fileno())
                self.writes[str(relative)] = (digest.hexdigest(), mode)
        except BaseException:
            if created is not None:
                try:
                    current = os.stat(parts[-1], dir_fd=parent, follow_symlinks=False)
                    if same_inode(created, current):
                        os.unlink(parts[-1], dir_fd=parent)
                except FileNotFoundError:
                    pass
            raise
        finally:
            os.close(parent)

    def write_bytes(self, relative, data, mode=0o644):
        with self.create_file(relative, mode) as stream:
            stream.write(data)

    def verify_outputs(self, require_exact_tree=True):
        if require_exact_tree and set(self.files()) != set(self.writes):
            raise ValueError('staged file set changed')
        for name, (expected, mode) in self.writes.items():
            digest = hashlib.sha256()
            with self.open_read(name) as (fd, _):
                if os.fstat(fd).st_mode & 0o7777 != mode:
                    raise ValueError(f'staged file mode changed: {name}')
                while True:
                    chunk = os.read(fd, 1024 * 1024)
                    if not chunk:
                        break
                    digest.update(chunk)
            if digest.hexdigest() != expected:
                raise ValueError(f'staged file changed: {name}')

    def verified_output_digest(self, relative, held_fd, limit):
        expected, mode = self.writes[str(relative)]
        before = os.fstat(held_fd)
        if not stat.S_ISREG(before.st_mode) or before.st_size > limit or before.st_mode & 0o7777 != mode:
            raise ValueError('created output has invalid mode or size')
        digest = hashlib.sha256()
        offset = 0
        while offset < before.st_size:
            chunk = os.pread(held_fd, min(1024 * 1024, before.st_size - offset), offset)
            if not chunk:
                raise ValueError('created output changed while reading')
            digest.update(chunk)
            offset += len(chunk)
        after = os.fstat(held_fd)
        if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise ValueError('created output changed while reading')
        if digest.hexdigest() != expected:
            raise ValueError('created output differs from written bytes')
        with self.open_read(relative) as (current, _):
            observed = os.fstat(current)
            if (not same_inode(after, observed) or mount_id(held_fd) != mount_id(current) or
                (after.st_size, after.st_mtime_ns, after.st_ctime_ns) !=
                    (observed.st_size, observed.st_mtime_ns, observed.st_ctime_ns)):
                raise ValueError('created output was replaced')
        return digest.hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f'duplicate JSON key: {key}')
        result[key] = value
    return result


def parse_json(data):
    return json.loads(data, object_pairs_hook=unique_object)


def safe_name(name):
    if not isinstance(name, str) or not name or name.startswith('/') or any(
            part in ('', '.', '..') for part in name.split('/')):
        raise ValueError(f'unsafe input path: {name}')
    return PurePosixPath(name)


def check_digest(data, expected, name):
    if not isinstance(expected, str) or not re.fullmatch('[0-9a-f]{64}', expected):
        raise ValueError(f'invalid input pin: {name}')
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError(f'input digest mismatch: {name}')


def review_paths(manifest, allow_pending=False):
    if not isinstance(manifest, dict) or set(manifest) != {'schema_version', 'files', 'reviews'} or type(manifest['schema_version']) is not int or manifest['schema_version'] != 1:
        raise ValueError('invalid runtime input manifest')
    reviews = manifest['reviews']
    if not isinstance(reviews, dict) or set(reviews) != {'a12', 'distribution'}:
        raise ValueError('missing required runtime reviews')
    paths = {}
    for name, review in reviews.items():
        if not isinstance(review, dict) or set(review) != {'status', 'evidence_path', 'sha256'}:
            raise ValueError(f'invalid {name} review record')
        if review['status'] != 'reviewed':
            if allow_pending and review['status'] in ('pending', 'superseded') and review['evidence_path'] is None and review['sha256'] is None:
                continue
            raise ValueError(f'{name} review is not complete')
        if not isinstance(review['sha256'], str) or review['sha256'] not in TRUSTED_REVIEWS[name]:
            raise ValueError(f'{name} review evidence is not trusted by the packaging guard')
        path = str(safe_name(review['evidence_path']))
        if not path.startswith('packaging/arch/runtime-review/') or not path.endswith('.md') or path in paths:
            raise ValueError(f'invalid {name} review evidence path')
        paths[path] = review['sha256']
    return paths


def validate_entries(entries, allow_pending=False):
    if MANIFEST not in entries:
        raise ValueError('runtime input manifest missing')
    manifest = parse_json(entries[MANIFEST])
    reviews = review_paths(manifest, allow_pending)
    files = manifest['files']
    if not isinstance(files, dict) or not (VERIFIERS | REQUIRED_WORKERS) <= files.keys():
        raise ValueError('runtime manifest lacks required verifier or worker inputs')
    for path in files:
        safe_name(path)
        if not (path.startswith('workers/blender/') or path.startswith('workers/ifc/') or path in VERIFIERS):
            raise ValueError(f'unexpected runtime manifest path: {path}')
    if {p for p in files if p.startswith('packaging/arch/runtime-verifier/')} != VERIFIERS:
        raise ValueError('verifier manifest is incomplete')
    for path, expected in {**files, **reviews}.items():
        if path not in entries or not entries[path]:
            raise ValueError(f'required input missing or empty: {path}')
        check_digest(entries[path], expected, path)
    notice_manifest = NOTICE_DIR + 'sources.json'
    if notice_manifest not in entries:
        raise ValueError('notice source manifest missing')
    notices = parse_json(entries[notice_manifest])
    if not isinstance(notices, dict) or not REQUIRED_NOTICES <= notices.keys():
        raise ValueError('required separately sourced notices missing')
    notice_paths = set()
    for name, record in notices.items():
        if str(safe_name(name)) != name or '/' in name or not isinstance(record, dict) or set(record) != {'url', 'sha256'} or not isinstance(record['url'], str) or not record['url'].startswith('https://'):
            raise ValueError('invalid notice source')
        path = NOTICE_DIR + name
        if path not in entries or not entries[path]:
            raise ValueError(f'notice missing: {path}')
        check_digest(entries[path], record['sha256'], path)
        notice_paths.add(path)
    expected_paths = set(files) | set(reviews) | TOOLS | {MANIFEST, notice_manifest} | notice_paths
    if set(entries) != expected_paths:
        raise ValueError('snapshot input membership mismatch')
    if any(not entries[path] for path in TOOLS):
        raise ValueError('runtime build tool is empty')
    # The guard contains the review hash allowlist. Excluding it avoids a
    # circular digest; the guard itself must be reviewed as packaging source.
    bound = {path: hashlib.sha256(entries[path]).hexdigest()
             for path in sorted(expected_paths - set(reviews) - {MANIFEST, GUARD})}
    identity = hashlib.sha256(json.dumps(bound, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    marker = ('Runtime-Input-Set-SHA256: ' + identity).encode()
    for path in reviews:
        if marker not in entries[path].splitlines():
            raise ValueError(f'review does not bind current inputs: {path}')
    return files, identity


def read_regular(root, relative):
    with AnchoredRoot(root) as source:
        return source.read_bytes(relative)


def capture_tree(root, allow_pending=False):
    with AnchoredRoot(root) as source:
        manifest_bytes = source.read_bytes(MANIFEST)
        manifest = parse_json(manifest_bytes)
        reviews = review_paths(manifest, allow_pending)
        entries = {MANIFEST: manifest_bytes}
        for relative in TOOLS | set(reviews):
            entries[relative] = source.read_bytes(relative)

        def walk(directory):
            parts = safe_name(directory).parts
            fd = source.directory(parts)
            try:
                with os.scandir(fd) as scan:
                    names = sorted(item.name for item in scan)
                for name in names:
                    info = os.stat(name, dir_fd=fd, follow_symlinks=False)
                    path = directory + '/' + name
                    if stat.S_ISLNK(info.st_mode):
                        raise ValueError(f'symlink input: {path}')
                    if stat.S_ISDIR(info.st_mode):
                        if name != '__pycache__':
                            walk(path)
                    elif stat.S_ISREG(info.st_mode):
                        if name.endswith('.pyc'):
                            continue
                        if len(entries) >= MAX_FILES:
                            raise ValueError('snapshot input count exceeds limit')
                        entries[path] = source.read_bytes(path)
                    else:
                        raise ValueError(f'special input: {path}')
                source.verify_parent(parts, fd)
            finally:
                os.close(fd)

        for directory in ('workers/blender', 'workers/ifc', 'packaging/arch/runtime-verifier',
                          'packaging/arch/runtime-notices'):
            walk(directory)
    if sum(map(len, entries.values())) > MAX_TOTAL:
        raise ValueError('snapshot inputs exceed size limit')
    validate_entries(entries, allow_pending)
    return entries


def validate_archive(path):
    entries = {}
    path = Path(os.path.abspath(path))
    with AnchoredRoot(path.parent) as source:
        data = source.read_bytes(path.name, MAX_TOTAL)
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        total = 0
        for member in archive:
            if not member.isfile() or not member.name.startswith('archy-source/'):
                raise ValueError('unsafe snapshot member')
            relative = member.name.removeprefix('archy-source/')
            safe_name(relative)
            total += member.size
            if relative in entries or len(entries) >= MAX_FILES or member.size > MAX_FILE or total > MAX_TOTAL or member.mode != 0o644:
                raise ValueError('invalid snapshot member')
            entries[relative] = archive.extractfile(member).read()
    validate_entries(entries)
    return entries, hashlib.sha256(data).hexdigest()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--identity', action='store_true', help='describe inputs for review; does not authorize a build')
    args = parser.parse_args()
    entries = capture_tree(args.source, allow_pending=args.identity)
    if args.identity:
        _, identity = validate_entries(entries, allow_pending=True)
        print(json.dumps({'input_set_sha256': identity, 'build_ready': False}))
