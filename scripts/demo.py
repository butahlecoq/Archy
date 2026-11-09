#!/usr/bin/env python3
"""Run the supported local CLI journey on a new, retained project."""

import argparse
import hashlib
import json
import re
import shlex
import subprocess
import sys
import tempfile
import uuid
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
    if not isinstance(run_id, str) or not re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", run_id):
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: run_id")
    if (not isinstance(ids, list) or len(ids) != 3
            or any(not isinstance(item, str) or not re.fullmatch(r"variant-[0-9a-f]{64}", item) for item in ids)
            or len(set(ids)) != 3):
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: expected three unique variant_ids")
    if type(passed) is not bool:
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: all_required_checks_passed")
    if type(revision) is not int or revision < 1 or data.get("path") != f"runs/{run_id}":
        raise DemoStopped(f"archy {step} returned an invalid JSON receipt: run path or revision")
    return data


def commands(help_text):
    return set(re.findall(r"^  ([a-z][a-z-]*)\s{2,}", help_text, re.MULTILINE))


def render_receipt(output, project, evaluated):
    try:
        data = json.loads(output)
    except ValueError as error:
        raise DemoStopped(f"archy render returned invalid JSON: {error}") from error
    if not isinstance(data, dict):
        raise DemoStopped("archy render returned an invalid receipt: expected object")
    run_id = data.get("run_id")
    record = data.get("record")
    if (not isinstance(run_id, str) or not re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", run_id)
            or data.get("path") != f"runs/{run_id}"
            or type(data.get("revision")) is not int
            or data["revision"] != evaluated["revision"] + 1
            or not isinstance(record, dict)
            or record.get("schema_version") != "archy-render-v1"
            or record.get("source_run_id") != evaluated["run_id"]
            or not isinstance(data.get("manifest_sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", data["manifest_sha256"])):
        raise DemoStopped("archy render returned an invalid receipt: identity, source or revision")
    run = project / "runs" / run_id
    manifest = run / "manifest.json"
    if (run.is_symlink() or manifest.is_symlink() or not manifest.is_file()
            or hashlib.sha256(manifest.read_bytes()).hexdigest() != data["manifest_sha256"]):
        raise DemoStopped("archy render receipt manifest is missing or differs from disk")
    record_path = run / "render.json"
    if record_path.is_symlink():
        raise DemoStopped("archy render record is a symlink")
    try:
        stored = json.loads(record_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise DemoStopped(f"archy render record is missing or invalid: {error}") from error
    if stored != record:
        raise DemoStopped("archy render receipt differs from retained render.json")
    source_manifest = project / "runs" / evaluated["run_id"] / "manifest.json"
    if (source_manifest.is_symlink() or not source_manifest.is_file()
            or record.get("source_manifest_sha256")
            != hashlib.sha256(source_manifest.read_bytes()).hexdigest()):
        raise DemoStopped("archy render receipt differs from evaluated source manifest")
    variants = record.get("variants")
    if (not isinstance(variants, list) or len(variants) != 3
            or any(not isinstance(v, dict) for v in variants)
            or [v.get("variant_id") for v in variants] != evaluated["variant_ids"]):
        raise DemoStopped("archy render receipt has wrong variant selection")
    verified = []
    failed = []
    for variant in variants:
        result = variant.get("result")
        if (type(variant.get("required_checks_passed")) is not bool
                or not isinstance(result, dict)):
            raise DemoStopped("archy render receipt has malformed variant result")
        if result.get("status") == "failed":
            error = result.get("error")
            if not isinstance(error, dict) or not isinstance(error.get("code"), str):
                raise DemoStopped("archy render receipt has malformed failure")
            failed.append((variant["variant_id"], error["code"]))
            continue
        if result.get("status") != "verified":
            raise DemoStopped("archy render receipt has unknown result status")
        previews = result.get("preview_foreground_pixels")
        if (result.get("verifier_version") != "archy-rust-png-glb-v1"
                or not isinstance(previews, list) or len(previews) != 3
                or any(not isinstance(item, list) or len(item) != 2
                       or item[0] != name or type(item[1]) is not int or item[1] <= 0
                       for item, name in zip(previews,
                           ("view-top.png", "view-front.png", "view-iso.png")))):
            raise DemoStopped("archy render receipt has invalid verification metadata")
        artifacts = result.get("artifacts")
        if not isinstance(artifacts, list):
            raise DemoStopped("archy render receipt has no artifact inventory")
        prefix = f"renders/{variant['variant_id']}/"
        expected = {prefix + name for name in (
            "response.json", "scene.blend", "model.glb", "view-iso.png",
            "view-front.png", "view-top.png")}
        if (len(artifacts) != len(expected)
                or any(not isinstance(a, dict) or not isinstance(a.get("path"), str)
                       for a in artifacts)
                or {a["path"] for a in artifacts} != expected):
            raise DemoStopped("archy render receipt has incomplete artifact inventory")
        for artifact in artifacts:
            path = artifact["path"]
            digest = artifact.get("sha256")
            size = artifact.get("byte_len")
            file = run / path
            if (not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest)
                    or type(size) is not int or size <= 0
                    or any(part.is_symlink() for part in (file, file.parent, file.parent.parent))
                    or not file.is_file() or file.stat().st_size != size
                    or hashlib.sha256(file.read_bytes()).hexdigest() != digest):
                raise DemoStopped(f"archy render artifact missing or differs: {path}")
            verified.append(file)
    return run, verified, failed, all(v["required_checks_passed"] for v in variants)


def accepted_render(binary, project_manifest, rendered, run, artifacts):
    output = invoke(binary, "desktop", "status", "--project", project_manifest,
                    "--run", rendered["run_id"], "--json")
    try:
        status = json.loads(output)
    except ValueError as error:
        raise DemoStopped(f"archy desktop status returned an invalid acceptance result: {error}") from error
    selected = status.get("selected_run") if isinstance(status, dict) else None
    summary = selected.get("summary") if isinstance(selected, dict) else None
    entries = selected.get("artifacts") if isinstance(selected, dict) else None
    if (not isinstance(summary, dict) or not isinstance(entries, list)
            or any(not isinstance(item, dict) or not isinstance(item.get("id"), str)
                   for item in entries)
            or type(summary.get("all_required_checks_passed")) is not bool
            or summary.get("render_outcome") not in ("verified", "partial", "failed")):
        raise DemoStopped("archy desktop status returned an invalid acceptance result")
    inventory = {item["id"]: item for item in entries}
    if (status.get("schema_version") != "archy-desktop-status-v1"
            or summary.get("run_id") != rendered["run_id"]
            or type(summary.get("revision")) is not int
            or summary.get("revision") != rendered["revision"]
            or summary.get("operation") != "render"
            or type(summary.get("variant_count")) is not int
            or summary.get("variant_count") != 3
            or len(inventory) != len(entries)):
        raise DemoStopped("native accepted render identity or inventory differs from receipt")
    expected = [run / "render.json", *artifacts]
    for path in expected:
        relative = path.relative_to(run).as_posix()
        entry = inventory.get(relative)
        if (not isinstance(entry, dict) or entry.get("integrity") != "verified"
                or type(entry.get("byte_len")) is not int
                or entry.get("byte_len") != path.stat().st_size
                or entry.get("sha256") != hashlib.sha256(path.read_bytes()).hexdigest()):
            raise DemoStopped(f"native accepted render does not verify artifact: {relative}")
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archy", default="archy", help="built Archy CLI executable")
    parser.add_argument("--project-dir", type=Path, help="new directory; must not exist")
    parser.add_argument("--name", default="courtyard-demo", help="new project identifier")
    parser.add_argument("--export-variant", help="candidate variant ID to identify for later human review; no IFC export")
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
    for command in ("init", "validate", "generate", "evaluate", "render", "compare"):
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
    if args.export_variant and args.export_variant not in evaluated["variant_ids"]:
        raise DemoStopped("selected variant is absent from evaluated run; inspect retained runs")
    key = str(uuid.uuid4())
    print(f"Render retry key: {key}; retry only the same render request after an uncertain outcome.", flush=True)
    rendered = invoke(args.archy, "render", "--project", manifest, "--all",
                      "--run", evaluated["run_id"], "--expected-project-revision",
                      evaluated["revision"], "--idempotency-key", key, "--json")
    run, artifacts, failures, render_checks_passed = render_receipt(rendered, project, evaluated)
    for suffix, extra in (("json", ("--json",)), ("md", ())):
        output = invoke(args.archy, "compare", "--project", manifest,
                        "--run", generated["run_id"], *extra)
        with (project / f"comparison.{suffix}").open("x", encoding="utf-8") as file:
            file.write(output)
    native = accepted_render(args.archy, manifest, json.loads(rendered), run, artifacts)

    if not evaluated["all_required_checks_passed"]:
        print("NOTE: one or more required checks failed; inspect retained evaluations.",
              file=sys.stderr)
    print(f"Render receipt: {run / 'render.json'}", flush=True)
    print(f"Comparison: {project / 'comparison.json'} and {project / 'comparison.md'}", flush=True)
    if failures:
        detail = ", ".join(f"{variant}: {code}" for variant, code in failures)
        raise DemoStopped(f"render incomplete ({detail}); retained receipt: {run / 'render.json'}")
    if (not evaluated["all_required_checks_passed"] or not render_checks_passed
            or not native["all_required_checks_passed"] or native.get("render_outcome") != "verified"):
        raise DemoStopped("required design checks failed; inspect retained evaluations and render receipt")
    for path in artifacts:
        print(f"Verified artifact: {path}", flush=True)
    if args.export_variant:
        print(f"Candidate for human review: {args.export_variant}; no IFC was exported.", flush=True)
    print("REVIEW READY: three rendered variants and comparisons are retained. "
          "Human selection and approval are required; no IFC export or approval was made.", flush=True)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (DemoStopped, OSError) as error:
        print(f"DEMO STOPPED: {error}", file=sys.stderr)
        sys.exit(2)
