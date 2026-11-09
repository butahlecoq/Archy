"""Disposable A10 WSL proof. Run with the pinned independent reader Python."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
from typing import NamedTuple

ROOT = Path(__file__).resolve().parents[3]
NAMES = {"scene.blend", "model.glb", "view-top.png", "view-front.png", "view-iso.png"}


class FixturePaths(NamedTuple):
    blender: Path
    evidence: Path
    cli: Path


def resolve_fixture_paths(root, common_git_dir, environ, *, is_dir=Path.is_dir):
    """Resolve test paths without creating or modifying any files."""
    common_checkout = common_git_dir.parent if common_git_dir.name == ".git" else root
    local_tools = common_checkout / ".tools"
    sibling_tools = common_checkout.parent / ".tools"
    default_tools = local_tools if is_dir(local_tools) or not is_dir(sibling_tools) else sibling_tools

    def override(name, default):
        if name not in environ:
            return default
        value = environ[name]
        if not value or not Path(value).is_absolute():
            raise ValueError(f"{name} must be an absolute nonempty path")
        return Path(value)

    tools_dir = override("ARCHY_A10_TOOLS_DIR", default_tools)
    return FixturePaths(
        blender=tools_dir / "blender-4.5.15-linux-x64/blender",
        evidence=override("ARCHY_A10_EVIDENCE_DIR", tools_dir / "targets/a10-three-variant-proof"),
        cli=override("ARCHY_A10_CLI", root / "target/debug/archy"),
    )


def discover_fixture_paths(root):
    common = subprocess.check_output(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd=root, text=True, timeout=10).strip()
    return resolve_fixture_paths(root, Path(common), os.environ)


def create_evidence_slot(evidence, slot):
    if evidence.is_symlink():
        raise ValueError(f"evidence root must not be a symlink: {evidence}")
    evidence.mkdir(parents=True, exist_ok=True)
    if evidence.is_symlink():
        raise ValueError(f"evidence root must not be a symlink: {evidence}")
    output = evidence / f"slot-{slot}"
    output.mkdir(mode=0o700)
    return output


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record_command(args, cwd, prefix, timeout):
    result = subprocess.run(args, cwd=cwd, capture_output=True, timeout=timeout)
    (prefix.parent / (prefix.name + ".stdout")).write_bytes(result.stdout[:131072])
    (prefix.parent / (prefix.name + ".stderr")).write_bytes(result.stderr[:131072])
    (prefix.parent / (prefix.name + ".exit")).write_text(str(result.returncode) + "\n")
    return result


def check(condition, message):
    if not condition:
        raise AssertionError(message)


def near(actual, expected, tol, label):
    delta = abs(float(actual) - float(expected))
    check(math.isfinite(delta) and delta <= tol, f"{label}: {actual} versus {expected}; error {delta} > {tol}")
    return delta


def polygon_contains(point, polygon, tolerance):
    x, y = point
    inside = False
    for a, b in zip(polygon, polygon[1:] + polygon[:1]):
        ax, ay = a; bx, by = b
        cross = abs((x - ax) * (by - ay) - (y - ay) * (bx - ax))
        if cross <= tolerance * math.hypot(bx - ax, by - ay) and min(ax, bx) - tolerance <= x <= max(ax, bx) + tolerance and min(ay, by) - tolerance <= y <= max(ay, by) + tolerance:
            return True
        if (ay > y) != (by > y) and x < ax + (y - ay) * (bx - ax) / (by - ay):
            inside = not inside
    return inside


def glb_geometry(path, variant, evaluation):
    import numpy as np
    import trimesh

    raw = path.read_bytes()
    check(len(raw) <= 64 * 1024 * 1024 and len(raw) >= 20, "GLB size")
    check(raw[:4] == b"glTF" and struct.unpack_from("<I", raw, 8)[0] == len(raw), "GLB header")
    size = struct.unpack_from("<I", raw, 12)[0]
    check(raw[16:20] == b"JSON" and 20 + size <= len(raw), "GLB JSON chunk")
    document = json.loads(raw[20:20 + size])
    check(document.get("extensionsRequired", []) == [], "unsupported required extension")
    check(all("uri" not in b for b in document["buffers"]), "external buffer")
    check(all("uri" not in i for i in document.get("images", [])), "external image")
    check(all(p.get("mode", 4) == 4 for m in document["meshes"] for p in m["primitives"]), "nontriangle primitive")
    check("skins" not in document and "animations" not in document, "unsupported deformation")
    scene = trimesh.load(path, force="scene", process=False, skip_materials=True,
                         merge_primitives=False, ignore_broken=False)
    nodes = sorted(scene.graph.nodes_geometry)
    check(len(nodes) == variant["storeys"], f"expected {variant['storeys']} mesh instances, got {len(nodes)}")
    polygon = [list(p) for p in variant["footprint_xy_m"][:-1]]
    budget = evaluation["geometry_parity_budget"]
    tol = budget["coordinate_length_m"]
    check(tol == 1e-6, "unexpected coordinate policy")
    metrics = {m["name"]: m["value"] for m in evaluation["metrics"]}
    area = metrics["footprint_area"]
    floor = variant["floor_to_floor_m"]
    bounds = []
    per_storey = []
    errors = []
    for node in nodes:
        transform, name = scene.graph[node]
        mesh = scene.geometry[name].copy()
        check(isinstance(mesh, trimesh.Trimesh) and len(mesh.faces) > 0, "nonmesh or empty geometry")
        mesh.apply_transform(transform)
        # glTF material/normal seams duplicate exact positions; weld only a topology copy.
        unique, inverse = np.unique(np.asarray(mesh.vertices), axis=0, return_inverse=True)
        solid = trimesh.Trimesh(vertices=unique, faces=inverse[mesh.faces], process=False)
        check(solid.is_watertight and solid.is_winding_consistent and solid.volume > 0, "invalid closed outward solid")
        world = np.column_stack((unique[:, 0], -unique[:, 2], unique[:, 1]))
        check(bool(np.isfinite(world).all()), "nonfinite world vertex")
        low, high = world.min(axis=0), world.max(axis=0)
        level = round(float(low[2] / floor))
        check(0 <= level < variant["storeys"], "unexpected elevation")
        for xyz in world:
            errors.append(min(abs(xyz[2] - level * floor), abs(xyz[2] - (level + 1) * floor)))
            check(polygon_contains(xyz[:2], polygon, tol), "world vertex outside core footprint")
        for corner in polygon:
            for z in (level * floor, (level + 1) * floor):
                distance = np.min(np.max(np.abs(world - [corner[0], corner[1], z]), axis=1))
                errors.append(float(distance))
                check(distance <= tol, f"missing transformed corner {corner} z={z}: {distance}")
        for axis, key in enumerate(("xmin", "ymin")):
            errors.append(near(low[axis], metrics[key], tol, key))
        for axis, key in enumerate(("xmax", "ymax")):
            errors.append(near(high[axis], metrics[key], tol, key))
        errors.append(near(low[2], level * floor, tol, "storey bottom"))
        errors.append(near(high[2], (level + 1) * floor, tol, "storey top"))
        # Cap triangles detect a filled L notch that bounds alone cannot see.
        triangle_world = world[solid.faces]
        roofs = triangle_world[np.all(np.abs(triangle_world[:, :, 2] - high[2]) <= tol, axis=1)]
        check(len(roofs) > 0, "missing roof")
        roof_area = sum(abs((t[1, 0] - t[0, 0]) * (t[2, 1] - t[0, 1])
                            - (t[1, 1] - t[0, 1]) * (t[2, 0] - t[0, 0])) / 2 for t in roofs)
        for triangle in roofs:
            check(polygon_contains(triangle[:, :2].mean(axis=0), polygon, tol), "roof fills concave notch")
        errors.append(near(roof_area, area, budget["footprint_area_m2"], "roof area"))
        volume_tol = budget["footprint_area_m2"] * floor + 2 * area * tol
        errors.append(near(solid.volume, area * floor, volume_tol, "storey volume"))
        bounds.append((low.tolist(), high.tolist()))
        per_storey.append({"level": level, "volume_m3": solid.volume, "roof_area_m2": float(roof_area),
                           "vertices": len(unique), "triangles": len(solid.faces), "bounds_m": [*low, *high]})
    check(sorted(x["level"] for x in per_storey) == list(range(variant["storeys"])), "missing/duplicate storey")
    low = np.min([b[0] for b in bounds], axis=0); high = np.max([b[1] for b in bounds], axis=0)
    errors.append(near(high[2] - low[2], metrics["height"], tol, "building height"))
    errors.append(near(sum(x["roof_area_m2"] for x in per_storey), metrics["gross_floor_area"], budget["gross_floor_area_m2"], "GFA"))
    return {"bounds_m": [*map(float, low), *map(float, high)], "storeys": per_storey,
            "coordinate_tolerance_m": tol,
            "area_tolerance_m2": budget["footprint_area_m2"], "gfa_tolerance_m2": budget["gross_floor_area_m2"]}


def inventory(root, destination):
    lines = ["path\tbytes\tsha256"]
    for path in sorted(root.rglob("*")):
        if path.is_file() and path != destination:
            lines.append(f"{path.relative_to(root)}\t{path.stat().st_size}\t{digest(path)}")
    destination.write_text("\n".join(lines) + "\n")


def run(slot, failure_index):
    import numpy as np
    import trimesh
    from PIL import Image

    paths = discover_fixture_paths(ROOT)
    blender, evidence, cli = paths
    check(sys.version_info[:2] == (3, 12), "reader requires pinned CPython 3.12")
    check((trimesh.__version__, Image.__version__, np.__version__) == ("5.2.0", "13.0.0", "2.5.4"), "reader pins changed")
    check(blender.is_file() and cli.is_file(), f"build CLI and provide pinned Blender first: {cli}, {blender}")
    output = create_evidence_slot(evidence, slot)
    source = ROOT / "examples/courtyard-housing/project.yaml"
    project = output / "project"
    project.mkdir(mode=0o700)
    shutil.copyfile(source, project / "project.yaml")
    result = {"source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "reader_versions": {"python": sys.version, "trimesh": trimesh.__version__, "pillow": Image.__version__, "numpy": np.__version__},
              "fixture_paths": {name: str(value) for name, value in paths._asdict().items()},
              "failure_index": failure_index, "variants": []}
    try:
        generation = record_command([str(cli), "generate", "--project", str(project / "project.yaml"), "--count", "3", "--seed", "42", "--json"], project, output / "generate", 90)
        check(generation.returncode == 0, "CLI generation failed")
        generate_receipt = json.loads(generation.stdout)
        generation_run = generate_receipt["run_id"]
        check((project / "runs" / generation_run / "generation.json").is_file(), "missing generation snapshot")
        evaluation = record_command([str(cli), "evaluate", "--project", str(project / "project.yaml"), "--all", "--run", generation_run, "--json"], project, output / "evaluate", 90)
        check(evaluation.returncode == 0, "CLI evaluate-all failed")
        evaluation_run = json.loads(evaluation.stdout)["run_id"]
        generation_dir = project / "runs" / generation_run
        evaluation_dir = project / "runs" / evaluation_run
        variants = json.loads((generation_dir / "generation.json").read_text())["variants"]
        check(len(variants) == 3 and len({v["id"] for v in variants}) == 3, "not three distinct variants")
        result.update({"generation_run": generation_run, "evaluation_run": evaluation_run,
                       "blender_version": record_command([str(blender), "--version"], output, output / "blender-version", 30).stdout.decode(errors="replace").splitlines()[0]})
        for index, variant in enumerate(variants):
            item = {"variant_id": variant["id"], "shape": variant["shape"], "design_checks": [], "render_status": "not_started"}
            result["variants"].append(item)
            try:
                saved_variant = json.loads((generation_dir / "variants" / (variant["id"] + ".json")).read_text())
                check(saved_variant == variant, "generation snapshot mismatch")
                report = json.loads((evaluation_dir / "evaluations" / (variant["id"] + ".json")).read_text())
                item["design_checks"] = report["checks"]
                item["required_checks_passed"] = report["required_checks_passed"]
                check(report["variant_id"] == variant["id"], "evaluation association mismatch")
                job = output / f"variant-{index}"
                job.mkdir(mode=0o700)
                request = {"schema_version": "blender-worker-v1", "operation": "build_massing",
                           "request_id": f"a10-{index}", "project_id": variant["project_id"],
                           "variant_id": variant["id"], "run_id": generation_run, "params": variant,
                           "site": {"polygon_xy_m": [[0, 0], [20, 0], [20, 35], [0, 35], [0, 0]], "north_angle_deg": 0.0},
                           "input_digest": digest(generation_dir / "variants" / (variant["id"] + ".json")),
                           "preset_id": "smoke-cpu-v1", "output_dir": f"runs/{generation_run}/blender"}
                (job / "request.json").write_text(json.dumps(request, indent=2) + "\n")
                if index == failure_index:
                    sentinel = b"a10-existing-output-sentinel\n"
                    (job / "model.glb").write_bytes(sentinel)
                command = [str(blender), "--background", "--factory-startup", "--disable-autoexec", "--offline-mode",
                           "--python-exit-code", "20", "--threads", "1", "--python", str(ROOT / "workers/blender/worker.py"),
                           "--", "--request", str(job / "request.json"), "--response", str(job / "response.json")]
                worker = record_command(command, job, job / "worker", 180)
                item["worker_exit"] = worker.returncode
                if index == failure_index:
                    check(worker.returncode != 0 and b"worker output collision" in worker.stderr,
                          "injected collision did not fail at worker boundary")
                    check((job / "model.glb").read_bytes() == sentinel and not (job / "response.json").exists(),
                          "failed worker damaged prior output or created response")
                    item["render_status"] = "failed_collision_preserved"
                    item["render_error"] = "worker output collision; existing file unchanged"
                    continue
                check(worker.returncode == 0, "Blender worker exit")
                response = json.loads((job / "response.json").read_text())
                check(response["status"] == "ok" and response["variant_id"] == variant["id"], "worker response")
                check({a["path"] for a in response["artifacts"]} == NAMES, "incomplete artifact inventory")
                for artifact in response["artifacts"]:
                    file = job / artifact["path"]
                    check(file.is_file() and file.stat().st_size == artifact["size_bytes"] and digest(file) == artifact["sha256"], "artifact digest/size")
                for view in ("top", "front", "iso"):
                    file = job / f"view-{view}.png"
                    with Image.open(file) as image:
                        check(image.format == "PNG" and image.size == (512, 512) and image.n_frames == 1, "PNG metadata")
                        image.verify()
                    with Image.open(file) as image:
                        image.load()
                        pixels = np.asarray(image.convert("RGB"), dtype=np.int16)
                        foreground = int(np.count_nonzero(np.max(np.abs(pixels - pixels[0, 0]), axis=2) > 8))
                        check(foreground > 512 * 512 // 100, f"empty {view} preview")
                        item.setdefault("png_foreground_pixels", {})[view] = foreground
                item["geometry"] = glb_geometry(job / "model.glb", variant, report)
                item["render_status"] = "fixture_verified"
            except Exception as exc:
                item["render_status"] = "failed"
                item["render_error"] = f"{type(exc).__name__}: {exc}"
        result["all_fixture_checks_passed"] = all(x["render_status"] == ("failed_collision_preserved" if i == failure_index else "fixture_verified")
                                                  and len(x["design_checks"]) > 0
                                                  for i, x in enumerate(result["variants"]))
    except Exception as exc:
        result["workflow_error"] = f"{type(exc).__name__}: {exc}"
        result["all_fixture_checks_passed"] = False
    finally:
        (output / "proof.json").write_text(json.dumps(result, indent=2, default=lambda value: value.tolist() if hasattr(value, "tolist") else float(value)) + "\n")
        inventory(output, output / "artifact-manifest.tsv")
        sources = [ROOT / x for x in ("workers/blender/worker.py", "workers/blender/scene.py", "workers/blender/protocol.py",
                  "examples/courtyard-housing/project.yaml", "tests/backend/blender/three_variant_proof.py",
                  "tests/backend/blender/test_a10_paths.py", "Cargo.lock")]
        snapshots = output / "source-snapshot"
        snapshots.mkdir()
        for source in sources:
            target = snapshots / source.relative_to(ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        (output / "source-manifest.tsv").write_text("path\tbytes\tsha256\n" + "".join(f"{x.relative_to(ROOT)}\t{x.stat().st_size}\t{digest(x)}\n" for x in sources))
        print(output)
        print(json.dumps({"all_fixture_checks_passed": result["all_fixture_checks_passed"],
                          "variants": [{"id": x["variant_id"], "design_passed": x.get("required_checks_passed"), "render": x["render_status"], "error": x.get("render_error")} for x in result["variants"]]}, indent=2))
    return 0 if result["all_fixture_checks_passed"] else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--slot", type=int, choices=range(10), required=True)
    parser.add_argument("--failure-index", type=int, choices=range(3))
    args = parser.parse_args()
    sys.exit(run(args.slot, args.failure_index))
