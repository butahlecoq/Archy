from __future__ import annotations

from dataclasses import dataclass, replace
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import tempfile
import uuid

from .massing import evaluate_brief


@dataclass(frozen=True)
class Project:
    path: Path
    project_id: str
    brief_project_id: str
    created_at: str
    current_revision_id: str
    current_generation: int


@dataclass(frozen=True)
class Revision:
    revision_id: str
    parent_revision_id: str | None
    created_at: str
    message: str
    brief_sha256: str
    path: Path
    is_current: bool
    status: str
    error: str | None


@dataclass(frozen=True)
class StudyRun:
    run_id: str
    revision_id: str
    created_at: str
    status: str
    brief_sha256: str
    path: Path
    comparison_html: Path | None
    valid_alternatives: int
    invalid_alternatives: int
    error: str | None


@dataclass(frozen=True)
class ProjectHistory:
    project: Project
    revisions: tuple[Revision, ...]
    runs: tuple[StudyRun, ...]


_UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\Z")
def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="microseconds").replace("+00:00", "Z")


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _json_bytes(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def _write_atomic(path: Path, data: bytes) -> None:
    fd, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.staging-", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        _fsync_directory(path.parent)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _read_json(path: Path) -> dict:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected a regular Archy metadata file: {path}")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read Archy metadata {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"Archy metadata must be an object: {path}")
    return value


def _safe_root(path: str | Path, *, must_exist: bool = True) -> Path:
    candidate = Path(path).expanduser().absolute()
    if candidate.is_symlink():
        raise ValueError("project root cannot be a symlink")
    root = candidate.resolve(strict=must_exist)
    if root == root.parent:
        raise ValueError("project root cannot be a filesystem root")
    if must_exist and not root.is_dir():
        raise ValueError("project root must be a directory")
    return root


def _managed_path(root: Path, relative: str) -> Path:
    path = root / relative
    try:
        path.resolve(strict=False).relative_to(root)
    except ValueError as error:
        raise ValueError(f"project path escapes its root: {relative}") from error
    current = root
    for part in Path(relative).parts:
        current = current / part
        if current.is_symlink():
            raise ValueError(f"project path cannot contain symlinks: {relative}")
    return path


def _valid_uuid(value: object, field: str) -> str:
    if not isinstance(value, str) or _UUID.fullmatch(value) is None:
        raise ValueError(f"invalid {field}")
    return value


def _load_project(path: str | Path) -> tuple[Path, dict, dict]:
    root = _safe_root(path)
    project_path = _managed_path(root, "project.json")
    current_path = _managed_path(root, "current.json")
    project = _read_json(project_path)
    current = _read_json(current_path)
    if project.get("format_version") != 1 or current.get("format_version") != 1:
        raise ValueError("unsupported Archy project format")
    _valid_uuid(project.get("project_id"), "project id")
    _valid_uuid(current.get("revision_id"), "current revision id")
    generation = current.get("generation")
    if isinstance(generation, bool) or not isinstance(generation, int) or generation < 1:
        raise ValueError("invalid current revision generation")
    if not isinstance(project.get("brief_project_id"), str) or not project["brief_project_id"]:
        raise ValueError("invalid brief project id")
    if not isinstance(project.get("created_at"), str):
        raise ValueError("invalid project creation time")
    return root, project, current


def _load_revision(
    root: Path, revision_id: str, expected_project_id: str, expected_brief_project_id: str
) -> tuple[Path, dict, bytes]:
    revision_id = _valid_uuid(revision_id, "revision id")
    relative = f"revisions/rev-{revision_id}"
    revision_path = _managed_path(root, relative)
    metadata = _read_json(_managed_path(root, f"{relative}/revision.json"))
    brief_path = _managed_path(root, f"{relative}/brief.yaml")
    if metadata.get("format_version") != 1 or metadata.get("revision_id") != revision_id:
        raise ValueError(f"invalid revision metadata: {revision_id}")
    if metadata.get("project_id") != expected_project_id:
        raise ValueError(f"revision belongs to a different project: {revision_id}")
    try:
        brief_bytes = brief_path.read_bytes()
    except OSError as error:
        raise ValueError(f"cannot read revision brief: {error}") from error
    if _sha256(brief_bytes) != metadata.get("brief_sha256"):
        raise ValueError(f"revision brief hash mismatch: {revision_id}")
    evaluation = evaluate_brief(brief_path)
    if evaluation["source"]["sha256"] != metadata["brief_sha256"]:
        raise ValueError(f"revision brief changed while loading: {revision_id}")
    if evaluation["project"]["id"] != expected_brief_project_id:
        raise ValueError(f"revision belongs to a different brief project: {revision_id}")
    return revision_path, metadata, brief_bytes


