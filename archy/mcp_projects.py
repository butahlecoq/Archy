"""Persistent project lifecycle tools for the local MCP server."""

import asyncio
import json
from pathlib import Path
import re
import tempfile
from typing import Any
import uuid

from .massing import evaluate_brief
from .mcp_inputs import encode_brief_text
from .mcp_media import build_run_summary_result, build_study_result
from .project import (
    ProjectHistory,
    StudyRun,
    create_revision,
    get_project_run,
    initialize_project,
    project_history,
    promote_revision,
    read_project_revision,
    run_project,
)
from .verify import verify_output

MAX_PROJECT_RUN_ALTERNATIVES = 6
_QUOTED_FILE_URL = re.compile(
    r"(?P<quote>['\"])file:///[^\r\n'\"<>]*(?P=quote)", re.IGNORECASE
)
_FILE_URL = re.compile(r"(?i)\bfile:///[^\s'\"<>:;,]+")
_QUOTED_LOCAL_PATH = re.compile(r"(?P<quote>['\"])/[^'\"]+(?P=quote)")
_LOCAL_PATH = re.compile(r"(?<![\w.>/\]])/(?:[^\s/'\"<>:;,]+/)*[^\s/'\"<>:;,]+")
_LOCAL_PATH_CONTINUATION = re.compile(r"\s+[^/\s:]+(?:/[^/\s:]+)+")
_LOCAL_PATH_CONTEXT = re.compile(
    r"\b(?:artifact|command|could not|directory|error|failed|failure|file|invalid|"
    r"missing|no such|not found|path|permission|source|unable)\b",
    re.IGNORECASE,
)


def _projects_root(path: str | Path) -> Path:
    root = Path(path).expanduser().absolute()
    if root.is_symlink():
        raise ValueError("configured projects directory cannot be a symlink")
    root.mkdir(parents=True, exist_ok=True)
    root = root.resolve(strict=True)
    if not root.is_dir() or root == root.parent:
        raise ValueError("configured projects directory must be a non-root directory")
    return root


def _project_path(root: Path, project_id: str) -> Path:
    if not isinstance(project_id, str) or len(project_id) != 36:
        raise ValueError("invalid project id")
    try:
        if str(uuid.UUID(project_id)) != project_id:
            raise ValueError("invalid project id")
    except ValueError as error:
        raise ValueError("invalid project id") from error
    project_path = root / project_id
    if project_path.is_symlink() or not project_path.is_dir():
        raise ValueError("project id was not found in the configured projects directory")
    try:
        project_path.resolve(strict=True).relative_to(root)
    except (OSError, ValueError) as error:
        raise ValueError("project directory escapes the configured projects directory") from error
    metadata_path = project_path / "project.json"
    if metadata_path.is_symlink() or not metadata_path.is_file():
        raise ValueError("project metadata is missing or is not a regular file")
    try:
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("project metadata is invalid") from error
    if not isinstance(metadata, dict) or metadata.get("project_id") != project_id:
        raise ValueError("project directory name does not match its project id")
    return project_path


def _project_json(history: ProjectHistory) -> dict[str, Any]:
    project = history.project
    return {
        "project_id": project.project_id,
        "brief_project_id": project.brief_project_id,
        "created_at": project.created_at,
        "current_revision_id": project.current_revision_id,
        "current_generation": project.current_generation,
        "revision_count": len(history.revisions),
        "run_count": len(history.runs),
    }


def _revision_json(revision: Any, root: Path | None = None) -> dict[str, Any]:
    return {
        "revision_id": revision.revision_id,
        "parent_revision_id": revision.parent_revision_id,
        "created_at": revision.created_at,
        "message": revision.message,
        "brief_sha256": revision.brief_sha256,
        "is_current": revision.is_current,
        "status": revision.status,
        "error": _redact_text(revision.error, root) if revision.error else None,
    }


def _run_json(run: StudyRun, root: Path | None = None) -> dict[str, Any]:
    return {
        "run_id": run.run_id,
        "revision_id": run.revision_id,
        "created_at": run.created_at,
        "status": run.status,
        "brief_sha256": run.brief_sha256,
        "comparison_available": run.comparison_html is not None,
        "valid_alternatives": run.valid_alternatives,
        "invalid_alternatives": run.invalid_alternatives,
        "error": _redact_text(run.error, root) if run.error else None,
    }


async def _durable_thread_call(function: Any, *args: Any, **kwargs: Any) -> Any:
    """Wait for durable worker work to finish before honoring cancellation."""
    task = asyncio.create_task(asyncio.to_thread(function, *args, **kwargs))
    try:
        return await asyncio.shield(task)
    except asyncio.CancelledError:
        while not task.done():
            try:
                await asyncio.shield(task)
            except asyncio.CancelledError:
                continue
            except Exception:
                break
        # Retrieve any worker exception so it does not become an unobserved task error.
        if task.done() and not task.cancelled():
            try:
                task.result()
            except Exception:
                pass
        raise


