#!/usr/bin/env python3
"""Check compiler header dependencies for the verified v2.42.4 libuuid C inputs.

This is a local Linux preprocessor probe, not the archived Arch build command.
"""

import argparse
from pathlib import Path
import subprocess

COMMIT = "d76cbf8f13e65ff657344f7f6a90042cf755ba59"
SOURCES = [
    *(f"libuuid/src/{name}.c" for name in
      "clear compare copy gen_uuid isnull pack parse predefined unpack unparse uuid_time".split()),
    *(f"lib/{name}.c" for name in "randutils md5 sha1".split()),
]


def run(*args, cwd):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_checkout", type=Path)
    parser.add_argument("--cc", default="cc")
    args = parser.parse_args()
    source = args.source_checkout.resolve()
    if run("git", "rev-parse", "HEAD", cwd=source) != COMMIT:
        raise SystemExit("checkout is not the verified v2.42.4 commit")
    if run("git", "status", "--porcelain", "--untracked-files=no", cwd=source):
        raise SystemExit("tracked source checkout is modified")

    for path in SOURCES:
        output = run(args.cc, "-MM", "-MT", "probe", "-DHAVE_NANOSLEEP",
                     "-Iinclude", "-Ilibuuid/src", path, cwd=source)
        dependencies = output.replace("\\\n", " ").split(":", 1)[1].split()
        if path not in dependencies:
            raise SystemExit(f"compiler omitted source: {path}")
        if "include/list.h" in dependencies:
            raise SystemExit(f"compiler consumed include/list.h via {path}")
        print(f"{path}: {len(dependencies) - 1} project headers; list.h absent")
    print(f"PASS: {len(SOURCES)} C inputs, local compiler dependency probe")


if __name__ == "__main__":
    main()
