#!/usr/bin/env python3
"""Admit an offline, pinned notice inventory for proposed authority binaries.

This produces review artifacts only. The Arch package manifest does not stage authority.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tomllib

from notices import (NOTICE_NAME, REGISTRY, SOURCE_PREFIX, checked_hash, digest,
                     load_json, source_bytes)
from package import normal_registry_graph, open_directory, read_regular_nofollow


def archive_files(cache, name, version, expected):
    archive = cache / f"{name}-{version}.crate"
    data = read_regular_nofollow(archive)
    if digest(data) != expected:
        raise ValueError(f"crate archive checksum mismatch: {name} {version}")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as stream:
        for member in stream:
            prefix = f"{name}-{version}/"
            if not member.name.startswith(prefix) or not member.isfile():
                raise ValueError(f"unexpected crate archive member: {member.name}")
            path = member.name[len(prefix):]
            if not path or path in files or any(part in ("", ".", "..") for part in path.split("/")):
                raise ValueError(f"invalid crate archive path: {member.name}")
            files[path] = stream.extractfile(member).read()
    return files


def normal_ids(metadata, roots):
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    pending = [p["id"] for p in packages.values() if p["name"] in roots and p.get("source") is None]
    if len(pending) != len(roots):
        raise ValueError("authority root missing")
    seen = set()
    while pending:
        pid = pending.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for edge in nodes[pid]["deps"]:
            if any(kind.get("kind") in (None, "normal") for kind in edge["dep_kinds"]):
                pending.append(edge["pkg"])
    return [packages[pid] for pid in seen]


def patched_zbus(source, metadata, pin):
    matches = [p for p in normal_ids(metadata, ("archy-authority",)) if p["name"] == "zbus"]
    if len(matches) != 1 or matches[0]["source"] is not None or matches[0]["version"] != pin["version"]:
        raise ValueError("patched zbus is absent from authority normal graph")
    local = source / "authority/vendor/zbus-5.19.0"
    if Path(matches[0]["manifest_path"]).resolve() != (local / "Cargo.toml").resolve():
        raise ValueError("authority zbus source path mismatch")
    original = archive_files(source / "packaging/arch/source-archives", "zbus", pin["version"],
                             checked_hash(pin["original_crate_sha256"], "zbus original archive"))
    actual_paths = {p.relative_to(local).as_posix() for p in local.rglob("*") if p.is_file()}
    actual_paths.discard(".cargo-ok")  # Cargo extraction marker, not source
    expected_paths = set(original) | set(pin["added_files"])
    if actual_paths != expected_paths:
        raise ValueError(f"patched zbus source file set mismatch: missing={sorted(expected_paths - actual_paths)}, extra={sorted(actual_paths - expected_paths)}")
    changed = {}
    for path in sorted(actual_paths):
        data = read_regular_nofollow(local / path)
        if path in original and data != original[path] or path not in original:
            changed[path] = digest(data)
    expected_changed = pin["changed_files"] | pin["added_files"]
    if changed != expected_changed:
        raise ValueError("patched zbus source/provenance checksum mismatch")
    license_text = read_regular_nofollow(local / "LICENSE")
    if license_text != original["LICENSE"] or digest(license_text) != checked_hash(pin["license_sha256"], "zbus license"):
        raise ValueError("patched zbus license mismatch")
    if matches[0].get("license") != "MIT":
        raise ValueError("patched zbus license declaration changed")
    return {"name": "zbus", "version": pin["version"], "license": "MIT",
            "source": "patched-local", "upstream_crate_sha256": pin["original_crate_sha256"],
            "path": "authority/vendor/zbus-5.19.0", "changed_files": pin["changed_files"],
            "added_files": pin["added_files"],
            "texts": [{"path": "LICENSE", "sha256": pin["license_sha256"], "origin": "patched-local"}]}, license_text


def generate(source, cache, metadata, rust_version, output_dir):
    source, cache, output_dir = map(Path, (source, cache, output_dir))
    pin = load_json(read_regular_nofollow(source / "packaging/arch/authority-notice-inputs.json"), "authority pins")
    if pin.get("schema_version") != 1 or pin.get("target") != "x86_64-unknown-linux-gnu":
        raise ValueError("unsupported authority notice pins")
    lock = tomllib.loads(read_regular_nofollow(source / "Cargo.lock").decode())
    locked = {(p["name"], p["version"]): p["checksum"] for p in lock["package"] if p.get("source") == REGISTRY}
    graph = normal_registry_graph(metadata, ("archy-authority",))
    local = sorted(p["name"] for p in normal_ids(metadata, ("archy-authority",)) if p["source"] is None)
    if local != pin["local_packages"]:
        raise ValueError(f"unreviewed local authority dependency: {local}")
    cli = normal_registry_graph(metadata)
    cli_keys = {(p["name"], p["version"]) for p in cli}
    added = {(p["name"], p["version"]) for p in graph} - cli_keys
    pins = {(p["name"], p["version"]): p for p in pin["additional_registry_crates"]}
    if len(pins) != len(pin["additional_registry_crates"]) or set(pins) != added:
        raise ValueError("authority dependency pin coverage mismatch")
    inputs = load_json(read_regular_nofollow(source / "packaging/arch/notice-inputs.json"), "CLI notice inputs")
    external = {}
    for entry in inputs["external"]:
        key = (entry["name"], entry["version"])
        if key not in cli_keys or not entry["path"].startswith(SOURCE_PREFIX):
            raise ValueError(f"external notice outside CLI graph: {key}")
        if entry["path"] in {e["path"] for e in external.get(key, [])}:
            raise ValueError(f"duplicate external notice: {key}")
        external.setdefault(key, []).append(entry)
    crates, chunks = [], []
    for package in graph:
        name, version = package["name"], package["version"]
        key = (name, version)
        source_hash = checked_hash(locked[key], "locked source")
        files = archive_files(cache, name, version, source_hash)
        texts = []
        notice_files = {path: data for path, data in files.items()
                        if NOTICE_NAME.fullmatch(path.rsplit("/", 1)[-1])}
        if key in pins:
            actual = {"name": name, "version": version, "source_sha256": source_hash,
                      "license": package.get("license"),
                      "texts": {path: digest(data) for path, data in notice_files.items()}}
            if actual != pins[key]:
                raise ValueError(f"authority license/source pin mismatch: {name} {version}")
        for path, data in sorted(notice_files.items()):
            texts.append({"path": path, "sha256": digest(data), "origin": "locked-crate-archive"})
            chunks.append((f"{name} {version}: {path}", data))
        for entry in sorted(external.get(key, []), key=lambda e: e["path"]):
            data = source_bytes(source, entry["path"])
            if digest(data) != checked_hash(entry["sha256"], "external notice"):
                raise ValueError(f"external notice checksum mismatch: {key}")
            blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
            vcs = json.loads(files[".cargo_vcs_info.json"])
            if (blob != entry["git_blob"] or vcs["git"]["sha1"] != entry["commit"]
                    or entry["source_url"] != f"https://github.com/{entry['repository']}/blob/{entry['commit']}/{entry['path_in_repository']}"):
                raise ValueError(f"external notice provenance mismatch: {key}")
            texts.append({"path": entry["path"], "sha256": digest(data), "origin": "primary",
                          "source_url": entry["source_url"], "git_blob": blob})
            chunks.append((f"{name} {version}: {entry['path']}", data))
        if not texts:
            raise ValueError(f"missing notice text: {name} {version}")
        crates.append({"name": name, "version": version, "license": package.get("license"),
                       "source": REGISTRY, "source_sha256": source_hash, "texts": texts})
    zbus, zbus_license = patched_zbus(source, metadata, pin["zbus"])
    crates.append(zbus)
    chunks.append(("zbus 5.19.0 patched-local: LICENSE", zbus_license))
    rust = inputs["rust_std"]
    if rust["version"] != rust_version or not rust["files"]:
        raise ValueError("Rust std notice version mismatch")
    for entry in rust["files"]:
        data = source_bytes(source, entry["path"])
        if digest(data) != checked_hash(entry["sha256"], "Rust std notice"):
            raise ValueError(f"Rust std notice checksum mismatch: {entry['path']}")
        chunks.append((f"Rust standard library {rust_version}: {entry['path']}", data))
    bundle = b"".join(f"\n===== {label} =====\n".encode() + data + b"\n" for label, data in chunks)
    inventory = {"schema_version": 1, "graph": "archy-authority locked x86_64 Linux normal dependencies",
                 "crate_count": len(crates), "crates": crates, "rust_std": rust,
                 "bundle_sha256": digest(bundle)}
    outputs = {"authority-third-party-notices.json": (json.dumps(inventory, indent=2, sort_keys=True) + "\n").encode(),
               "AUTHORITY-THIRD-PARTY-NOTICES.txt": bundle}
    if (digest(outputs["authority-third-party-notices.json"]) != checked_hash(pin["inventory_sha256"], "authority inventory")
            or digest(bundle) != checked_hash(pin["bundle_sha256"], "authority bundle")):
        raise ValueError("authority reviewed notice output mismatch")
    parent_fd = open_directory(output_dir)
    try:
        for name, data in outputs.items():
            try:
                existing = read_regular_nofollow(output_dir / name)
            except FileNotFoundError:
                existing = None
            if existing is not None and existing != data:
                raise ValueError(f"stale authority notice output: {name}")
        for name, data in outputs.items():
            if (output_dir / name).exists():
                continue
            fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644, dir_fd=parent_fd)
            with os.fdopen(fd, "wb") as stream:
                stream.write(data)
    finally:
        os.close(parent_fd)
    return tuple(output_dir / name for name in outputs)


def main():
    parser = argparse.ArgumentParser()
    for flag in ("source", "cache", "output-dir"):
        parser.add_argument(f"--{flag}", required=True)
    args = parser.parse_args()
    try:
        env = dict(os.environ, CARGO_NET_OFFLINE="true", RUSTUP_AUTO_INSTALL="0")
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--offline", "--locked", "--format-version", "1",
             "--filter-platform", "x86_64-unknown-linux-gnu"], cwd=args.source, env=env))
        rust_version = subprocess.check_output(["rustc", "--version"], env=env, text=True).split()[1]
        generate(args.source, args.cache, metadata, rust_version, args.output_dir)
    except (OSError, ValueError, KeyError, TypeError, UnicodeError, tarfile.TarError,
            subprocess.CalledProcessError) as error:
        print(f"authority notices: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
