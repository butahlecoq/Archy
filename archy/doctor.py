"""Read-only diagnostics for the local Archy runtime."""

import argparse
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Any


_BLENDER_PROBE_MARKER = "ARCHY_BLENDER_RUNTIME_PROBE_V1:"
_BLENDER_PROBE_EXPRESSION = (
    "import bpy, math, hashlib, subprocess, sys, json; "
    f"print({_BLENDER_PROBE_MARKER!r} + json.dumps({{"
    "'version': bpy.app.version_string, "
    "'python': sys.version.split()[0], "
    "'prefix': sys.prefix"
    "}))"
)


def _check(name: str, status: str, detail: str, remedy: str | None = None) -> dict[str, str]:
    result = {"name": name, "status": status, "detail": detail}
    if remedy:
        result["remedy"] = remedy
    return result


def _blender_environment() -> dict[str, str]:
    env = os.environ.copy()
    env["PATH"] = os.defpath
    for name in tuple(env):
        if name.startswith("PYTHON"):
            env.pop(name)
    return env


def _blender_check() -> dict[str, str]:
    executable = shutil.which("blender", path=os.defpath)
    if executable is None:
        return _check("blender", "unavailable", "Blender was not found on the system PATH.",
                      "Install Blender and ensure its executable is on the system PATH.")
    try:
        result = subprocess.run(
            [executable, "--background", "--factory-startup", "--python-exit-code", "1",
             "--python-expr", _BLENDER_PROBE_EXPRESSION],
            capture_output=True, text=True, errors="replace", timeout=10,
            check=False, env=_blender_environment(),
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return _check("blender", "unavailable", f"Blender runtime probe failed: {error}.",
                      "Check the Blender installation and that it can start in background mode within 10 seconds.")
    if result.returncode:
        return _check("blender", "unavailable",
                      f"Blender runtime probe exited with status {result.returncode}.",
                      "Check Blender's background startup and Python runtime; review its startup errors.")

    payload_lines = [line[len(_BLENDER_PROBE_MARKER):] for line in result.stdout.splitlines()
                     if line.startswith(_BLENDER_PROBE_MARKER)]
    if len(payload_lines) != 1:
        return _check("blender", "unavailable",
                      "Blender started but did not return exactly one runtime probe marker.",
                      "Check Blender's startup output and Python runtime, then retry the diagnostic.")
    try:
        payload = json.loads(payload_lines[0])
    except (json.JSONDecodeError, TypeError):
        return _check("blender", "unavailable", "Blender returned malformed runtime probe data.",
                      "Check Blender's startup output and Python runtime, then retry the diagnostic.")
    if (not isinstance(payload, dict)
            or any(not isinstance(payload.get(key), str) or not payload[key]
                   for key in ("version", "python", "prefix"))):
        return _check("blender", "unavailable", "Blender returned incomplete runtime probe data.",
                      "Check Blender's startup output and Python runtime, then retry the diagnostic.")
    return _check("blender", "available",
                  f"{Path(executable)} (Blender {payload['version']}; Python {payload['python']}; "
                  f"prefix {payload['prefix']}).")


def _ifc_check() -> dict[str, str]:
    try:
        import ifcopenshell.api  # noqa: F401
        import ifcopenshell.geom  # noqa: F401
        import ifcopenshell
    except (ImportError, OSError) as error:
        return _check("ifc", "unavailable", f"IfcOpenShell API and geometry support could not be imported ({error}).",
                      "Install Archy's IFC extra in this Python environment: python -m pip install '.[ifc]'.")
    version = getattr(ifcopenshell, "version", None)
    if not version:
        try:
            version = importlib.metadata.version("ifcopenshell")
        except importlib.metadata.PackageNotFoundError:
            version = "version not reported"
    return _check("ifc", "available", f"IfcOpenShell {version}; API and geometry modules import successfully.")


def diagnose() -> dict[str, Any]:
    checks = [
        _check("python", "available", f"{sys.executable} (Python {sys.version.split()[0]})."),
        _blender_check(),
        _ifc_check(),
        _check("compatibility", "deferred", "Cross-version compatibility is not checked by this diagnostic."),
        _check("packaging", "deferred", "Installed-package completeness and packaging health are not checked."),
        _check("isolation", "deferred", "Runtime isolation and environment contamination are not fully audited."),
    ]
    return {"checks": checks}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args(argv)
    report = diagnose()
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        for item in report["checks"]:
            print(f"{item['status'].upper():11} {item['name']}: {item['detail']}")
            if item.get("remedy"):
                print(f"            Remedy: {item['remedy']}")
    return int(any(item["status"] == "unavailable" for item in report["checks"]))
