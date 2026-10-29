#!/usr/bin/env python3
"""Preview or apply a user-owned local Archy plugin transaction."""
import argparse
import ctypes
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import uuid

PLUGIN_ID = "archy.status"
FILES = {"manifest.json", "Main.qml", "DoctorModel.js", "DoctorProcess.qml",
         "DesktopModel.js", "DesktopProcess.qml", "OpenModel.js", "OpenProcess.qml"}
MANIFEST = {"schemaVersion": 1, "id": PLUGIN_ID, "name": "Archy", "version": "0.1.0",
            "kinds": ["panel"], "entryPoints": {"panel": "Main.qml"}}
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
_RENAME_NOREPLACE = 1
_libc = ctypes.CDLL(None, use_errno=True)
_libc.renameat2.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
_libc.renameat2.restype = ctypes.c_int


class Refused(Exception):
    pass


class PublishedStateError(OSError):
    """State was linked, but final temporary-file cleanup failed."""


def digest(data):
    return hashlib.sha256(data).hexdigest()


def rename_exclusive(src_fd, src, dst_fd, dst):
    result = _libc.renameat2(src_fd, os.fsencode(src), dst_fd, os.fsencode(dst), _RENAME_NOREPLACE)
    if result:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), dst)


def open_dir(path, create=False):
    if not path.is_absolute():
        raise Refused(f"directory is not absolute: {path}")
    fd = os.open("/", DIR_FLAGS)
    try:
        for part in path.parts[1:]:
            try:
                next_fd = os.open(part, DIR_FLAGS, dir_fd=fd)
            except FileNotFoundError:
                if not create:
                    raise
                os.mkdir(part, 0o700, dir_fd=fd)
                next_fd = os.open(part, DIR_FLAGS, dir_fd=fd)
            os.close(fd)
            fd = next_fd
        return fd
    except BaseException:
        os.close(fd)
        raise


def read_file(fd, name):
    try:
        file_fd = os.open(name, FILE_FLAGS, dir_fd=fd)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise Refused(f"not a regular file: {name}") from error
        raise
    try:
        if not stat.S_ISREG(os.fstat(file_fd).st_mode):
            raise Refused(f"not a regular file: {name}")
        with os.fdopen(file_fd, "rb", closefd=False) as file:
            return file.read()
    finally:
        os.close(file_fd)


def inventory_fd(fd):
    names = set(os.listdir(fd))
    if names != FILES:
        raise Refused(f"plugin inventory differs: {sorted(names ^ FILES)}")
    if json.loads(read_file(fd, "manifest.json")) != MANIFEST:
        raise Refused("unsupported plugin manifest")
    return {name: digest(read_file(fd, name)) for name in sorted(FILES)}


def inventory(root):
    fd = open_dir(root)
    try:
        return inventory_fd(fd)
    finally:
        os.close(fd)


def state_read_fd(fd):
    try:
        value = json.loads(read_file(fd, "install.json"))
    except FileNotFoundError:
        return None
    try:
        if (not isinstance(value, dict) or set(value) != {"schema_version", "plugin_id", "files", "device", "inode"}
                or value["schema_version"] != 2 or value["plugin_id"] != PLUGIN_ID
                or type(value["device"]) is not int or value["device"] < 0
                or type(value["inode"]) is not int or value["inode"] < 0
                or not isinstance(value["files"], dict) or set(value["files"]) != FILES
                or not all(isinstance(h, str) and re.fullmatch(r"[0-9a-f]{64}", h)
                           for h in value["files"].values())):
            raise ValueError("unsupported state")
        return value
    except (ValueError, TypeError) as error:
        raise Refused(f"unrecognized ownership state: {error}") from error


def atomic_state_fd(fd, value):
    temp = f".install-{uuid.uuid4().hex}.json"
    file_fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=fd)
    published = False
    try:
        with os.fdopen(file_fd, "w", encoding="utf8", closefd=False) as file:
            json.dump(value, file, sort_keys=True)
            file.flush()
            os.fsync(file_fd)
        os.link(temp, "install.json", src_dir_fd=fd, dst_dir_fd=fd, follow_symlinks=False)
        published = True
    finally:
        try:
            os.close(file_fd)
            os.unlink(temp, dir_fd=fd)
        except OSError as error:
            if published:
                raise PublishedStateError(error.errno, error.strerror) from error
            raise


