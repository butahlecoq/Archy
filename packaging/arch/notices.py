#!/usr/bin/env python3
"""Admit locally verified third-party notice texts for the locked core CLI graph."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

from package import open_directory, read_regular_nofollow, relative


REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
NOTICE_NAME = re.compile(r"(?:LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)(?:[._-].*)?", re.I)
SHA = re.compile(r"[0-9a-f]{64}\Z")
GIT_ID = re.compile(r"[0-9a-f]{40}\Z")
REPOSITORY = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\Z")
SOURCE_PREFIX = "packaging/arch/notices-sources/"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked_hash(value, label):
    if not isinstance(value, str) or not SHA.fullmatch(value):
        raise ValueError(f"invalid {label} SHA-256")
    return value


def source_bytes(source, path):
    if not isinstance(path, str):
        raise ValueError("notice path must be a string")
    relative(path)
    return read_regular_nofollow(source / path)


def normal_registry_graph(metadata):
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    roots = [p["id"] for p in packages.values() if p["name"] == "archy-cli" and p.get("source") is None]
    if len(roots) != 1:
        raise ValueError("expected exactly one local archy-cli package")
    visited = set()
    pending = roots
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


def load_json(data, label):
    try:
        return json.loads(data)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"invalid {label}: {error}") from error


def generate(source, vendor, metadata, rust_version, output_dir):
    source, vendor, output_dir = map(Path, (source, vendor, output_dir))
    lock = tomllib.loads(read_regular_nofollow(source / "Cargo.lock").decode("utf-8"))
    locked = {}
    for package in lock["package"]:
        if package.get("source") == REGISTRY:
            key = (package["name"], package["version"])
            if key in locked:
                raise ValueError(f"duplicate locked registry package: {key}")
            locked[key] = package
    inputs = load_json(read_regular_nofollow(source / "packaging/arch/notice-inputs.json"), "notice inputs")
    if inputs.get("schema_version") != 1 or not isinstance(inputs.get("external"), list):
        raise ValueError("unsupported notice input schema")
    external = {}
    external_paths = set()
    for entry in inputs["external"]:
        key = (entry["name"], entry["version"])
        path = entry["path"]
        if (key, path) in external_paths:
            raise ValueError(f"duplicate external notice: {key}/{path}")
        external_paths.add((key, path))
        if not isinstance(path, str) or not path.startswith(SOURCE_PREFIX):
            raise ValueError(f"external notice outside owned sources: {path}")
        relative(path)
        if not isinstance(entry.get("source_url"), str) or not entry["source_url"].startswith("https://"):
            raise ValueError(f"missing primary source URL: {key}")
        checked_hash(entry["sha256"], "external notice")
        repository, commit = entry["repository"], entry["commit"]
        upstream_path, blob = entry["path_in_repository"], entry["git_blob"]
        if not isinstance(repository, str) or not REPOSITORY.fullmatch(repository):
            raise ValueError(f"invalid external repository: {key}")
        if not isinstance(commit, str) or not GIT_ID.fullmatch(commit):
            raise ValueError(f"invalid external commit: {key}")
        if not isinstance(blob, str) or not GIT_ID.fullmatch(blob):
            raise ValueError(f"invalid external Git blob: {key}")
        relative(upstream_path)
        expected_url = f"https://github.com/{repository}/blob/{commit}/{upstream_path}"
        if entry["source_url"] != expected_url:
            raise ValueError(f"external source URL mismatch: {key}")
        external.setdefault(key, []).append(entry)

    crates = []
    chunks = []
    graph = normal_registry_graph(metadata)
    graph_keys = {(p["name"], p["version"]) for p in graph}
    if external.keys() - graph_keys:
        raise ValueError(f"external notice outside graph: {sorted(external.keys() - graph_keys)}")
    missing = []
    for package in graph:
        name, version = package["name"], package["version"]
        key = (name, version)
        locked_package = locked.get(key)
        if not locked_package:
            raise ValueError(f"missing locked registry source: {key}")
        source_hash = checked_hash(locked_package.get("checksum"), "locked source")
        vendor_dir = vendor / f"{name}-{version}"
        checksum = load_json(read_regular_nofollow(vendor_dir / ".cargo-checksum.json"), "vendor checksum")
        if checksum.get("package") != source_hash:
            raise ValueError(f"source checksum mismatch: {key}")
        files = checksum.get("files")
        if not isinstance(files, dict):
            raise ValueError(f"invalid vendor files: {key}")
        names = sorted(n for n in files if "/" not in n and NOTICE_NAME.fullmatch(n))
        texts = []
        for filename in names:
            expected = checked_hash(files[filename], "vendor notice")
            data = read_regular_nofollow(vendor_dir / filename)
            if digest(data) != expected:
                raise ValueError(f"notice checksum mismatch: {key}/{filename}")
            texts.append({"path": filename, "sha256": expected, "origin": "vendor"})
            chunks.append((f"{name} {version}: {filename}", data))
        for entry in sorted(external.get(key, []), key=lambda value: value["path"]):
            data = source_bytes(source, entry["path"])
            if digest(data) != entry["sha256"]:
                raise ValueError(f"external notice checksum mismatch: {key}")
            blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
            if blob != entry["git_blob"]:
                raise ValueError(f"external Git blob mismatch: {key}")
            vcs = load_json(read_regular_nofollow(vendor_dir / ".cargo_vcs_info.json"), "vendor VCS info")
            if vcs.get("git", {}).get("sha1") != entry["commit"]:
                raise ValueError(f"external source commit mismatch: {key}")
            texts.append({"path": entry["path"], "sha256": entry["sha256"],
                          "origin": "primary", "repository": entry["repository"],
                          "commit": entry["commit"], "path_in_repository": entry["path_in_repository"],
                          "git_blob": entry["git_blob"], "source_url": entry["source_url"]})
            chunks.append((f"{name} {version}: {entry['path']}", data))
        if not texts:
            missing.append(f"{name} {version}")
        crates.append({"name": name, "version": version, "license": package.get("license"),
                       "source": REGISTRY, "source_sha256": source_hash, "texts": texts})
    if missing:
        raise ValueError(f"missing notice text for {len(missing)} of {len(graph)} locked normal crates: {', '.join(missing)}")

    rust = inputs.get("rust_std")
    if not isinstance(rust, dict) or rust.get("version") != rust_version or not rust.get("files"):
        raise ValueError("Rust std notice version/files mismatch")
    rust_files = []
    seen = set()
    for entry in rust["files"]:
        path = entry["path"]
        if path in seen or not path.startswith(f"{SOURCE_PREFIX}rust-{rust_version}/"):
            raise ValueError(f"duplicate or misplaced Rust std notice: {path}")
        seen.add(path)
        expected = checked_hash(entry["sha256"], "Rust std notice")
        data = source_bytes(source, path)
        if digest(data) != expected:
            raise ValueError(f"Rust std notice checksum mismatch: {path}")
        rust_files.append({"path": path, "sha256": expected})
        chunks.append((f"Rust standard library {rust_version}: {path}", data))
    bundle = b"".join((f"\n===== {label} =====\n".encode() + data + b"\n") for label, data in chunks)
    inventory = {"schema_version": 1, "graph": "archy-cli locked normal dependencies",
                 "crate_count": len(crates), "crates": crates,
                 "rust_std": {"version": rust_version, "files": rust_files},
                 "bundle_sha256": digest(bundle)}
    inventory_bytes = (json.dumps(inventory, indent=2, sort_keys=True) + "\n").encode()
    outputs = [("third-party-notices.json", inventory_bytes), ("THIRD-PARTY-NOTICES.txt", bundle)]
    parent_fd = open_directory(output_dir)
    created = []
    try:
        for filename, data in outputs:
            fd = os.open(filename, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                         0o644, dir_fd=parent_fd)
            identity = os.fstat(fd)
            created.append((filename, identity.st_dev, identity.st_ino))
            with os.fdopen(fd, "wb") as stream:
                stream.write(data)
    except Exception:
        for filename, device, inode in reversed(created):
            try:
                found = os.stat(filename, dir_fd=parent_fd, follow_symlinks=False)
                if (found.st_dev, found.st_ino) == (device, inode):
                    os.unlink(filename, dir_fd=parent_fd)
            except FileNotFoundError:
                pass
        raise
    finally:
        os.close(parent_fd)
    return output_dir / outputs[0][0], output_dir / outputs[1][0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True)
    parser.add_argument("--vendor", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    source = Path(args.source)
    try:
        env = dict(os.environ, CARGO_NET_OFFLINE="true", RUSTUP_AUTO_INSTALL="0")
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--offline", "--locked", "--format-version", "1",
             "--filter-platform", "x86_64-unknown-linux-gnu"], cwd=source, env=env))
        rust_version = subprocess.check_output(["rustc", "--version"], env=env, text=True).split()[1]
        generate(source, Path(args.vendor), metadata, rust_version, Path(args.output_dir))
    except (OSError, ValueError, KeyError, UnicodeError, subprocess.CalledProcessError) as error:
        print(f"notices: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