def _redact(error: BaseException, root: Path) -> str:
    detail = str(error).strip() or error.__class__.__name__
    return _redact_text(detail, root)


def _redact_text(detail: str, root: Path | None) -> str:
    detail = _QUOTED_FILE_URL.sub(
        lambda match: f"{match.group('quote')}<local-path>{match.group('quote')}", detail
    )
    detail = _FILE_URL.sub("<local-path>", detail)
    if root is not None:
        root_path = (
            re.escape(str(root))
            + r"(?=$|[/\s'\"<>:;,])(?:/[^\r\n'\"<>:;,]*)?"
        )
        detail = re.sub(root_path, "<projects-root>", detail)
    detail = _QUOTED_LOCAL_PATH.sub(
        lambda match: f"{match.group('quote')}<local-path>{match.group('quote')}", detail
    )
    redacted_lines = []
    for line in detail.splitlines(keepends=True):
        content = line.rstrip("\r\n")
        line_ending = line[len(content):]
        path = _LOCAL_PATH.search(content)
        suffix = content[path.end():] if path else ""
        has_unquoted_path_space = (
            path and path.end() < len(content) and content[path.end()].isspace()
        )
        if path and has_unquoted_path_space and (
            path.start() == 0
            or _LOCAL_PATH_CONTEXT.search(content[:path.start()])
            or _LOCAL_PATH_CONTINUATION.search(suffix)
        ):
            detail_end = content.find(":", path.end())
            if detail_end >= 0:
                content = f"{content[:path.start()]}<local-path>{content[detail_end:]}"
            else:
                content = f"{content[:path.start()]}<local-path>"
        else:
            content = _LOCAL_PATH.sub("<local-path>", content)
        redacted_lines.append(content + line_ending)
    return "".join(redacted_lines)


