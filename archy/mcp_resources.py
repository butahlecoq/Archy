"""Expose Archy's fixed massing contracts as MCP resources."""

from pathlib import Path
import sys
from typing import Any


_RESOURCE_FILES = {
    "brief-schema-v1": (
        Path(__file__).resolve().parent.parent / "docs" / "massing-brief.schema.json",
        Path(sys.prefix) / "share" / "archy" / "schemas" / "massing-brief.schema.json",
    ),
    "massing-operations-v1": (
        Path(__file__).resolve().parent.parent / "docs" / "massing-operations.md",
        Path(sys.prefix) / "share" / "archy" / "massing-operations.md",
    ),
}


def _read_resource(name: str) -> str:
    for path in _RESOURCE_FILES[name]:
        if path.is_file():
            return path.read_text(encoding="utf-8")
    raise RuntimeError(f"Archy MCP resource is not installed: {name}")


def register_resources(server: Any) -> None:
    """Register the supported schema and operation contract."""

    @server.resource("archy://massing/brief-schema-v1", mime_type="application/schema+json")
    def massing_brief_schema_v1() -> str:
        """Return the JSON Schema for massing brief version 1."""
        return _read_resource("brief-schema-v1")

    @server.resource("archy://massing/operations-v1", mime_type="text/markdown")
    def massing_operations_v1() -> str:
        """Return the deterministic massing operation contract for version 1."""
        return _read_resource("massing-operations-v1")
