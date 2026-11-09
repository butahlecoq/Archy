"""Validation helpers for text inputs received over MCP."""

MAX_BRIEF_BYTES = 1_000_000


def encode_brief_text(brief_yaml: str) -> bytes:
    """Validate and encode a brief received from an MCP caller."""
    if not isinstance(brief_yaml, str):
        raise ValueError("brief_yaml must be text")
    if len(brief_yaml) > MAX_BRIEF_BYTES:
        raise ValueError(f"brief_yaml exceeds the {MAX_BRIEF_BYTES}-byte limit")
    try:
        brief_bytes = brief_yaml.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ValueError("brief_yaml must be valid UTF-8 text") from error
    if len(brief_bytes) > MAX_BRIEF_BYTES:
        raise ValueError(f"brief_yaml exceeds the {MAX_BRIEF_BYTES}-byte limit")
    return brief_bytes
