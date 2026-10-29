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
import sys
import tarfile


PREFIXES = ("usr/bin/", "usr/share/archy/", "usr/share/licenses/archy/")
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW


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


def read_regular_nofollow(path):
    parent_fd = open_directory(path.parent)
    try:
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=parent_fd)
        with os.fdopen(fd, "rb") as stream:
            if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
                raise ValueError(f"non-regular source: {path}")
            return stream.read()
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
        if src != "@binary":
            relative(src)
        relative(dst)
        if not dst.startswith(PREFIXES) or dst in seen:
            raise ValueError(f"unowned or duplicate destination: {dst}")
        if mode not in ("0644", "0755") or (dst == "usr/bin/archy") != (mode == "0755"):
            raise ValueError(f"invalid mode: {dst}")
        seen.add(dst)
        origin = binary if src == "@binary" else source / src
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
        rows.append((origin, target, mode, dst))
    if not rows or "usr/bin/archy" not in seen or "usr/share/licenses/archy/LICENSE" not in seen:
        raise ValueError("missing required core paths")
    inventory = []
    with ExitStack() as stack:
        root_fd = open_directory(dest)
        stack.callback(os.close, root_fd)
        created = []
        try:
            for origin, target, mode, dst in rows:
                data = read_regular_nofollow(origin)
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
                    f"{root}/packaging/arch/package.py")
        if any(name not in by_name or not by_name[name].isfile() for name in required):
            raise ValueError("archive lacks regular locked package sources")
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
makedepends=('cargo' 'python')
source=({shlex.quote('archy-source.tar.gz::file://' + str(archive))})
sha256sums=('{args.sha256}')

build() {{
  cd "$srcdir/{root}"
  export RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true
  export CARGO_TARGET_DIR="$srcdir/archy-target"
  cargo build --offline --locked --release -p archy-cli
}}

package() {{
  cd "$srcdir/{root}"
  python3 packaging/arch/package.py stage --source . \\
    --binary "$srcdir/archy-target/release/archy" \\
    --manifest packaging/arch/files.txt --dest "$pkgdir"
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
    r = subs.add_parser("recipe")
    for flag in ("archive", "source-id", "sha256", "output"):
        r.add_argument(f"--{flag}", required=True)
    args = parser.parse_args()
    try:
        (stage if args.command == "stage" else recipe)(args)
    except (OSError, ValueError, tarfile.TarError) as error:
        print(f"package: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
