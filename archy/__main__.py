import argparse
import json
import sys

from .massing import BriefError, evaluate_brief


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    if arguments and arguments[0] == "project":
        from .project_cli import main as project_main

        return project_main(arguments[1:])
    if arguments and arguments[0] == "doctor":
        from .doctor import main as doctor_main

        return doctor_main(arguments[1:])

    parser = argparse.ArgumentParser(description="Validate and measure an Archy massing brief")
    parser.add_argument("brief", help="path to massing brief YAML")
    args = parser.parse_args(arguments)
    try:
        result = evaluate_brief(args.brief)
    except BriefError as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if all(item["valid"] for item in result["results"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