def read_project_revision(project_path: str | Path, revision_id: str | None = None) -> dict[str, object]:
    """Read a project's selected revision metadata and exact validated brief text."""
    root, project, current = _load_project(project_path)
    chosen_revision = current["revision_id"] if revision_id is None else _valid_uuid(revision_id, "revision id")
    _, revision, brief_bytes = _load_revision(
        root, chosen_revision, project["project_id"], project["brief_project_id"]
    )
    try:
        brief_text = brief_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError("revision brief is not valid UTF-8 text") from error
    return {
        "project": {
            "project_id": project["project_id"],
            "brief_project_id": project["brief_project_id"],
            "created_at": project["created_at"],
            "current_revision_id": current["revision_id"],
            "current_generation": current["generation"],
        },
        "revision": {
            "revision_id": revision["revision_id"],
            "parent_revision_id": revision["parent_revision_id"],
            "created_at": revision["created_at"],
            "message": revision["message"],
            "brief_sha256": revision["brief_sha256"],
            "is_current": revision["revision_id"] == current["revision_id"],
        },
        "brief_yaml": brief_text,
    }


def _parse_brief_bytes(path: Path, brief_bytes: bytes) -> dict:
    path.write_bytes(brief_bytes)
    return evaluate_brief(path)


def _new_staging(parent: Path, prefix: str) -> Path:
    return Path(tempfile.mkdtemp(prefix=prefix, dir=parent))


def _publish_directory(staging: Path, destination: Path) -> None:
    if os.path.lexists(destination):
        raise FileExistsError(f"Archy destination already exists: {destination}")
    os.replace(staging, destination)
    _fsync_directory(destination.parent)


def initialize_project(
    path: str | Path,
    brief_path: str | Path,
    *,
    project_id: str | None = None,
) -> Project:
    """Create a project whose first revision contains the exact validated brief."""
    destination = Path(path).expanduser().absolute()
    if project_id is not None:
        project_id = _valid_uuid(project_id, "project id")
        if destination.name != project_id:
            raise ValueError("project directory name must match its project id")
    if destination.is_symlink() or os.path.lexists(destination):
        raise FileExistsError(f"project destination already exists: {destination}")
    parent = destination.parent.resolve()
    destination = parent / destination.name
    parent.mkdir(parents=True, exist_ok=True)
    brief_bytes = Path(brief_path).read_bytes()
    project_id = str(uuid.uuid4()) if project_id is None else project_id
    revision_id = str(uuid.uuid4())
    created_at = _utc_now()
    staging = _new_staging(parent, f".{destination.name}.staging-")
    try:
        (staging / "revisions").mkdir()
        revision_dir = staging / "revisions" / f"rev-{revision_id}"
        revision_dir.mkdir()
        snapshot = revision_dir / "brief.yaml"
        evaluation = _parse_brief_bytes(snapshot, brief_bytes)
        brief_project_id = evaluation["project"]["id"]
        if not isinstance(brief_project_id, str) or not brief_project_id:
            raise ValueError("brief project id is missing")
        (revision_dir / "revision.json").write_bytes(_json_bytes({
            "format_version": 1,
            "project_id": project_id,
            "revision_id": revision_id,
            "parent_revision_id": None,
            "created_at": created_at,
            "message": "Initial revision",
            "brief_sha256": _sha256(brief_bytes),
        }))
        (staging / "project.json").write_bytes(_json_bytes({
            "format_version": 1,
            "project_id": project_id,
            "brief_project_id": brief_project_id,
            "created_at": created_at,
        }))
        (staging / "current.json").write_bytes(_json_bytes({
            "format_version": 1,
            "revision_id": revision_id,
            "generation": 1,
        }))
        (staging / "runs").mkdir()
        _publish_directory(staging, destination)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise
    return Project(destination, project_id, brief_project_id, created_at, revision_id, 1)


