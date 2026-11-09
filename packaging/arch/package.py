#!/usr/bin/env python3
"""Bounded core package staging and pinned local-source PKGBUILD generation."""
import argparse
from contextlib import ExitStack
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import stat
import struct
import subprocess
import sys
import tarfile
import tomllib


PREFIXES = ("usr/bin/", "usr/lib/archy/", "usr/share/archy/", "usr/share/licenses/archy/")
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
NOTICE_LIMIT = 16 * 1024 * 1024
NOTICE_INVENTORY_LIMIT = 2 * 1024 * 1024
NOTICE_PATH = "usr/share/licenses/archy/THIRD-PARTY-NOTICES.txt"
NOTICE_INVENTORY_PATH = "usr/share/archy/third-party-notices.json"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def normal_registry_graph(metadata, roots=("archy-cli", "archy-service-gate")):
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root_ids = [p["id"] for p in packages.values() if p["name"] in roots and p.get("source") is None]
    if len(root_ids) != len(roots) or {packages[p]["name"] for p in root_ids} != set(roots):
        raise ValueError(f"expected local packages: {roots}")
    visited = set()
    pending = root_ids
    while pending:
        pid = pending.pop()
        if pid in visited:
            continue
        visited.add(pid)
        if pid not in nodes or pid not in packages:
            raise ValueError(f"metadata graph lacks package or node: {pid}")
        for edge in nodes[pid].get("deps", []):
            if any(kind.get("kind") in (None, "normal") for kind in edge.get("dep_kinds", [])):
                pending.append(edge["pkg"])
    result = []
    for pid in visited:
        package = packages[pid]
        if package.get("source") is not None:
            if package["source"] != REGISTRY:
                raise ValueError(f"unsupported registry source: {pid}")
            result.append(package)
    return sorted(result, key=lambda p: (p["name"], p["version"]))


def locked_graph(source):
    env = dict(os.environ, CARGO_NET_OFFLINE="true", RUSTUP_AUTO_INSTALL="0")
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--offline", "--locked", "--format-version", "1",
         "--filter-platform", "x86_64-unknown-linux-gnu"], cwd=source, env=env))
    lock = tomllib.loads(read_regular_nofollow(source / "Cargo.lock").decode("utf-8"))
    checksums = {(p["name"], p["version"]): p["checksum"] for p in lock["package"]
                 if p.get("source") == REGISTRY}
    graph = normal_registry_graph(metadata)
    return {(p["name"], p["version"], checksums[p["name"], p["version"]]) for p in graph}


def require_static_gate(data):
    address_space = 1 << 64
    if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01" or data[6] != 1:
        raise ValueError("gate must be ELF64 little-endian")
    if struct.unpack_from("<H", data, 16)[0] not in (2, 3) or struct.unpack_from("<I", data, 20)[0] != 1:
        raise ValueError("gate must be an executable ELF")
    if struct.unpack_from("<H", data, 18)[0] != 62:
        raise ValueError("gate must target x86_64")
    offset = struct.unpack_from("<Q", data, 32)[0]
    entry = struct.unpack_from("<Q", data, 24)[0]
    header_size, entry_size, count = struct.unpack_from("<HHH", data, 52)
    if header_size != 64 or entry_size != 56 or count in (0, 65535) or offset < 64 or offset + entry_size * count > len(data):
        raise ValueError("invalid gate program headers")
    loads = []
    dynamics = []
    for index in range(count):
        kind, flags, file_offset, address, _, file_size, memory_size, alignment = struct.unpack_from(
            "<IIQQQQQQ", data, offset + entry_size * index)
        if kind == 3:
            raise ValueError("gate has PT_INTERP and could load manager LD_PRELOAD")
        if file_offset > len(data) or file_size > len(data) - file_offset:
            raise ValueError("invalid gate program header file range")
        if address + memory_size > address_space:
            raise ValueError("invalid gate program header virtual range")
        if kind == 1:
            if memory_size < file_size or alignment not in (0, 1) and (alignment & (alignment - 1)) != 0:
                raise ValueError("invalid gate load segment")
            if alignment > 1 and (address - file_offset) % alignment:
                raise ValueError("invalid gate load alignment")
            loads.append((address, file_offset, file_size, flags))
        elif kind == 2:
            dynamics.append((address, file_offset, file_size))
    if not any(flags & 1 and start <= entry < start + size
               for start, _, size, flags in loads):
        raise ValueError("gate entry is outside executable PT_LOAD file range")
    if len(dynamics) > 1:
        raise ValueError("multiple PT_DYNAMIC segments")
    for address, file_offset, size in dynamics:
        if size == 0 or size % 16 or not any(
            start <= address and address + size <= start + load_size
            and file_offset == load_offset + address - start
            for start, load_offset, load_size, _ in loads
        ):
            raise ValueError("invalid gate PT_DYNAMIC range")
        entries = [struct.unpack_from("<QQ", data, file_offset + position)
                   for position in range(0, size, 16)]
        if (0, 0) not in entries or any(entry != (0, 0) for entry in entries[entries.index((0, 0)) + 1:]):
            raise ValueError("unterminated or malformed gate PT_DYNAMIC")
        forbidden = {1, 14, 15, 29, 0x6ffffefb, 0x6ffffefc, 0x7ffffffd, 0x7fffffff}
        if any(tag in forbidden for tag, _ in entries):
            raise ValueError("gate has DT_NEEDED or other dynamic loader dependency")