def atomic_state(path, value):
    fd = open_dir(path.parent)
    try:
        atomic_state_fd(fd, value)
    finally:
        os.close(fd)


def same_inode(left, right):
    a, b = os.fstat(left), os.fstat(right)
    return (a.st_dev, a.st_ino) == (b.st_dev, b.st_ino)


def entry_dir(parent_fd, name):
    try:
        return os.open(name, DIR_FLAGS, dir_fd=parent_fd)
    except FileNotFoundError:
        return None
    except (NotADirectoryError, OSError) as error:
        if error.errno in (errno.ENOTDIR, errno.ELOOP):
            raise Refused(f"directory collision: {name}") from error
        raise


def rollback_install(parent_fd, state_fd, owned_fd, expected_state, state_published):
    """Park our tree until its ownership state can be removed."""
    parked = f".archy-parked-{uuid.uuid4().hex}"
    rename_exclusive(parent_fd, PLUGIN_ID, parent_fd, parked)
    moved_fd = None
    restore = True
    try:
        moved_fd = entry_dir(parent_fd, parked)
        if moved_fd is None or not same_inode(moved_fd, owned_fd):
            raise Refused("plugin destination changed during transaction; ownership state preserved")
        if state_published:
            current_state = state_read_fd(state_fd)
            if current_state != expected_state:
                raise Refused("ownership state changed during rollback; preserve the plugin")
            os.unlink("install.json", dir_fd=state_fd)
        restore = False
        for item in FILES:
            os.unlink(item, dir_fd=moved_fd)
        os.rmdir(parked, dir_fd=parent_fd)
    finally:
        try:
            if restore:
                rename_exclusive(parent_fd, parked, parent_fd, PLUGIN_ID)
        finally:
            if moved_fd is not None:
                os.close(moved_fd)


