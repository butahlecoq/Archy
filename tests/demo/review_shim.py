#!/usr/bin/env python3
"""Injected demo protocol fixture; it does not exercise Blender or native verification."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

args = sys.argv[1:]
mode = os.environ.get("A18_REPRO", "success")
if args[:2] == ["desktop", "status"] and mode in (
        "simulated_ready", "status_null_summary", "status_bad_hash"):
    project = Path(args[args.index("--project") + 1]).parent
    run_id = args[args.index("--run") + 1]
    run = project / "runs" / run_id
    evaluated = json.loads(Path(os.environ["A18_EVAL_RECEIPT"]).read_text())
    record = json.loads((run / "render.json").read_text())
    paths = ["render.json"] + [artifact["path"] for variant in record["variants"]
                               for artifact in variant["result"]["artifacts"]]
    inventory = []
    for path in paths:
        content = (run / path).read_bytes()
        kind = "preview" if path.endswith(".png") else "blender" if path.endswith(".blend") else "run_data"
        inventory.append({"id": path, "kind": kind, "byte_len": len(content),
                          "sha256": hashlib.sha256(content).hexdigest(),
                          "integrity": "verified"})
    if mode == "status_bad_hash":
        inventory[-1]["sha256"] = "0" * 64
    summary = {"run_id": run_id, "revision": evaluated["revision"] + 1,
               "operation": "render", "variant_count": 3,
               "all_required_checks_passed": True, "render_outcome": "verified",
               "artifact_count": len(inventory)}
    runs = [summary]
    if mode == "status_null_summary":
        summary = None
    print(json.dumps({"schema_version": "archy-desktop-status-v1",
                      "project": {"id": "courtyard-demo", "revision": evaluated["revision"] + 1,
                                  "source_sha256": hashlib.sha256((project / "project.yaml").read_bytes()).hexdigest()},
                      "runs": runs, "next_cursor": None,
                      "backend_jobs": {"status": "unknown", "reason": "simulated fixture"},
                      "selected_run": {"summary": summary, "artifacts": inventory}}))
    sys.exit(0)
if args and args[0] == "render":
    if mode == "error_receipt":
        print(json.dumps({"code": "REVISION_CONFLICT", "message": "fixture rejection",
                          "field": "revision", "retriable": True}))
        sys.exit(1)
    project = Path(args[args.index("--project") + 1]).parent
    evaluated = json.loads(Path(os.environ["A18_EVAL_RECEIPT"]).read_text())
    run_id = str(uuid.uuid4())
    run = project / "runs" / run_id
    run.mkdir()
    variants = []
    names = ("response.json", "scene.blend", "model.glb", "view-iso.png",
             "view-front.png", "view-top.png")
    for variant_id in evaluated["variant_ids"]:
        artifacts = []
        for name in names:
            relative = f"renders/{variant_id}/{name}"
            file = run / relative
            file.parent.mkdir(parents=True, exist_ok=True)
            content = f"INJECTED PROTOCOL FIXTURE {variant_id} {name}".encode()
            file.write_bytes(content)
            artifacts.append({"path": relative, "sha256": hashlib.sha256(content).hexdigest(),
                              "byte_len": len(content), "media_type": "fixture/only",
                              "producer": "injected-demo-fixture"})
        variants.append({"variant_id": variant_id, "required_checks_passed": True,
                         "result": {"status": "verified", "verifier_version": "archy-rust-png-glb-v1",
                                    "artifacts": artifacts,
                                    "preview_foreground_pixels": [[name, 10000] for name in
                                        ("view-top.png", "view-front.png", "view-iso.png")]}})
    source_manifest = project / "runs" / evaluated["run_id"] / "manifest.json"
    record = {"schema_version": "archy-render-v1", "source_run_id": evaluated["run_id"],
              "source_manifest_sha256": hashlib.sha256(source_manifest.read_bytes()).hexdigest(),
              "variants": variants}
    if mode == "failed":
        variants[0]["result"] = {"status": "failed", "error": {"code": "BACKEND_UNAVAILABLE"},
                                  "worker_response_sha256": None}
    if mode == "wrong_selector":
        variants[0]["variant_id"] = "variant-other"
    (run / "render.json").write_text(json.dumps(record))
    manifest = b'{"fixture":"not a native accepted run"}'
    (run / "manifest.json").write_bytes(manifest)
    receipt = {"run_id": run_id, "revision": evaluated["revision"] + 1,
               "path": f"runs/{run_id}", "manifest_sha256": hashlib.sha256(manifest).hexdigest(),
               "record": record}
    if mode == "wrong_source":
        receipt["record"]["source_run_id"] = "other"
    if mode == "wrong_revision":
        receipt["revision"] += 1
    if mode == "missing_artifact":
        variants[0]["result"]["artifacts"].pop()
        (run / "render.json").write_text(json.dumps(record))
    if mode == "wrong_verifier":
        variants[0]["result"]["verifier_version"] = "fixture-only"
        (run / "render.json").write_text(json.dumps(record))
    if mode == "bad_digest":
        variants[0]["result"]["artifacts"][0]["sha256"] = "0" * 64
        (run / "render.json").write_text(json.dumps(record))
    if mode == "malformed":
        print("not JSON")
    else:
        print(json.dumps(receipt))
    sys.exit(0)
result = subprocess.run([os.environ["ARCHY_REAL_BIN"], *args], capture_output=True, text=True)
if args and args[0] == "evaluate" and result.returncode == 0:
    Path(os.environ["A18_EVAL_RECEIPT"]).write_text(result.stdout)
    if mode in ("missing_checks", "wrong_checks_type", "duplicate_ids", "wrong_ids", "same_run"):
        receipt = json.loads(result.stdout)
        if mode == "missing_checks":
            receipt.pop("all_required_checks_passed")
        elif mode == "wrong_checks_type":
            receipt["all_required_checks_passed"] = "true"
        elif mode == "duplicate_ids":
            receipt["variant_ids"][1] = receipt["variant_ids"][0]
        elif mode == "wrong_ids":
            receipt["variant_ids"][0] = "variant-unrelated"
        else:
            receipt["run_id"] = args[args.index("--run") + 1]
        result.stdout = json.dumps(receipt)
sys.stdout.write(result.stdout)
sys.stderr.write(result.stderr)
sys.exit(result.returncode)
