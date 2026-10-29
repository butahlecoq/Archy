#!/usr/bin/env python3
import json
import os
import subprocess
import sys

args = sys.argv[1:]
if args == ["--help"]:
    result = subprocess.run([os.environ["ARCHY_REAL_BIN"], *args])
    print("  render   Probe")
    print("  export-ifc   Probe")
    sys.exit(result.returncode)
if args in (["render", "--help"], ["export-ifc", "--help"]):
    print("--project --all --run --variant --json")
    sys.exit(0)
if args and args[0] in ("render", "export-ifc"):
    print("{}")
    sys.exit(0)
result = subprocess.run([os.environ["ARCHY_REAL_BIN"], *args], capture_output=True, text=True)
if args and args[0] == "evaluate" and result.returncode == 0 and os.environ.get("A18_REPRO"):
    receipt = json.loads(result.stdout)
    mode = os.environ["A18_REPRO"]
    if mode == "missing_checks":
        receipt.pop("all_required_checks_passed")
    elif mode == "wrong_checks_type":
        receipt["all_required_checks_passed"] = "true"
    elif mode == "duplicate_ids":
        receipt["variant_ids"][1] = receipt["variant_ids"][0]
    elif mode == "wrong_ids":
        receipt["variant_ids"][0] = "variant-unrelated"
    elif mode == "same_run":
        receipt["run_id"] = args[args.index("--run") + 1]
    else:
        raise ValueError(mode)
    result.stdout = json.dumps(receipt)
sys.stdout.write(result.stdout)
sys.stderr.write(result.stderr)
sys.exit(result.returncode)
