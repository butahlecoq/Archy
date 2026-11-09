"""Expose a temporary, inspectable massing study over MCP."""

import asyncio
import importlib
import os
import shutil
import signal
import sys
import tempfile
from pathlib import Path
from typing import Any

from .massing import evaluate_brief
from .mcp_inputs import encode_brief_text
from .mcp_media import build_study_result


MAX_STUDY_ALTERNATIVES = 6
WORKFLOW_TIMEOUT_SECONDS = 300
_WORKFLOW_RUNNER = (
    "from pathlib import Path; import sys; "
    "from archy.workflow import run_workflow; "
    "run_workflow(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3])"
)


def _check_runtime() -> str:
    blender = shutil.which("blender")
    if blender is None:
        raise ValueError("Blender is required for previews. Install Blender and add it to PATH.")
    try:
        importlib.import_module("ifcopenshell.api")
        importlib.import_module("ifcopenshell.geom")
    except (ImportError, OSError) as error:
        raise ValueError("IfcOpenShell is required for verified studies. Install Archy with its '[ifc]' extra.") from error
    return blender


async def _drain_stderr(stream: asyncio.StreamReader, tail: bytearray) -> None:
    while chunk := await stream.read(4096):
        tail.extend(chunk)
        if len(tail) > 8192:
            del tail[:-8192]


def _process_group_exists(process_id: int) -> bool:
    try:
        os.killpg(process_id, 0)
    except ProcessLookupError:
        return False
    return True


async def _terminate_process_group(process_id: int, wait_task: asyncio.Task[int]) -> None:
    try:
        os.killpg(process_id, signal.SIGTERM)
    except ProcessLookupError:
        pass

    loop = asyncio.get_running_loop()
    deadline = loop.time() + 5
    while _process_group_exists(process_id) and loop.time() < deadline:
        await asyncio.sleep(0.05)

    if _process_group_exists(process_id):
        try:
            os.killpg(process_id, signal.SIGKILL)
        except ProcessLookupError:
            pass
    await wait_task


async def _wait_shielded(task: asyncio.Task[Any]) -> tuple[Any, bool]:
    cancelled = False
    while not task.done():
        try:
            await asyncio.shield(task)
        except asyncio.CancelledError:
            cancelled = True
    return task.result(), cancelled


async def _run_workflow_in_child(brief_path: Path, study_dir: Path, blender: str) -> None:
    environment = os.environ.copy()
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    runtime_root = brief_path.parent / "runtime"
    blender_paths = {
        "BLENDER_USER_RESOURCES": runtime_root / "blender-resources",
        "BLENDER_USER_CONFIG": runtime_root / "blender-config",
        "BLENDER_USER_SCRIPTS": runtime_root / "blender-scripts",
        "BLENDER_USER_DATAFILES": runtime_root / "blender-datafiles",
        "XDG_CONFIG_HOME": runtime_root / "xdg-config",
        "XDG_CACHE_HOME": runtime_root / "xdg-cache",
        "XDG_DATA_HOME": runtime_root / "xdg-data",
    }
    for name, path in blender_paths.items():
        path.mkdir(parents=True, exist_ok=True)
        environment[name] = str(path)
    package_root = str(Path(__file__).resolve().parent.parent)
    environment["PYTHONPATH"] = os.pathsep.join(
        item for item in (package_root, environment.get("PYTHONPATH", "")) if item
    )
    spawn_task = asyncio.create_task(asyncio.create_subprocess_exec(
        sys.executable,
        "-c",
        _WORKFLOW_RUNNER,
        str(brief_path),
        str(study_dir),
        blender,
        cwd=brief_path.parent,
        stdin=asyncio.subprocess.DEVNULL,
        stdout=asyncio.subprocess.DEVNULL,
        stderr=asyncio.subprocess.PIPE,
        env=environment,
        start_new_session=True,
    ))
    try:
        process = await asyncio.shield(spawn_task)
    except asyncio.CancelledError:
        process, _ = await _wait_shielded(spawn_task)
        wait_task = asyncio.create_task(process.wait())
        assert process.stderr is not None
        stderr_tail = bytearray()
        stderr_task = asyncio.create_task(_drain_stderr(process.stderr, stderr_tail))
        cleanup_task = asyncio.create_task(_terminate_process_group(process.pid, wait_task))
        await _wait_shielded(asyncio.create_task(_terminate_and_drain(cleanup_task, stderr_task)))
        raise

    wait_task = asyncio.create_task(process.wait())
    stderr_tail = bytearray()
    assert process.stderr is not None
    stderr_task = asyncio.create_task(_drain_stderr(process.stderr, stderr_tail))
    try:
        return_code = await asyncio.wait_for(asyncio.shield(wait_task), timeout=WORKFLOW_TIMEOUT_SECONDS)
    except asyncio.TimeoutError as error:
        cleanup_task = asyncio.create_task(
            _terminate_and_drain(
                asyncio.create_task(_terminate_process_group(process.pid, wait_task)), stderr_task
            )
        )
        _, cancelled = await _wait_shielded(cleanup_task)
        if cancelled:
            raise asyncio.CancelledError
        raise ValueError(f"massing study exceeded the {WORKFLOW_TIMEOUT_SECONDS}-second limit") from error
    except asyncio.CancelledError:
        cleanup_task = asyncio.create_task(
            _terminate_and_drain(
                asyncio.create_task(_terminate_process_group(process.pid, wait_task)), stderr_task
            )
        )
        await _wait_shielded(cleanup_task)
        raise
    await stderr_task
    if return_code != 0:
        detail = stderr_tail.decode("utf-8", errors="replace").strip()
        message = detail[-4000:] if detail else f"workflow process exited with status {return_code}"
        raise ValueError(f"massing study failed: {message}")


async def _terminate_and_drain(
    cleanup_task: asyncio.Task[None], stderr_task: asyncio.Task[None]
) -> None:
    await cleanup_task
    await stderr_task


def register_study_tool(server: Any) -> None:
    """Register the temporary massing study operation."""
    from mcp.types import CallToolResult

    @server.tool()
    async def preview_massing_study(brief_yaml: str) -> CallToolResult:
        """Run a temporary Blender and IFC study and return the comparison and previews.

        The complete version 1 YAML brief must be at most 1,000,000 UTF-8
        bytes and contain no more than six alternatives. The tool does not
        accept paths or retain generated files after returning.
        """
        brief_bytes = encode_brief_text(brief_yaml)
        with tempfile.TemporaryDirectory(prefix="archy-mcp-study-") as directory:
            temporary_dir = Path(directory)
            brief_path = temporary_dir / "brief.yaml"
            brief_path.write_bytes(brief_bytes)
            evaluation = evaluate_brief(brief_path)
            alternative_count = len(evaluation["results"])
            if alternative_count > MAX_STUDY_ALTERNATIVES:
                raise ValueError(
                    f"preview_massing_study accepts at most {MAX_STUDY_ALTERNATIVES} alternatives; "
                    f"received {alternative_count}"
                )
            blender = await asyncio.to_thread(_check_runtime)
            study_dir = temporary_dir / "study"
            await _run_workflow_in_child(brief_path, study_dir, blender)
            return build_study_result(study_dir)
