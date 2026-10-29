#!/usr/bin/env python3
"""Run the supported local CLI journey on a new, retained project."""

import argparse
import json
import re
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path


class DemoStopped(Exception):
    pass


def invoke(binary, *args):
    command = [binary, *map(str, args)]
    print("$ " + shlex.join(["archy", *map(str, args)]), flush=True)
    result = subprocess.run(command, text=True, capture_output=True, check=False)
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n", flush=True)
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n",
              file=sys.stderr, flush=True)
    print(f"[exit {result.returncode}]", flush=True)
    if result.returncode:
        raise DemoStopped(f"archy {args[0]} failed with exit {result.returncode}")
    return result.stdout


def receipt(output, step):
    try:
        data = json.loads(output)
    except ValueError as error:
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: {error}") from error
    if not isinstance(data, dict):
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: expected object")
    run_id = data.get("run_id")
    ids = data.get("variant_ids")
    passed = data.get("all_required_checks_passed")
    revision = data.get("revision")
    if not isinstance(run_id, str) or not run_id:
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: run_id")
    if (not isinstance(ids, list) or len(ids) != 3
            or any(not isinstance(item, str) or not item for item in ids)
            or len(set(ids)) != 3):
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: expected three unique variant_ids")
    if type(passed) is not bool:
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: all_required_checks_passed")
    if type(revision) is not int or revision < 1 or data.get("path") != f"runs/{run_id}":
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: run path or revision")
    return data


def commands(help_text):
    return set(re.findall(r"^  ([a-z][a-z-]*)\s{2,}", help_text, re.MULTILINE))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archy", default="archy", help="built Archy CLI executable")
    parser.add_argument("--project-dir", type=Path, help="new directory; must not exist")
    parser.add_argument("--name", default="courtyard-demo", help="new project identifier")
    parser.add_argument("--export-variant", help="explicit variant ID for draft IFC export")
    args = parser.parse_args()

    if args.project_dir is None:
        parent = Path(tempfile.mkdtemp(prefix="archy-demo-"))
        project = parent / "courtyard-demo"
    else:
        project = args.project_dir.absolute()
    if project.exists() or project.is_symlink():
        raise DemoStopped(f"project path already exists: {project}; choose a new directory")
    if not project.parent.is_dir():
        raise DemoStopped(f"project parent directory does not exist: {project.parent}")

    print(f"Project: {project}", flush=True)
    available = commands(invoke(args.archy, "--help"))
    for command in ("init", "validate", "generate", "evaluate", "compare"):
        if command not in available:
            raise DemoStopped(f"UNAVAILABLE: archy {command} is absent from this CLI")

    invoke(args.archy, "init", "--name", args.name, "--dir", project)
    manifest = project / "project.yaml"
    invoke(args.archy, "validate", "--project", manifest, "--json")
    generated = receipt(invoke(args.archy, "generate", "--project", manifest,
                               "--count", 3, "--seed", 42, "--json"), "generate")
    evaluated = receipt(invoke(args.archy, "evaluate", "--project", manifest,
                               "--all", "--run", generated["run_id"], "--json"), "evaluate")
    if (evaluated["variant_ids"] != generated["variant_ids"]
            or evaluated["run_id"] == generated["run_id"]
            or evaluated["revision"] <= generated["revision"]):
        raise DemoStopped("evaluation run identity, revision or variant IDs differ from generation; "
                          "inspect retained runs")
    for suffix, extra in (("json", ("--json",)), ("md", ())):
        output = invoke(args.archy, "compare", "--project", manifest,
                        "--run", generated["run_id"], *extra)
        with (project / f"comparison.{suffix}").open("x", encoding="utf-8") as file:
            file.write(output)

    if not evaluated["all_required_checks_passed"]:
        print("NOTE: one or more required checks failed; inspect retained evaluations.",
              file=sys.stderr)
    if "render" not in available:
        raise DemoStopped("UNAVAILABLE: archy render is absent from this CLI. "
                          "Existing runs and comparison remain in the project.")
    raise DemoStopped("UNAVAILABLE: archy render/export-ifc have no reviewed CLI receipt "
                      "and artifact contract. A18 remains open; inspect retained runs "
                      "and comparison, then integrate and verify real backend files.")


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (DemoStopped, OSError) as error:
        print(f"DEMO STOPPED: {error}", file=sys.stderr)
        sys.exit(2)