def create_revision(
    project_path: str | Path,
    brief_path: str | Path,
    message: str,
    parent_revision_id: str | None = None,
) -> Revision:
    """Publish a new immutable revision without changing the current pointer."""
    root, project, current = _load_project(project_path)
    if not isinstance(message, str) or not message.strip():
        raise ValueError("revision message must be a non-empty string")
    parent_id = current["revision_id"] if parent_revision_id is None else _valid_uuid(parent_revision_id, "parent revision id")
    _load_revision(root, parent_id, project["project_id"], project["brief_project_id"])
    brief_bytes = Path(brief_path).read_bytes()
    revision_id = str(uuid.uuid4())
    created_at = _utc_now()
    revisions = _managed_path(root, "revisions")
    staging = _new_staging(revisions, ".staging-")
    try:
        brief_snapshot = staging / "brief.yaml"
        evaluation = _parse_brief_bytes(brief_snapshot, brief_bytes)
        if evaluation["project"]["id"] != project["brief_project_id"]:
            raise ValueError("brief project.id does not match this project's stable brief project id")
        metadata = {
            "format_version": 1,
            "project_id": project["project_id"],
            "revision_id": revision_id,
            "parent_revision_id": parent_id,
            "created_at": created_at,
            "message": message,
            "brief_sha256": _sha256(brief_bytes),
        }
        (staging / "revision.json").write_bytes(_json_bytes(metadata))
        destination = revisions / f"rev-{revision_id}"
        _publish_directory(staging, destination)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise
    return Revision(revision_id, parent_id, created_at, message, _sha256(brief_bytes), destination, False, "valid", None)


def _project_lock(root: Path):
    lock_path = _managed_path(root, ".project.lock")
    if lock_path.exists() and (lock_path.is_symlink() or not lock_path.is_file()):
        raise ValueError("project lock must be a regular file")
    return lock_path.open("a+b")


def promote_revision(
    project_path: str | Path,
    revision_id: str,
    expected_current_revision_id: str,
) -> Project:
    """Promote a revision only if the caller's expected current revision is still current."""
    root, project, current = _load_project(project_path)
    revision_id = _valid_uuid(revision_id, "revision id")
    expected = _valid_uuid(expected_current_revision_id, "expected current revision id")
    _load_revision(root, revision_id, project["project_id"], project["brief_project_id"])
    with _project_lock(root) as lock_file:
        fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
        root, project, current = _load_project(root)
        if current["revision_id"] != expected:
            raise ValueError("stale promotion: current revision changed")
        if revision_id == current["revision_id"]:
            return Project(root, project["project_id"], project["brief_project_id"], project["created_at"],
                           revision_id, current["generation"])
        next_current = {
            "format_version": 1,
            "revision_id": revision_id,
            "generation": current["generation"] + 1,
        }
        _write_atomic(_managed_path(root, "current.json"), _json_bytes(next_current))
    return Project(root, project["project_id"], project["brief_project_id"], project["created_at"], revision_id, next_current["generation"])


def _publish_run_record(staging: Path, destination: Path, run_record: dict) -> None:
    (staging / "run.json").write_bytes(_json_bytes(run_record))
    _publish_directory(staging, destination)


