"""Expose the massing brief evaluator as a local MCP tool."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import tempfile
from typing import Any

from .massing import evaluate_brief
from .mcp_inputs import encode_brief_text
from .mcp_prompts import register_prompts
from .mcp_resources import register_resources


def _prepare_projects_root(projects_dir: str | Path | None = None) -> Path:
    """Create and validate the only persistent storage root exposed to MCP tools."""
    if projects_dir is None:
        configured = os.environ.get("ARCHY_PROJECTS_DIR")
        if configured:
            projects_dir = configured
        else:
            xdg_data_home = os.environ.get("XDG_DATA_HOME")
            data_home = Path(xdg_data_home).expanduser() if xdg_data_home else Path.home() / ".local" / "share"
            if not data_home.is_absolute():
                raise ValueError("XDG_DATA_HOME must be an absolute path")
            projects_dir = data_home / "archy" / "projects"

    candidate = Path(projects_dir).expanduser().absolute()
    if candidate.is_symlink():
        raise ValueError("projects directory cannot be a symlink")
    if candidate == candidate.parent:
        raise ValueError("projects directory cannot be a filesystem root")
    candidate.mkdir(parents=True, exist_ok=True, mode=0o700)
    if candidate.is_symlink() or not candidate.is_dir():
        raise ValueError("projects directory must be a regular directory, not a symlink")
    root = candidate.resolve(strict=True)
    if root == root.parent:
        raise ValueError("projects directory cannot be a filesystem root")
    return root


def build_server(projects_dir: str | Path | None = None) -> Any:
    """Create an MCP server without making the optional SDK a base dependency."""
    projects_root = _prepare_projects_root(projects_dir)
    try:
        from mcp.server.fastmcp import FastMCP
    except ImportError as error:
        raise RuntimeError(
            "The Archy MCP server requires the optional MCP dependencies; "
            "install Archy with its 'mcp' extra."
        ) from error

    server = FastMCP("Archy")
    register_resources(server)
    register_prompts(server)
    from .mcp_study import register_study_tool
    register_study_tool(server)
    from .mcp_projects import register_project_tools
    register_project_tools(server, projects_root)

    @server.tool()
    def evaluate_massing_brief(brief_yaml: str) -> dict[str, Any]:
        """Validate a massing brief and return measurements and constraint outcomes.

        The complete version 1 YAML brief must be at most 1,000,000 UTF-8 bytes.
        Constraint failures are returned as ordinary evaluation results so an
        agent can compare alternatives. Malformed or unsupported briefs fail
        the tool call.
        """
        brief_bytes = encode_brief_text(brief_yaml)
        with tempfile.TemporaryDirectory(prefix="archy-mcp-") as directory:
            brief_path = Path(directory) / "brief.yaml"
            brief_path.write_bytes(brief_bytes)
            return evaluate_brief(brief_path)

    return server


def main() -> int:
    """Serve Archy tools to a local MCP host over stdio."""
    parser = argparse.ArgumentParser(prog="archy-mcp", description="Serve Archy tools over MCP stdio")
    parser.add_argument(
        "--projects-dir",
        type=Path,
        help="persistent project directory (defaults to ARCHY_PROJECTS_DIR or the XDG data path)",
    )
    arguments = parser.parse_args()
    try:
        server = build_server(arguments.projects_dir)
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(f"archy-mcp: error: {error}") from error
    server.run(transport="stdio")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