def register_project_tools(server: Any, projects_root: str | Path) -> None:
    """Register persistent lifecycle operations scoped to one managed directory."""
    from mcp.types import CallToolResult

    root = _projects_root(projects_root)

    @server.tool()
    async def list_archy_projects() -> dict[str, Any]:
        """List projects stored in Archy's configured projects directory."""
        projects: list[dict[str, Any]] = []
        for child in root.iterdir():
            if child.is_symlink() or not child.is_dir() or len(child.name) != 36:
                continue
            try:
                project_path = _project_path(root, child.name)
                history = await asyncio.to_thread(project_history, project_path)
            except (OSError, ValueError, RuntimeError):
                continue
            projects.append(_project_json(history))
        projects.sort(key=lambda project: (project["created_at"], project["project_id"]))
        return {"projects": projects}

    @server.tool()
    async def create_archy_project(brief_yaml: str) -> dict[str, Any]:
        """Create a persistent project from a complete version 1 brief."""
        brief_bytes = encode_brief_text(brief_yaml)
        project_id = str(uuid.uuid4())
        destination = root / project_id
        try:
            with tempfile.NamedTemporaryFile(prefix=".archy-brief-", suffix=".yaml", dir=root, delete=False) as stream:
                stream.write(brief_bytes)
                brief_path = Path(stream.name)
            project = await _durable_thread_call(
                initialize_project, destination, brief_path, project_id=project_id
            )
            history = await asyncio.to_thread(project_history, project.path)
            return _project_json(history)
        except (OSError, ValueError, RuntimeError) as error:
            raise ValueError(_redact(error, root)) from None
        finally:
            if "brief_path" in locals():
                brief_path.unlink(missing_ok=True)

    @server.tool()
    async def read_archy_project(project_id: str, revision_id: str | None = None) -> dict[str, Any]:
        """Read the selected or requested immutable revision and its brief text."""
        try:
            project_path = _project_path(root, project_id)
            return await asyncio.to_thread(read_project_revision, project_path, revision_id)
        except (OSError, ValueError, RuntimeError) as error:
            raise ValueError(_redact(error, root)) from None

    @server.tool()
    async def create_archy_revision(
        project_id: str,
        brief_yaml: str,
        message: str,
        parent_revision_id: str | None = None,
    ) -> dict[str, Any]:
        """Create an immutable proposal without changing the current revision."""
        brief_bytes = encode_brief_text(brief_yaml)
        try:
            project_path = _project_path(root, project_id)
            with tempfile.NamedTemporaryFile(prefix=".archy-brief-", suffix=".yaml", dir=root, delete=False) as stream:
                stream.write(brief_bytes)
                brief_path = Path(stream.name)
            revision = await _durable_thread_call(
                create_revision, project_path, brief_path, message, parent_revision_id
            )
            return _revision_json(revision, root)
        except (OSError, ValueError, RuntimeError) as error:
            raise ValueError(_redact(error, root)) from None
        finally:
            if "brief_path" in locals():
                brief_path.unlink(missing_ok=True)

    @server.tool()
    async def run_archy_project_study(
        project_id: str, revision_id: str | None = None
    ) -> CallToolResult:
        """Run a durable study for the selected or requested project revision."""
        try:
            project_path = _project_path(root, project_id)
            selected = await asyncio.to_thread(read_project_revision, project_path, revision_id)
            brief_text = selected["brief_yaml"]
            with tempfile.TemporaryDirectory(prefix="archy-project-check-") as directory:
                brief_path = Path(directory) / "brief.yaml"
                brief_path.write_text(str(brief_text), encoding="utf-8")
                evaluation = await asyncio.to_thread(evaluate_brief, brief_path)
            alternative_count = len(evaluation["results"])
            if alternative_count > MAX_PROJECT_RUN_ALTERNATIVES:
                raise ValueError(
                    f"project studies accept at most {MAX_PROJECT_RUN_ALTERNATIVES} alternatives; "
                    f"received {alternative_count}"
                )
            run = await _durable_thread_call(run_project, project_path, revision_id, quiet=True)
            summary = _run_json(run, root)
            if run.status != "completed":
                return build_run_summary_result(
                    summary,
                    "The project study failed and its failure record was saved. "
                    f"Reason: {summary['error'] or 'no failure detail was recorded.'}",
                )
            try:
                return build_study_result(
                    run.path,
                    summary=summary,
                    note="The workflow verified the IFC model. The IFC and complete study bundle "
                    "were saved to this project's run.",
                )
            except Exception as error:
                detail = _redact(error, root)
                return build_run_summary_result(
                    summary,
                    "The project study completed and its bundle was saved, but comparison previews "
                    f"could not be returned: {detail}",
                )
        except (OSError, ValueError, RuntimeError) as error:
            raise ValueError(_redact(error, root)) from None

    @server.tool()
    async def get_archy_project_history(project_id: str) -> dict[str, Any]:
        """Return revision and study history without local filesystem paths."""
        try:
            history = await asyncio.to_thread(project_history, _project_path(root, project_id))
            return {
                "project": _project_json(history),
                "revisions": [_revision_json(item, root) for item in history.revisions],
                "runs": [_run_json(item, root) for item in history.runs],
            }
        except (OSError, ValueError, RuntimeError) as error:
            raise ValueError(_redact(error, root)) from None

    @server.tool()
    async def get_archy_project_run(project_id: str, run_id: str) -> CallToolResult:
        """Retrieve a saved project run by UUID without launching Blender."""
        try:
            project_path = _project_path(root, project_id)
            run = await asyncio.to_thread(get_project_run, project_path, run_id)
            summary = _run_json(run, root)
            if run.status != "completed":
                summary["artifact_integrity"] = "unverified"
                return build_run_summary_result(
                    summary,
                    "This saved run has no retrievable media because it did not complete successfully.",
                )

            try:
                failures = await asyncio.to_thread(verify_output, run.path)
            except Exception:
                failures = ["verification unavailable"]
            if failures:
                summary["artifact_integrity"] = "unverified"
                return build_run_summary_result(
                    summary,
                    "Saved study artifacts could not be verified, so no comparison or previews were returned.",
                )

            summary["artifact_integrity"] = "verified"
            try:
                return build_study_result(
                    run.path,
                    summary=summary,
                    note="Retrieved the saved, verified study comparison and previews.",
                )
            except Exception:
                return build_run_summary_result(
                    summary,
                    "Saved study artifacts were verified, but comparison previews could not be packaged.",
                )
        except (OSError, ValueError, RuntimeError) as error:
            raise ValueError(_redact(error, root)) from None

    @server.tool()
    async def promote_archy_revision(
        project_id: str, revision_id: str, expected_current_revision_id: str
    ) -> dict[str, Any]:
        """Promote a revision only if the expected current revision still matches."""
        try:
            project = await asyncio.to_thread(
                promote_revision,
                _project_path(root, project_id),
                revision_id,
                expected_current_revision_id,
            )
            return {
                "project_id": project.project_id,
                "current_revision_id": project.current_revision_id,
                "current_generation": project.current_generation,
            }
        except (OSError, ValueError, RuntimeError) as error:
            raise ValueError(_redact(error, root)) from None