def run_project(
    project_path: str | Path,
    revision_id: str | None = None,
    blender: str = "blender",
    *,
    quiet: bool = False,
) -> StudyRun:
    """Run one immutable revision and publish either a complete bundle or a failure record."""
    root, project, current = _load_project(project_path)
    chosen_revision = current["revision_id"] if revision_id is None else _valid_uuid(revision_id, "revision id")
    _, revision, brief_bytes = _load_revision(
        root, chosen_revision, project["project_id"], project["brief_project_id"]
    )
    runs_dir = _managed_path(root, "runs")
    if not runs_dir.exists():
        runs_dir.mkdir()
    run_id = str(uuid.uuid4())
    created_at = _utc_now()
    destination = runs_dir / f"run-{run_id}"
    staging: Path | None = None
    bundle: Path | None = None
    snapshot_verified = False
    try:
        from .workflow import run_workflow
        from .verify import verify_output

        staging = _new_staging(runs_dir, ".staging-")
        snapshot = staging / "brief.yaml"
        snapshot.write_bytes(brief_bytes)
        if _sha256(snapshot.read_bytes()) != revision["brief_sha256"]:
            raise ValueError("revision brief changed while preparing study")
        snapshot_verified = True
        bundle = staging / "bundle"
        run_workflow(snapshot, bundle, blender, quiet=quiet)
        (staging / ".bundle.lock").unlink(missing_ok=True)
        evaluation_path = bundle / "evaluation.json"
        evaluation = json.loads(evaluation_path.read_text(encoding="utf-8"))
        source = evaluation.get("source")
        if not isinstance(source, dict) or source.get("sha256") != revision["brief_sha256"]:
            raise ValueError("workflow evaluation source hash does not match revision")
        results = evaluation.get("results")
        if not isinstance(results, list):
            raise ValueError("workflow evaluation has no results list")
        valid_count = sum(1 for result in results if isinstance(result, dict) and result.get("valid") is True)
        invalid_count = len(results) - valid_count
        run_record = {
            "format_version": 1,
            "project_id": project["project_id"],
            "run_id": run_id,
            "revision_id": chosen_revision,
            "created_at": created_at,
            "status": "completed",
            "brief_sha256": revision["brief_sha256"],
            "comparison_path": "comparison.html",
            "valid_alternatives": valid_count,
            "invalid_alternatives": invalid_count,
        }
        shutil.move(str(bundle / "manifest.json"), str(staging / "manifest.json"))
        manifest = json.loads((staging / "manifest.json").read_text(encoding="utf-8"))
        entries = manifest.get("artifacts")
        if not isinstance(entries, dict):
            raise ValueError("workflow manifest has no artifact map")
        for name in ("brief.yaml", "run.json"):
            if name == "run.json":
                (staging / name).write_bytes(_json_bytes(run_record))
            file_path = staging / name
            content = file_path.read_bytes()
            entries[name] = {"sha256": _sha256(content), "size_bytes": len(content)}
        (staging / "manifest.json").write_bytes(_json_bytes(manifest))
        for child in list(bundle.iterdir()):
            os.replace(child, staging / child.name)
        bundle.rmdir()
        failures = verify_output(staging)
        if failures:
            raise ValueError("workflow bundle verification failed: " + "; ".join(failures))
        (staging / "manifest.json").write_bytes(_json_bytes(manifest))
        _publish_directory(staging, destination)
        return StudyRun(run_id, chosen_revision, created_at, "completed", revision["brief_sha256"], destination,
                        destination / "comparison.html", valid_count, invalid_count, None)
    except Exception as error:
        if staging is None or not staging.exists():
            staging = _new_staging(runs_dir, ".staging-")
        if bundle is not None:
            shutil.rmtree(bundle, ignore_errors=True)
        for child in list(staging.iterdir()):
            if child.name not in {"brief.yaml"}:
                if child.is_dir() and not child.is_symlink():
                    shutil.rmtree(child, ignore_errors=True)
                else:
                    child.unlink(missing_ok=True)
        snapshot = staging / "brief.yaml"
        if not snapshot_verified or snapshot.is_symlink() or not snapshot.is_file():
            if snapshot.is_dir() and not snapshot.is_symlink():
                shutil.rmtree(snapshot, ignore_errors=True)
            else:
                snapshot.unlink(missing_ok=True)
            snapshot.write_bytes(brief_bytes)
        elif _sha256(snapshot.read_bytes()) != revision["brief_sha256"]:
            snapshot.write_bytes(brief_bytes)
        message = str(error)[:4000] or error.__class__.__name__
        run_record = {
            "format_version": 1,
            "project_id": project["project_id"],
            "run_id": run_id,
            "revision_id": chosen_revision,
            "created_at": created_at,
            "status": "failed",
            "brief_sha256": revision["brief_sha256"],
            "comparison_path": None,
            "valid_alternatives": 0,
            "invalid_alternatives": 0,
            "error": message,
        }
        _publish_run_record(staging, destination, run_record)
        return StudyRun(run_id, chosen_revision, created_at, "failed", revision["brief_sha256"], destination,
                        None, 0, 0, message)


