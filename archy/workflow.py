"""Run the complete local massing workflow from a brief."""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

from .massing import evaluate_brief
from .comparison import build_comparison, render_html
from .ifc import export_evaluation, require_ifc_support, validate_evaluation
from .ifc_verify import verify as verify_ifc


def _remove_path(path: Path) -> None:
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path, ignore_errors=True)
    else:
        try:
            path.unlink()
        except OSError:
            pass


def _is_archy_output(path: Path) -> bool:
    if not path.is_dir() or path.is_symlink():
        return False
    try:
        manifest_path = path / "manifest.json"
        if manifest_path.is_symlink() or not manifest_path.is_file():
            return False
        with manifest_path.open("rb") as manifest_file:
            manifest_bytes = manifest_file.read(16 * 1024 * 1024 + 1)
        if len(manifest_bytes) > 16 * 1024 * 1024:
            return False
        manifest = json.loads(manifest_bytes)
    except (OSError, json.JSONDecodeError, UnicodeDecodeError):
        return False
    required = {"evaluation.json", "inspection.json", "massing.blend", "massing.ifc", "comparison.json", "comparison.html"}
    artifacts = manifest.get("artifacts") if isinstance(manifest, dict) else None
    return (
        isinstance(manifest, dict)
        and manifest.get("contract_version") == 2
        and isinstance(artifacts, dict)
        and required.issubset(artifacts)
    )


def _matching_siblings(parent: Path, prefix: str) -> list[Path]:
    return [path for path in parent.iterdir() if path.name.startswith(prefix)]


def _recover_output(output_dir: Path) -> None:
    parent = output_dir.parent
    backup_prefix = f".{output_dir.name}.previous-"
    backups = sorted(
        (path for path in _matching_siblings(parent, backup_prefix) if _is_archy_output(path)),
        key=lambda path: (path.lstat().st_mtime_ns, path.name),
    )
    if not os.path.lexists(output_dir) and backups:
        os.replace(backups.pop(), output_dir)
    for stale in backups:
        _remove_path(stale)
    staging_prefix = f".{output_dir.name}.staging-"
    for stale_stage in _matching_siblings(parent, staging_prefix):
        marker = stale_stage / ".archy-staging"
        if marker.is_file():
            _remove_path(stale_stage)


def run_workflow(
    brief: Path,
    output_dir: Path,
    blender: str = "blender",
    *,
    quiet: bool = False,
) -> Path:
    brief_argument = brief.absolute()
    brief = brief_argument.resolve()
    output_dir = output_dir.resolve()
    if output_dir == output_dir.parent:
        raise ValueError("output directory cannot be a filesystem root")
    if brief_argument.is_relative_to(output_dir) or brief.is_relative_to(output_dir):
        raise ValueError("output directory cannot contain the source brief")
    if os.path.lexists(output_dir) and not _is_archy_output(output_dir):
        raise ValueError("existing output directory is not a recognized Archy workflow bundle")
    output_dir.parent.mkdir(parents=True, exist_ok=True)
    lock_path = output_dir.with_name(f".{output_dir.name}.lock")
    with lock_path.open("a+b") as lock_file:
        fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
        _recover_output(output_dir)
        staging = Path(tempfile.mkdtemp(prefix=f".{output_dir.name}.staging-", dir=output_dir.parent))
        (staging / ".archy-staging").write_text("Archy workflow staging directory\n", encoding="utf-8")
        backup = output_dir.with_name(f".{output_dir.name}.previous-{uuid.uuid4().hex}")
        try:
            comparison_path = _build_workflow(brief, staging, blender, quiet=quiet)
            (staging / ".archy-staging").unlink()
            if os.path.lexists(output_dir):
                os.replace(output_dir, backup)
            os.replace(staging, output_dir)
            if os.path.lexists(backup):
                _remove_path(backup)
            return output_dir / comparison_path.name
        except BaseException:
            _remove_path(staging)
            if os.path.lexists(backup) and not os.path.lexists(output_dir):
                os.replace(backup, output_dir)
            raise


def _build_workflow(brief: Path, output_dir: Path, blender: str, *, quiet: bool = False) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    report = evaluate_brief(brief)
    validate_evaluation(report)
    require_ifc_support()
    report_path = output_dir / "evaluation.json"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    scene = output_dir / "massing.blend"
    script = Path(__file__).with_name("blender_scene.py")
    blender_stdout = subprocess.DEVNULL if quiet else None
    blender = str(Path(shutil.which(blender) or blender).absolute())
    blender_env = os.environ.copy()
    blender_env["PATH"] = os.defpath
    for variable in tuple(blender_env):
        if variable.startswith("PYTHON"):
            blender_env.pop(variable)
    subprocess.run(
        [blender, "--background", "--factory-startup", "--python-exit-code", "1", "--python", str(script), "--", str(report_path), str(scene)],
        check=True,
        stdout=blender_stdout,
        env=blender_env,
    )
    renders = output_dir / "renders"
    subprocess.run(
        [blender, "--background", "--factory-startup", "--python-exit-code", "1", "--python", str(script), "--", str(report_path), str(scene), "--render-dir", str(renders)],
        check=True,
        stdout=blender_stdout,
        env=blender_env,
    )
    inspection_path = output_dir / "inspection.json"
    subprocess.run(
        [blender, "--background", "--factory-startup", "--python-exit-code", "1", "--python", str(script), "--", str(report_path), str(scene), "--inspect", "--inspection-json", str(inspection_path)],
        check=True,
        stdout=blender_stdout,
        env=blender_env,
    )
    inspection = json.loads(inspection_path.read_text(encoding="utf-8"))
    ifc_path = export_evaluation(report, output_dir / "massing.ifc")
    ifc_failures = verify_ifc(report_path, ifc_path)
    if ifc_failures:
        raise ValueError("IFC verification failed: " + "; ".join(ifc_failures))
    comparison = build_comparison(report, inspection, renders, ifc_model="massing.ifc")
    comparison_path = output_dir / "comparison.json"
    comparison_path.write_text(json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (output_dir / "comparison.html").write_text(render_html(comparison), encoding="utf-8")
    artifact_paths = [report_path, scene, inspection_path, ifc_path, comparison_path, output_dir / "comparison.html", *sorted(renders.glob("*.png"))]
    manifest = {
        "contract_version": 2,
        "artifacts": {
            path.relative_to(output_dir).as_posix(): {
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "size_bytes": path.stat().st_size,
            }
            for path in artifact_paths
        },
        "blender": inspection["runtime"],
    }
    (output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return comparison_path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("brief", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("--blender", default=shutil.which("blender") or "blender")
    args = parser.parse_args(argv)
    try:
        comparison = run_workflow(args.brief, args.output_dir, args.blender)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(2, f"archy-workflow: error: {error}\n")
    print(comparison)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