def open_directory(path):
    """Walk from / without following any path component."""
    absolute = Path(os.path.abspath(path))
    fd = os.open("/", DIR_FLAGS)
    try:
        for component in absolute.parts[1:]:
            child = os.open(component, DIR_FLAGS, dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except Exception:
        os.close(fd)
        raise


def read_regular_nofollow(path, *, max_bytes=None):
    parent_fd = open_directory(path.parent)
    try:
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=parent_fd)
        with os.fdopen(fd, "rb") as stream:
            info = os.fstat(stream.fileno())
            if not stat.S_ISREG(info.st_mode):
                raise ValueError(f"non-regular source: {path}")
            if max_bytes is not None and info.st_size > max_bytes:
                raise ValueError(f"source exceeds byte limit: {path}")
            data = stream.read(max_bytes + 1 if max_bytes is not None else -1)
            if max_bytes is not None and len(data) > max_bytes:
                raise ValueError(f"source exceeds byte limit: {path}")
            return data
    finally:
        os.close(parent_fd)


def open_stage_parent(root_fd, relative_path, *, create):
    fd = os.dup(root_fd)
    try:
        for component in PurePosixPath(relative_path).parts[:-1]:
            try:
                child = os.open(component, DIR_FLAGS, dir_fd=fd)
            except FileNotFoundError:
                if not create:
                    raise
                os.mkdir(component, 0o755, dir_fd=fd)
                child = os.open(component, DIR_FLAGS, dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except Exception:
        os.close(fd)
        raise


def verify_created(dest, root_fd, created):
    current_root = open_directory(dest)
    try:
        current = os.fstat(current_root)
        pinned = os.fstat(root_fd)
        if (current.st_dev, current.st_ino) != (pinned.st_dev, pinned.st_ino):
            raise ValueError("destination root substituted")
    finally:
        os.close(current_root)
    for parent_fd, name, identity, relative_path in created:
        check_parent = open_stage_parent(root_fd, relative_path, create=False)
        try:
            found = os.stat(name, dir_fd=check_parent, follow_symlinks=False)
            if (found.st_dev, found.st_ino) != identity or not stat.S_ISREG(found.st_mode):
                raise ValueError(f"staged path substituted: {relative_path}")
        finally:
            os.close(check_parent)


def write_inventory(path, inventory):
    out = Path(path)
    parent_fd = open_directory(out.parent)
    created = None
    try:
        fd = os.open(out.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                     0o644, dir_fd=parent_fd)
        created = os.fstat(fd)
        with os.fdopen(fd, "w") as stream:
            stream.write(json.dumps(inventory, indent=2) + "\n")
    except Exception:
        if created is not None:
            try:
                found = os.stat(out.name, dir_fd=parent_fd, follow_symlinks=False)
                if (found.st_dev, found.st_ino) == (created.st_dev, created.st_ino):
                    os.unlink(out.name, dir_fd=parent_fd)
            except FileNotFoundError:
                pass
        raise
    finally:
        os.close(parent_fd)


def regular_nofollow(path):
    parts = list(path.parents)[::-1] + [path]
    for component in parts:
        info = component.lstat()
        if component != path and not stat.S_ISDIR(info.st_mode):
            raise ValueError(f"non-directory or symlink parent: {component}")
        if component == path and not stat.S_ISREG(info.st_mode):
            raise ValueError(f"non-regular source: {component}")


def relative(value):
    path = PurePosixPath(value)
    if not value or path.is_absolute() or any(p in (".", "..", "") for p in value.split("/")):
        raise ValueError(f"invalid relative path: {value}")
    return path


def stage(args):
    source, binary, dest, manifest = map(Path, (args.source, args.binary, args.dest, args.manifest))
    if not dest.is_dir() or dest.is_symlink() or dest.resolve() == Path("/"):
        raise ValueError("destination must be an existing disposable directory")
    for parent in [dest, *dest.parents]:
        if parent.is_symlink():
            raise ValueError(f"symlink destination ancestor: {parent}")
    regular_nofollow(manifest)
    rows = []
    seen = set()
    for line in read_regular_nofollow(manifest).decode("utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 3:
            raise ValueError("inventory needs source, destination, mode")
        src, dst, mode = fields
        if src not in ("@binary", "@gate-binary", "@notices", "@notice-inventory"):
            relative(src)
        relative(dst)
        if not dst.startswith(PREFIXES) or dst in seen:
            raise ValueError(f"unowned or duplicate destination: {dst}")
        reserved = {"@notices": NOTICE_PATH,
                    "@notice-inventory": NOTICE_INVENTORY_PATH}
        if (src in reserved and dst != reserved[src]) or (dst in reserved.values() and src not in reserved):
            raise ValueError(f"invalid notice destination/source: {dst}")
        executable_paths = ("usr/bin/archy", "usr/lib/archy/archy-service-gate")
        if mode not in ("0644", "0755") or (dst in executable_paths) != (mode == "0755"):
            raise ValueError(f"invalid mode: {dst}")
        if (src == "@gate-binary") != (dst == "usr/lib/archy/archy-service-gate"):
            raise ValueError("gate binary must use fixed destination")
        seen.add(dst)
        special = {"@binary": binary,
                   "@gate-binary": getattr(args, "gate_binary", None),
                   "@notices": getattr(args, "notices_file", None),
                   "@notice-inventory": getattr(args, "notice_inventory_file", None)}
        if src in special and special[src] is None:
            raise ValueError(f"missing generated notice input: {src}")
        origin = Path(special[src]) if src in special else source / src
        regular_nofollow(origin)
        target = dest / dst
        for parent in reversed(target.parents):
            if parent == dest.parent:
                break
            if parent.exists() or parent.is_symlink():
                if parent.is_symlink() or not parent.is_dir():
                    raise ValueError(f"unsafe destination parent: {parent}")
        if target.exists() or target.is_symlink():
            raise ValueError(f"destination collision: {dst}")
        rows.append((origin, target, mode, dst, src))
    if not rows or "usr/bin/archy" not in seen or "usr/lib/archy/archy-service-gate" not in seen or "usr/share/licenses/archy/LICENSE" not in seen:
        raise ValueError("missing required core paths")
    if NOTICE_PATH not in seen or NOTICE_INVENTORY_PATH not in seen:
        raise ValueError("missing required notice bundle/inventory paths")
    notices = read_regular_nofollow(Path(args.notices_file), max_bytes=NOTICE_LIMIT)
    inventory_bytes = read_regular_nofollow(Path(args.notice_inventory_file), max_bytes=NOTICE_INVENTORY_LIMIT)
    metadata = json.loads(inventory_bytes)
    if not isinstance(metadata, dict) or metadata.get("bundle_sha256") != hashlib.sha256(notices).hexdigest():
        raise ValueError("notice bundle/inventory checksum mismatch")
    crates = metadata.get("crates")
    if not isinstance(crates, list) or metadata.get("crate_count") != len(crates):
        raise ValueError("invalid notice crate inventory")
    try:
        covered = {(p["name"], p["version"], p["source_sha256"]) for p in crates}
    except (KeyError, TypeError) as error:
        raise ValueError("invalid notice crate inventory") from error
    if len(covered) != len(crates) or covered != locked_graph(source):
        raise ValueError("notice dependency coverage mismatch")
    captured = {NOTICE_PATH: notices, NOTICE_INVENTORY_PATH: inventory_bytes}
    for origin, _, _, dst, src in rows:
        if dst not in captured:
            captured[dst] = read_regular_nofollow(origin)
        if src == "@gate-binary":
            require_static_gate(captured[dst])
    inventory = []
    with ExitStack() as stack:
        root_fd = open_directory(dest)
        stack.callback(os.close, root_fd)
        created = []
        try:
            for origin, target, mode, dst, src in rows:
                data = captured[dst]
                parent_fd = open_stage_parent(root_fd, dst, create=True)
                stack.callback(os.close, parent_fd)
                fd = os.open(target.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                             int(mode, 8), dir_fd=parent_fd)
                own = os.fstat(fd)
                created.append((parent_fd, target.name, (own.st_dev, own.st_ino), dst))
                with os.fdopen(fd, "wb") as out:
                    out.write(data)
                    os.fchmod(out.fileno(), int(mode, 8))
                verify_created(dest, root_fd, created)
                inventory.append({"path": dst, "mode": mode, "sha256": hashlib.sha256(data).hexdigest()})
            inventory.sort(key=lambda item: item["path"])
            verify_created(dest, root_fd, created)
            if args.inventory:
                write_inventory(args.inventory, inventory)
        except Exception:
            for parent_fd, name, identity, _ in reversed(created):
                try:
                    found = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
                    if (found.st_dev, found.st_ino) == identity and stat.S_ISREG(found.st_mode):
                        os.unlink(name, dir_fd=parent_fd)
                except FileNotFoundError:
                    pass
            raise
    print(json.dumps(inventory, indent=2))


def recipe(args):
    archive = Path(os.path.abspath(args.archive))
    regular_nofollow(archive)
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]{0,100}", args.source_id):
        raise ValueError("invalid source identity")
    if not re.fullmatch(r"[0-9a-f]{64}", args.sha256):
        raise ValueError("invalid SHA-256")
    archive_bytes = read_regular_nofollow(archive)
    if hashlib.sha256(archive_bytes).hexdigest() != args.sha256:
        raise ValueError("source archive digest mismatch")
    with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:*") as tf:
        members = tf.getmembers()
        if not members or any(m.issym() or m.islnk() or not (m.isfile() or m.isdir()) for m in members):
            raise ValueError("archive has link or special entry")
        by_name = {}
        for member in members:
            name = member.name.rstrip("/")
            relative(name)
            normalized = str(PurePosixPath(name))
            if normalized in by_name:
                raise ValueError(f"duplicate archive path: {normalized}")
            by_name[normalized] = member
        for name in by_name:
            for prefix in PurePosixPath(name).parents:
                if str(prefix) in by_name and by_name[str(prefix)].isfile():
                    raise ValueError(f"file/directory prefix collision: {name}")
        names = list(by_name)
        roots = {name.split("/")[0] for name in names}
        if len(roots) != 1:
            raise ValueError("archive needs one source root")
        root = roots.pop()
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", root):
            raise ValueError("unsafe archive root")
        required = (f"{root}/Cargo.lock", f"{root}/packaging/arch/files.txt",
                    f"{root}/packaging/arch/package.py", f"{root}/packaging/arch/notices.py",
                    f"{root}/packaging/arch/notice-inputs.json")
        if any(name not in by_name or not by_name[name].isfile() for name in required):
            raise ValueError("archive lacks regular locked package sources")
        manifest_member = by_name[f"{root}/packaging/arch/files.txt"]
        if manifest_member.size > NOTICE_INVENTORY_LIMIT:
            raise ValueError("archive package manifest exceeds byte limit")
        manifest_bytes = tf.extractfile(manifest_member).read(NOTICE_INVENTORY_LIMIT + 1)
        if len(manifest_bytes) > NOTICE_INVENTORY_LIMIT:
            raise ValueError("archive package manifest exceeds byte limit")
        manifest_lines = manifest_bytes.decode("utf-8").splitlines()
        required_notices = (f"@notices\t{NOTICE_PATH}\t0644",
                            f"@notice-inventory\t{NOTICE_INVENTORY_PATH}\t0644",
                            "@gate-binary\tusr/lib/archy/archy-service-gate\t0755")
        if any(manifest_lines.count(line) != 1 for line in required_notices):
            raise ValueError("archive package manifest lacks required notice pair")
    version = args.source_id.removeprefix("v").replace("-", "_")
    if not re.fullmatch(r"[0-9][A-Za-z0-9._+]*", version):
        raise ValueError("source identity cannot form pkgver")
    # The exact local archive is the sole package source; makepkg verifies its digest.
    content = f'''# Generated from identified source {args.source_id}; SHA-256 verified before generation.
pkgname=archy
pkgver={version}
pkgrel=1
pkgdesc='Local-first architectural workflow core CLI (backend runtimes separate)'
arch=('x86_64')
url='https://github.com/butahlecoq/Archy'
license=('MIT')
options=('!debug')
depends=('glibc' 'libgcc')
optdepends=('archy-runtime: private backend runtimes and isolation' 'systemd: user-service supervisor')
makedepends=('cargo' 'python')
source=({shlex.quote('archy-source.tar.gz::file://' + str(archive))})
sha256sums=('{args.sha256}')

build() {{
  cd "$srcdir/{root}"
  export RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true
  export CARGO_TARGET_DIR="$srcdir/archy-target"
  mkdir -p "$srcdir/archy-notices"
  python3 packaging/arch/notices.py --source . --vendor vendor \\
    --output-dir "$srcdir/archy-notices"
  cargo build --offline --locked --release -p archy-cli
  cargo rustc --offline --locked --release -p archy-service-gate --bin archy-service-gate -- -C target-feature=+crt-static
}}

package() {{
  cd "$srcdir/{root}"
  python3 packaging/arch/package.py stage --source . \\
    --binary "$srcdir/archy-target/release/archy" \\
    --gate-binary "$srcdir/archy-target/release/archy-service-gate" \\
    --manifest packaging/arch/files.txt --dest "$pkgdir" \\
    --notices-file "$srcdir/archy-notices/THIRD-PARTY-NOTICES.txt" \\
    --notice-inventory-file "$srcdir/archy-notices/third-party-notices.json"
}}
'''
    output = Path(args.output)
    if output.exists() or output.is_symlink():
        raise ValueError("recipe output collision")
    output.write_text(content)


def main():
    parser = argparse.ArgumentParser()
    subs = parser.add_subparsers(dest="command", required=True)
    s = subs.add_parser("stage")
    for flag in ("source", "binary", "manifest", "dest"):
        s.add_argument(f"--{flag}", required=True)
    s.add_argument("--inventory")
    s.add_argument("--gate-binary")
    s.add_argument("--notices-file")
    s.add_argument("--notice-inventory-file")
    r = subs.add_parser("recipe")
    for flag in ("archive", "source-id", "sha256", "output"):
        r.add_argument(f"--{flag}", required=True)
    args = parser.parse_args()
    try:
        (stage if args.command == "stage" else recipe)(args)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError, tarfile.TarError) as error:
        print(f"package: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
