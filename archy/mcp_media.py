"""Package a massing study as MCP text and image content."""

from __future__ import annotations

import base64
import json
from pathlib import Path, PurePosixPath
import re
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from mcp.types import CallToolResult

MAX_COMPARISON_BYTES = 4 * 1024 * 1024
MAX_STUDY_PREVIEW_BYTES = 8 * 1024 * 1024
MAX_STUDY_ALTERNATIVES = 6
_PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
_SAFE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9_-]*\Z")


def _read_limited(path: Path, limit: int, label: str) -> bytes:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"{label} is missing or is not a regular file")
    with path.open("rb") as source:
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f"{label} exceeds the {limit}-byte limit")
    return data


def build_study_result(
    study_dir: Path,
    *,
    summary: dict[str, Any] | None = None,
    note: str = "The workflow verified the IFC model. The IFC file was temporary and is not attached.",
) -> CallToolResult:
    """Return the comparison and rendered alternatives without local paths."""
    from mcp.types import CallToolResult, ImageContent, TextContent

    study_dir = study_dir.resolve()
    comparison_path = study_dir / "comparison.json"
    try:
        comparison_bytes = _read_limited(comparison_path, MAX_COMPARISON_BYTES, "comparison report")
        comparison = json.loads(comparison_bytes)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read comparison report: {error}") from error
    if not isinstance(comparison, dict) or not isinstance(comparison.get("alternatives"), list):
        raise ValueError("comparison report must contain an alternatives list")

    alternatives = comparison["alternatives"]
    if not alternatives or len(alternatives) > MAX_STUDY_ALTERNATIVES:
        raise ValueError(f"study must contain between 1 and {MAX_STUDY_ALTERNATIVES} alternatives")

    root = study_dir.resolve()
    previews: list[tuple[str, bytes]] = []
    total_bytes = 0
    for alternative in alternatives:
        if not isinstance(alternative, dict):
            raise ValueError("comparison alternative entries must be objects")
        alt_id = alternative.get("id")
        render = alternative.get("render")
        if not isinstance(alt_id, str) or _SAFE_ID.fullmatch(alt_id) is None:
            raise ValueError("comparison alternative has an invalid id")
        expected_render = f"renders/{alt_id}.png"
        if render != expected_render:
            raise ValueError(f"alternative {alt_id} has an unexpected render path")
        relative_path = PurePosixPath(render)
        if relative_path.is_absolute() or ".." in relative_path.parts or relative_path.parts != ("renders", f"{alt_id}.png"):
            raise ValueError(f"alternative {alt_id} has an unsafe render path")

        render_path = study_dir.joinpath(*relative_path.parts)
        if render_path.is_symlink():
            raise ValueError(f"alternative {alt_id} render cannot be a symbolic link")
        try:
            resolved_render = render_path.resolve(strict=True)
            resolved_render.relative_to(root)
        except (OSError, ValueError) as error:
            raise ValueError(f"alternative {alt_id} render is missing or escapes the study directory") from error
        remaining = MAX_STUDY_PREVIEW_BYTES - total_bytes
        if remaining <= 0:
            raise ValueError(f"combined preview images exceed the {MAX_STUDY_PREVIEW_BYTES}-byte limit")
        image = _read_limited(resolved_render, remaining, f"alternative {alt_id} preview")
        if not image.startswith(_PNG_SIGNATURE):
            raise ValueError(f"alternative {alt_id} preview is not a PNG image")
        total_bytes += len(image)
        previews.append((alt_id, image))
        alternative.pop("render", None)

    comparison.pop("ifc_model", None)
    content: list[Any] = []
    if summary is not None:
        content.append(TextContent(type="text", text=json.dumps({"run": summary}, indent=2, sort_keys=True)))
    content.extend([
        TextContent(type="text", text=json.dumps(comparison, indent=2, sort_keys=True)),
        TextContent(type="text", text=note),
    ])
    for alt_id, image in previews:
        content.append(TextContent(type="text", text=f"Preview for alternative {alt_id}."))
        content.append(ImageContent(type="image", data=base64.b64encode(image).decode("ascii"), mimeType="image/png"))
    return CallToolResult(content=content)


def build_run_summary_result(summary: dict[str, Any], note: str) -> CallToolResult:
    """Return a path-free persistent run summary when previews are unavailable."""
    from mcp.types import CallToolResult, TextContent

    return CallToolResult(content=[
        TextContent(type="text", text=json.dumps({"run": summary}, indent=2, sort_keys=True)),
        TextContent(type="text", text=note),
    ])