def run(source, home, apply=False, uninstall=False):
    source_hashes = None if uninstall else inventory(source)
    if not home.is_absolute() or home.stat().st_uid != os.geteuid():
        raise Refused("HOME must be absolute and owned by the invoking user")
    plugins = home / ".config/omarchy/plugins"
    state_dir = home / ".local/state/archy/omarchy"
    dest = plugins / PLUGIN_ID
    state_path = state_dir / "install.json"
    # Preview does not create directories. Missing roots have no state or plugin.
    try:
        plugins_fd = open_dir(plugins)
    except FileNotFoundError:
        plugins_fd = None
    try:
        state_fd = open_dir(state_dir)
    except FileNotFoundError:
        state_fd = None
    try:
        owned_fd = None
        state = state_read_fd(state_fd) if state_fd is not None else None
        owned_fd = entry_dir(plugins_fd, PLUGIN_ID) if plugins_fd is not None else None
        if owned_fd is not None:
            if not state:
                raise Refused("plugin destination exists without Archy ownership state")
            meta = os.fstat(owned_fd)
            if (meta.st_dev, meta.st_ino) != (state["device"], state["inode"]):
                raise Refused("plugin directory differs from Archy ownership state")
            if inventory_fd(owned_fd) != state["files"]:
                raise Refused("owned plugin has local edits; preserve it and the state")
        elif state:
            raise Refused("owned plugin is missing; preserve state for manual recovery")
        action = "uninstall" if uninstall else "install"
        if not uninstall and state and state["files"] == source_hashes:
            action = "already-installed"
        if not uninstall and state and state["files"] != source_hashes:
            raise Refused("plugin upgrade needs separate reviewed transaction")
        if uninstall and not state:
            action = "already-absent"
        result = {"action": action, "plugin_root": str(source), "destination": str(dest),
                  "files": state["files"] if uninstall and state else source_hashes, "applied": False}
        if not apply or action in ("already-installed", "already-absent"):
            return result
        if plugins_fd is None:
            plugins_fd = open_dir(plugins, create=True)
        if state_fd is None:
            state_fd = open_dir(state_dir, create=True)
        if uninstall:
            backup = f"removed-{uuid.uuid4().hex}"
            rename_exclusive(plugins_fd, PLUGIN_ID, state_fd, backup)
            moved_fd = None
            try:
                moved_fd = entry_dir(state_fd, backup)
                if moved_fd is None or not same_inode(moved_fd, owned_fd):
                    rename_exclusive(state_fd, backup, plugins_fd, PLUGIN_ID)
                    raise Refused("plugin destination changed during uninstall; ownership state preserved")
                # Check content once more after move, before removing ownership state.
                if inventory_fd(moved_fd) != state["files"]:
                    rename_exclusive(state_fd, backup, plugins_fd, PLUGIN_ID)
                    raise Refused("owned plugin changed during uninstall; ownership state preserved")
                os.unlink("install.json", dir_fd=state_fd)
            except Refused:
                if moved_fd is None:
                    rename_exclusive(state_fd, backup, plugins_fd, PLUGIN_ID)
                raise
            except OSError:
                if moved_fd is not None:
                    try:
                        rename_exclusive(state_fd, backup, plugins_fd, PLUGIN_ID)
                    except OSError:
                        pass
                raise
            finally:
                if moved_fd is not None:
                    os.close(moved_fd)
            result["backup"] = str(state_dir / backup)
        else:
            staged = f".archy-status-{uuid.uuid4().hex}"
            os.mkdir(staged, 0o700, dir_fd=plugins_fd)
            staged_fd = os.open(staged, DIR_FLAGS, dir_fd=plugins_fd)
            published = False
            state_published = False
            expected_state = None
            try:
                source_fd = open_dir(source)
                try:
                    for name in sorted(FILES):
                        data = read_file(source_fd, name)
                        file_fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                                          0o600, dir_fd=staged_fd)
                        try:
                            with os.fdopen(file_fd, "wb", closefd=False) as file:
                                file.write(data)
                        finally:
                            os.close(file_fd)
                finally:
                    os.close(source_fd)
                if inventory_fd(staged_fd) != source_hashes:
                    raise Refused("source changed while staging")
                rename_exclusive(plugins_fd, staged, plugins_fd, PLUGIN_ID)
                published = True
                try:
                    current_fd = entry_dir(plugins_fd, PLUGIN_ID)
                    try:
                        if current_fd is None or not same_inode(current_fd, staged_fd):
                            raise Refused("plugin destination changed before ownership publication")
                        meta = os.fstat(staged_fd)
                        expected_state = {"schema_version": 2, "plugin_id": PLUGIN_ID,
                                          "files": source_hashes, "device": meta.st_dev,
                                          "inode": meta.st_ino}
                        try:
                            atomic_state_fd(state_fd, expected_state)
                            state_published = True
                        except PublishedStateError:
                            state_published = True
                            raise
                        try:
                            final_fd = entry_dir(plugins_fd, PLUGIN_ID)
                        except Refused as error:
                            raise Refused("plugin destination changed after ownership publication") from error
                        try:
                            if final_fd is None or not same_inode(final_fd, staged_fd):
                                raise Refused("plugin destination changed after ownership publication")
                        finally:
                            if final_fd is not None:
                                os.close(final_fd)
                    finally:
                        if current_fd is not None:
                            os.close(current_fd)
                except Exception:
                    try:
                        rollback_install(plugins_fd, state_fd, staged_fd, expected_state, state_published)
                    except (OSError, Refused):
                        pass  # Preserve any changed tree; never recursively delete a path.
                    raise
            finally:
                if not published:
                    for name in os.listdir(staged_fd):
                        os.unlink(name, dir_fd=staged_fd)
                    os.rmdir(staged, dir_fd=plugins_fd)
                os.close(staged_fd)
        result["applied"] = True
        return result
    finally:
        if owned_fd is not None:
            os.close(owned_fd)
        if state_fd is not None:
            os.close(state_fd)
        if plugins_fd is not None:
            os.close(plugins_fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true", help="apply the previewed user-owned change")
    parser.add_argument("--uninstall", action="store_true", help="remove only the unchanged owned plugin")
    parser.add_argument("--plugin-root", type=Path, default=Path(__file__).resolve().parent / "plugin")
    args = parser.parse_args()
    try:
        result = run(args.plugin_root, Path.home(), args.apply, args.uninstall)
    except (Refused, OSError, ValueError) as error:
        print(json.dumps({"status": "refused", "reason": str(error)}))
        return 1
    print(json.dumps({"status": "ok", **result}, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