def _study_from_dir(
    root: Path, run_path: Path, expected_project_id: str, expected_brief_project_id: str
) -> StudyRun:
    revision_id = ""
    try:
        run_json = _read_json(_managed_path(root, f"{run_path.relative_to(root).as_posix()}/run.json"))
        run_id = _valid_uuid(run_json.get("run_id"), "run id")
        if run_path.name != f"run-{run_id}":
            raise ValueError("run metadata does not match containing directory")
        revision_id = _valid_uuid(run_json.get("revision_id"), "run revision id")
        created_at = run_json.get("created_at")
        status = run_json.get("status")
        brief_sha = run_json.get("brief_sha256")
        if run_json.get("format_version") != 1 or run_json.get("revision_id") != revision_id:
            raise ValueError("run metadata does not match containing revision")
        if run_json.get("project_id") != expected_project_id:
            raise ValueError("run belongs to a different project")
        if not isinstance(created_at, str) or status not in {"completed", "failed"}:
            raise ValueError("invalid run metadata")
        if not isinstance(brief_sha, str) or re.fullmatch(r"[0-9a-f]{64}", brief_sha) is None:
            raise ValueError("invalid run brief hash")
        _, revision, _ = _load_revision(root, revision_id, expected_project_id, expected_brief_project_id)
        if brief_sha != revision["brief_sha256"]:
            raise ValueError("run brief hash does not match its revision")
        brief_path = _managed_path(root, f"{run_path.relative_to(root).as_posix()}/brief.yaml")
        if _sha256(brief_path.read_bytes()) != brief_sha:
            raise ValueError("run brief hash mismatch")
        valid = run_json.get("valid_alternatives", 0)
        invalid = run_json.get("invalid_alternatives", 0)
        if any(isinstance(value, bool) or not isinstance(value, int) or value < 0 for value in (valid, invalid)):
            raise ValueError("invalid alternative counts")
        comparison = run_path / "comparison.html" if status == "completed" else None
        error = run_json.get("error")
        if status == "failed" and not isinstance(error, str):
            raise ValueError("failed run has no error description")
        if status == "completed":
            run_relative = run_path.relative_to(root).as_posix()
            comparison = _managed_path(root, f"{run_relative}/comparison.html")
            manifest_path = _managed_path(root, f"{run_relative}/manifest.json")
            if not comparison.is_file():
                raise ValueError("completed run is missing comparison.html")
            manifest = _read_json(manifest_path)
            entries = manifest.get("artifacts")
            if manifest.get("contract_version") != 2 or not isinstance(entries, dict):
                raise ValueError("completed run has an invalid workflow manifest")
            for name in ("brief.yaml", "run.json"):
                entry = entries.get(name)
                artifact = _managed_path(root, f"{run_relative}/{name}")
                if not isinstance(entry, dict):
                    raise ValueError(f"run manifest does not verify {name}")
                expected_hash = entry.get("sha256")
                expected_size = entry.get("size_bytes")
                if (not isinstance(expected_hash, str)
                        or re.fullmatch(r"[0-9a-f]{64}", expected_hash) is None
                        or isinstance(expected_size, bool)
                        or not isinstance(expected_size, int)
                        or not artifact.is_file()):
                    raise ValueError(f"run manifest has an invalid {name} entry")
                artifact_bytes = artifact.read_bytes()
                if len(artifact_bytes) != expected_size or _sha256(artifact_bytes) != expected_hash:
                    raise ValueError(f"run manifest hash or size mismatch for {name}")
        return StudyRun(run_id, revision_id, created_at, status, brief_sha, run_path, comparison, valid, invalid, error)
    except (OSError, ValueError, TypeError, KeyError):
        fallback_id = run_path.name.removeprefix("run-")
        try:
            run_id = _valid_uuid(fallback_id, "run id")
        except ValueError:
            run_id = fallback_id
        return StudyRun(run_id, revision_id, "", "corrupt", "", run_path, None, 0, 0, "run metadata or artifacts are corrupt")


def get_project_run(project_path: str | Path, run_id: str) -> StudyRun:
    """Return one validated project run by UUID, marking damaged runs as corrupt."""
    root, project, _ = _load_project(project_path)
    run_id = _valid_uuid(run_id, "run id")
    run_path = _managed_path(root, f"runs/run-{run_id}")
    if not run_path.is_dir():
        raise ValueError("run id was not found in this project")
    return _study_from_dir(root, run_path, project["project_id"], project["brief_project_id"])


