#!/usr/bin/env python3
"""Reproduce the bounded util-linux v2.42.4 libuuid source notice inventory."""

import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys

COMMIT = "d76cbf8f13e65ff657344f7f6a90042cf755ba59"
TREE = "2b1bca56bae90356243e14c80b947fc625d9a897"
TAG = "a1d2b6980b84f38b389a6d6dda4a9623f47d13be"

UUID_C = "clear compare copy gen_uuid isnull pack parse predefined unpack unparse uuid_time".split()
SHARED_C = "randutils md5 sha1".split()
UUID_HEADERS = "uuidP uuidd uuid".split()
INCLUDE_HEADERS = "all-io c list md5 nls randutils sha1 strutils timeutils".split()


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), *args])


def source(repo, path):
    return git(repo, "show", f"{COMMIT}:{path}")


def notice(path, data):
    header = re.match(rb"/\*.*?\*/", data, re.S)
    if not header:
        raise ValueError(f"missing leading notice: {path}")
    header = header.group().decode("utf-8")
    if "%Begin-Header%" in header and "%End-Header%" in header:
        return "BSD-3-Clause-header"
    if "SPDX-License-Identifier: BSD-3-Clause" in header:
        return "BSD-3-Clause-SPDX"
    if "SPDX-License-Identifier: LGPL-2.1-or-later" in header:
        return "LGPL-2.1-or-later-SPDX"
    if "GNU Lesser General Public License" in header:
        return "LGPL-header-unspecified-version"
    if "public domain" in header.lower():
        return "public-domain-claim"
    raise ValueError(f"unclassified leading notice: {path}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream_git", type=Path)
    parser.add_argument("--generate", action="store_true", help="print the expected TSV")
    args = parser.parse_args()
    repo = args.upstream_git
    for rev, expected in (("v2.42.4", TAG), ("v2.42.4^{commit}", COMMIT),
                          ("v2.42.4^{tree}", TREE)):
        actual = git(repo, "rev-parse", rev).decode().strip()
        if actual != expected:
            raise ValueError(f"{rev}: {actual} != {expected}")

    meson = source(repo, "libuuid/meson.build").decode()
    block = meson.split("lib_uuid_sources = '''", 1)[1].split("'''", 1)[0]
    listed = set(re.findall(r"src/([A-Za-z0-9_]+)\.c", block))
    listed_headers = set(re.findall(r"src/([A-Za-z0-9_]+)\.h", block))
    if listed != set(UUID_C) - {"predefined", "unpack", "unparse"} or listed_headers != {"uuidP", "uuidd"}:
        raise ValueError("lib_uuid_sources changed")
    library = meson.split("lib_uuid = library(", 1)[1].split("uuid_dep =", 1)[0]
    for token in ("list_h", "lib_uuid_sources", "predefined_c", "unpack_c", "unparse_c",
                  "randutils_c", "md5_c", "sha1_c"):
        if not re.search(r"\b" + token + r"\b", library):
            raise ValueError(f"missing library input {token}")
    lib_meson = source(repo, "lib/meson.build").decode()
    for name in SHARED_C:
        if not re.search(rf"\b{name}_c\s*=\s*files\('{name}\.c'\)", lib_meson):
            raise ValueError(f"missing shared source {name}")
    if "list_h = files('list.h')" not in source(repo, "include/meson.build").decode():
        raise ValueError("list_h definition changed")

    paths = ([(f"libuuid/src/{name}.c", "compiled-C") for name in UUID_C]
             + [(f"lib/{name}.c", "compiled-C") for name in SHARED_C]
             + [(f"libuuid/src/{name}.h", "included-header" if name != "uuidd" else "included-and-Meson-header") for name in UUID_HEADERS]
             + [(f"include/{name}.h", "Meson-input-header" if name == "list" else "included-header") for name in INCLUDE_HEADERS])
    local_headers = {path for path, _ in paths if path.endswith(".h")}
    available_headers = {path for path in git(repo, "ls-tree", "-r", "--name-only", COMMIT,
                                              "include", "libuuid/src").decode().splitlines()
                         if path.endswith(".h")}
    discovered = set()
    pending = [path for path, role in paths if role == "compiled-C"]
    while pending:
        current = pending.pop()
        for name in re.findall(rb'^\s*#\s*include\s+"([^"]+)"', source(repo, current), re.M):
            name = name.decode()
            candidates = (str(Path(current).parent / name), f"include/{name}",
                          f"libuuid/src/{name}")
            resolved = next((candidate for candidate in candidates if candidate in available_headers), None)
            if resolved and resolved not in discovered:
                discovered.add(resolved)
                pending.append(resolved)
    if discovered != local_headers - {"include/list.h"}:
        raise ValueError(f"header closure differs: missing={local_headers - discovered - {'include/list.h'}}, extra={discovered - local_headers}")
    lines = ["path\trole\tnotice\tsha256"]
    for path, role in sorted(paths):
        data = source(repo, path)
        lines.append(f"{path}\t{role}\t{notice(path, data)}\t{hashlib.sha256(data).hexdigest()}")
    expected = "\n".join(lines) + "\n"
    if args.generate:
        sys.stdout.write(expected)
        return
    manifest = Path(__file__).resolve().parents[2] / "packaging/arch/issue21-source-notices/util-linux-libs/libuuid-source-notices.tsv"
    if manifest.read_text() != expected:
        raise ValueError(f"manifest differs: {manifest}; run --generate to inspect")
    license_path = Path(__file__).resolve().parents[2] / "packaging/arch/issue21-source-notices/util-linux-libs/COPYING.LGPL-2.1-or-later"
    if license_path.read_bytes() != source(repo, "Documentation/licenses/COPYING.LGPL-2.1-or-later"):
        raise ValueError("LGPL full text differs from signed source tree")
    print(f"VERIFIED {len(paths)} mapped inputs, tag {TAG}, commit {COMMIT}, tree {TREE}")


if __name__ == "__main__":
    main()
