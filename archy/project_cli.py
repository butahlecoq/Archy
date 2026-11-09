import argparse
from dataclasses import asdict, is_dataclass
import json
from pathlib import Path
import subprocess
import sys
from typing import Any

from .project import (
    create_revision,
    initialize_project,
    project_history,
    promote_revision,
    run_project,
)


def _json_value(value: Any) -> Any:
    if is_dataclass(value) and not isinstance(value, type):
        return _json_value(asdict(value))
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, dict):
        return {key: _json_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_json_value(item) for item in value]
    return value


def _emit(value: Any) -> None:
    print(json.dumps(_json_value(value), indent=2, sort_keys=True))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="archy project", description="Manage persistent Archy projects")
    commands = parser.add_subparsers(dest="command", required=True)

    init = commands.add_parser("init", help="initialize a project")
    init.add_argument("dir", type=Path)
    init.add_argument("--brief", type=Path, required=True)

    revise = commands.add_parser("revise", help="create an immutable project revision")
    revise.add_argument("dir", type=Path)
    revise.add_argument("--brief", type=Path, required=True)
    revise.add_argument("--message", required=True)
    revise.add_argument("--parent", dest="parent_revision_id")

    run = commands.add_parser("run", help="run a study for a project revision")
    run.add_argument("dir", type=Path)
    run.add_argument("--revision", dest="revision_id")
    run.add_argument("--blender", default="blender")

    history = commands.add_parser("history", help="show project revision and study history")
    history.add_argument("dir", type=Path)

    promote = commands.add_parser("promote", help="move the current revision pointer")
    promote.add_argument("dir", type=Path)
    promote.add_argument("revision_id")
    promote.add_argument("--expected-current", dest="expected_current_revision_id", required=True)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _parser()
    args = parser.parse_args(argv)
    try:
        if args.command == "init":
            result = initialize_project(args.dir, args.brief)
        elif args.command == "revise":
            result = create_revision(
                args.dir,
                args.brief,
                message=args.message,
                parent_revision_id=args.parent_revision_id,
            )
        elif args.command == "run":
            result = run_project(args.dir, revision_id=args.revision_id, blender=args.blender)
        elif args.command == "history":
            result = project_history(args.dir)
        else:
            result = promote_revision(
                args.dir,
                args.revision_id,
                expected_current_revision_id=args.expected_current_revision_id,
            )
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        detail = str(error).strip() or error.__class__.__name__
        print(f"archy project: error: {detail}", file=sys.stderr)
        return 2
    _emit(result)
    if args.command == "run" and getattr(result, "status", None) == "failed":
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