def project_history(project_path: str | Path) -> ProjectHistory:
    """Return revisions and runs in stable creation order, marking damaged runs."""
    root, project_data, current = _load_project(project_path)
    current_id = current["revision_id"]
    project = Project(root, project_data["project_id"], project_data["brief_project_id"],
                      project_data["created_at"], current_id, current["generation"])
    revisions_dir = _managed_path(root, "revisions")
    revisions: list[Revision] = []
    runs: list[StudyRun] = []
    for path in sorted(revisions_dir.iterdir(), key=lambda item: item.name):
        if not path.name.startswith("rev-"):
            continue
        revision_id = path.name[4:]
        parent: str | None = None
        created_at = ""
        message = ""
        brief_sha256 = ""
        error: str | None = None
        try:
            mode = path.lstat().st_mode
        except OSError as caught:
            error = str(caught) or "cannot inspect revision entry"
            mode = 0
        if stat.S_ISLNK(mode):
            error = "revision entry is a symlink"
        elif not stat.S_ISDIR(mode):
            error = error or "revision entry is not a directory"
        else:
            try:
                revision_id = _valid_uuid(revision_id, "revision id")
                revision_path, metadata, _ = _load_revision(
                    root, revision_id, project_data["project_id"], project_data["brief_project_id"]
                )
                parent_value = metadata.get("parent_revision_id")
                if parent_value is not None:
                    parent = _valid_uuid(parent_value, "parent revision id")
                created_at = metadata.get("created_at")
                message = metadata.get("message")
                brief_sha256 = metadata.get("brief_sha256")
                if not isinstance(created_at, str) or not isinstance(message, str):
                    raise ValueError("invalid revision metadata")
            except (OSError, ValueError, TypeError, KeyError) as caught:
                error = str(caught) or "invalid revision metadata"
        revisions.append(Revision(
            revision_id=revision_id,
            parent_revision_id=parent,
            created_at=created_at if isinstance(created_at, str) else "",
            message=message if isinstance(message, str) else "",
            brief_sha256=brief_sha256 if isinstance(brief_sha256, str) else "",
            path=path,
            is_current=revision_id == current_id,
            status="corrupt" if error else "valid",
            error=error,
        ))

    revision_by_id = {revision.revision_id: revision for revision in revisions}
    for index, revision in enumerate(revisions):
        if revision.status != "valid" or revision.parent_revision_id is None:
            continue
        parent = revision_by_id.get(revision.parent_revision_id)
        if parent is None or parent.status != "valid" or parent.revision_id == revision.revision_id:
            revisions[index] = replace(
                revision,
                status="corrupt",
                error="revision parent is missing, invalid, or self-referential",
            )
    revision_by_id = {revision.revision_id: revision for revision in revisions}
    cycle_ids: set[str] = set()
    for revision in revisions:
        chain: list[str] = []
        positions: dict[str, int] = {}
        current = revision
        while current.status == "valid" and current.parent_revision_id is not None:
            if current.revision_id in positions:
                cycle_ids.update(chain[positions[current.revision_id]:])
                break
            positions[current.revision_id] = len(chain)
            chain.append(current.revision_id)
            parent = revision_by_id.get(current.parent_revision_id)
            if parent is None:
                break
            current = parent
    for index, revision in enumerate(revisions):
        if revision.revision_id in cycle_ids:
            revisions[index] = replace(revision, status="corrupt", error="revision parent chain contains a cycle")
    changed = True
    while changed:
        changed = False
        revision_by_id = {revision.revision_id: revision for revision in revisions}
        for index, revision in enumerate(revisions):
            if revision.status != "valid" or revision.parent_revision_id is None:
                continue
            parent = revision_by_id.get(revision.parent_revision_id)
            if parent is None or parent.status != "valid":
                revisions[index] = replace(
                    revision,
                    status="corrupt",
                    error="revision parent is missing or corrupt",
                )
                changed = True
    runs_dir = _managed_path(root, "runs")
    if runs_dir.is_symlink():
        raise ValueError("runs directory cannot be a symlink")
    if runs_dir.exists():
        for run_path in sorted(runs_dir.iterdir(), key=lambda item: item.name):
            if run_path.is_symlink() or not run_path.is_dir() or not run_path.name.startswith("run-"):
                continue
            runs.append(_study_from_dir(
                root, run_path, project_data["project_id"], project_data["brief_project_id"]
            ))
    revisions.sort(key=lambda item: (item.created_at, item.revision_id))
    runs.sort(key=lambda item: (item.created_at, item.run_id))
    return ProjectHistory(project, tuple(revisions), tuple(runs))
