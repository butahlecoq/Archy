"""Fixed, isolated-module entry point for the IFC worker."""

import argparse
from contextlib import ExitStack
from dataclasses import asdict, dataclass
import hashlib
import json
import os
from pathlib import Path
import selectors
import signal
import stat
import subprocess
import sys
import time

from . import VERSION, protocol

NATIVE_TIMEOUT_S = 30
MAX_DIAGNOSTIC_BYTES = 65536


@dataclass(frozen=True)
class NativeResult:
    returncode: int | None
    stdout: str
    stderr: str
    failure: str | None


def run_native_process(argv, timeout_s=NATIVE_TIMEOUT_S, max_output_bytes=MAX_DIAGNOSTIC_BYTES):
    """Run one trusted child with per-stream caps and a whole-process-group deadline."""
    try:
        process = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True, close_fds=True)
    except OSError as exc:
        return NativeResult(None, "", str(exc)[:max_output_bytes], "launch_error")
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    failure = None
    deadline = time.monotonic() + timeout_s
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ, "stdout")
        selector.register(process.stderr, selectors.EVENT_READ, "stderr")
        try:
            while selector.get_map() or process.poll() is None:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    failure = "timeout"
                    break
                if not selector.get_map():
                    time.sleep(min(remaining, 0.01))
                    continue
                for key, _ in selector.select(min(remaining, 0.05)):
                    block = os.read(key.fileobj.fileno(), 4096)
                    if not block:
                        selector.unregister(key.fileobj)
                        key.fileobj.close()
                        continue
                    buffer = buffers[key.data]
                    available = max_output_bytes - len(buffer)
                    buffer.extend(block[:available])
                    if len(block) > available:
                        failure = "output_limit"
                        break
                if failure:
                    break
        finally:
            if failure or process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            for pipe in (process.stdout, process.stderr):
                if not pipe.closed:
                    pipe.close()
            try:
                process.wait(timeout=0.5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=0.5)
    return NativeResult(process.returncode,
                        buffers["stdout"].decode("utf-8", errors="replace"),
                        buffers["stderr"].decode("utf-8", errors="replace"), failure)


def prepare_output(project_root, logical_path):
    protocol.relative(logical_path, "output_dir")
    root = Path(project_root)
    if not root.is_absolute() or root.is_symlink() or not root.is_dir():
        protocol.fail("invalid project root", "project_root", "PERMISSION_DENIED")
    current = root
    parts = logical_path.split("/")
    for part in parts[:-1]:
        current = current / part
        if current.is_symlink() or not current.is_dir():
            protocol.fail("unsafe or absent output parent", "output_dir", "PERMISSION_DENIED")
    output = current / parts[-1]
    try:
        output.mkdir(mode=0o700)
    except FileExistsError:
        protocol.fail("output destination is occupied", "output_dir", "PERMISSION_DENIED")
    return output


def input_bytes(project_root, request_path):
    def components(value):
        raw = os.fspath(value)
        if type(raw) is not str or not raw.startswith("/") or len(raw) > 4096:
            protocol.fail("noncanonical absolute path", "request", "PERMISSION_DENIED")
        parts = raw[1:].split("/")
        if any(part in ("", ".", "..") or "\x00" in part for part in parts):
            protocol.fail("noncanonical path component", "request", "PERMISSION_DENIED")
        return parts

    root_parts = components(project_root)
    request_parts = components(request_path)
    if request_parts[:len(root_parts)] != root_parts or len(request_parts) <= len(root_parts):
        protocol.fail("request must be within project root", "request", "PERMISSION_DENIED")
    try:
        with ExitStack() as stack:
            directory = os.open("/", os.O_RDONLY | os.O_DIRECTORY)
            stack.callback(os.close, directory)
            for part in request_parts[:-1]:
                directory = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                    dir_fd=directory)
                stack.callback(os.close, directory)
            source = os.open(request_parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                             dir_fd=directory)
            stack.callback(os.close, source)
            info = os.fstat(source)
            if not stat.S_ISREG(info.st_mode):
                protocol.fail("request must be a regular file", "request", "PERMISSION_DENIED")
            if info.st_size > protocol.MAX_REQUEST_BYTES:
                protocol.fail("request exceeds 1 MiB", "request")
            chunks = []
            remaining = protocol.MAX_REQUEST_BYTES + 1
            while remaining:
                block = os.read(source, min(65536, remaining))
                if not block:
                    break
                chunks.append(block)
                remaining -= len(block)
            data = b"".join(chunks)
            if len(data) > protocol.MAX_REQUEST_BYTES:
                protocol.fail("request exceeds 1 MiB", "request")
            return data
    except OSError as exc:
        raise protocol.ProtocolError("unsafe or unavailable request path", "request",
                                     "PERMISSION_DENIED") from exc


def artifact(path, media_type):
    data = path.read_bytes()
    return {"path": path.name, "sha256": hashlib.sha256(data).hexdigest(),
            "media_type": media_type, "producer": VERSION, "size_bytes": len(data)}


def run(request, output, request_path):
    try:
        import ifcopenshell
        from . import hierarchy
    except (ImportError, OSError) as exc:
        raise protocol.ProtocolError("pinned IfcOpenShell runtime unavailable: " + str(exc),
                                     "backend", "BACKEND_UNAVAILABLE") from exc
    model = hierarchy.build(request)
    path = output / "model.ifc"
    model.write(str(path))
    validation_path = output / "validation.json"

    def persist_validation(value):
        validation_path.write_text(json.dumps(value, sort_keys=True, allow_nan=False) + "\n")

    def reject_native(result, field):
        code = "BACKEND_TIMEOUT" if result.failure == "timeout" else (
            "BACKEND_UNAVAILABLE" if result.failure == "launch_error" else "ARTIFACT_INVALID")
        raise protocol.ProtocolError("bounded native child failed: " + str(result.failure), field, code)

    reopen = run_native_process([sys.executable, "-I", "-m", "archy_ifc_worker.verify",
                                 "--ifc", str(path), "--request", str(request_path)])
    reopen_evidence = asdict(reopen)
    if reopen.failure:
        persist_validation({"status": "error", "reopen_process": reopen_evidence})
        reject_native(reopen, "model.ifc")
    try:
        validation = json.loads(reopen.stdout)
    except json.JSONDecodeError as exc:
        persist_validation({"status": "error", "reopen_process": reopen_evidence})
        raise protocol.ProtocolError("fresh process reopen returned invalid JSON", "model.ifc",
                                     "ARTIFACT_INVALID") from exc
    if type(validation) is not dict:
        persist_validation({"status": "error", "reopen_process": reopen_evidence})
        raise protocol.ProtocolError("fresh process reopen returned non-object JSON", "model.ifc",
                                     "ARTIFACT_INVALID")
    if reopen.returncode != 0 or validation.get("status") != "ok":
        persist_validation({"status": "error", "reopen_process": reopen_evidence})
        raise protocol.ProtocolError("fresh process reopen failed: " +
                                     validation.get("message", "unknown artifact error"),
                                     "model.ifc", "ARTIFACT_INVALID")
    validation["fresh_process_reopen"] = True
    validation["reopen_process"] = {"returncode": reopen.returncode,
                                    "stderr": reopen.stderr, "failure": None}
    validator = run_native_process([sys.executable, "-I", "-m", "ifcopenshell.validate",
                                    str(path), "--rules", "--json"])
    stdout_lines = validator.stdout.strip().splitlines()
    allowed_success = ["No validation issues found."]
    optional_sql_notice = "No SQL support: No module named '_sqlite3'"
    clean = (validator.failure is None and validator.returncode == 0 and
             stdout_lines in (allowed_success, [optional_sql_notice, *allowed_success]) and
             validator.stderr.strip() == f"Validating {path}")
    validation["express_validation"] = {"exit_code": validator.returncode,
                                         "findings": [] if clean else stdout_lines,
                                         "stderr": validator.stderr,
                                         "failure": validator.failure}
    persist_validation(validation)
    if validator.failure:
        reject_native(validator, "model.ifc")
    if not clean:
        raise protocol.ProtocolError("EXPRESS validation found issues or failed", "model.ifc",
                                     "ARTIFACT_INVALID")
    return {"schema_version": protocol.SCHEMA_VERSION, "request_id": request["request_id"],
            "project_id": request["project_id"], "variant_id": request["variant_id"],
            "run_id": request["run_id"], "source_digest": request["source_digest"],
            "worker_version": VERSION, "backend_version": ifcopenshell.version,
            "python_version": sys.version.split()[0], "status": "ok", "warnings": [],
            "validation": validation,
            "artifacts": [artifact(path, "application/x-step"),
                          artifact(validation_path, "application/json")]}


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("--project-root", required=True)
    parser.add_argument("--request", required=True)
    args = parser.parse_args(argv)
    request = protocol.parse_request(input_bytes(args.project_root, args.request))
    output = prepare_output(args.project_root, request["output_dir"])
    try:
        response = run(request, output, args.request)
    except protocol.ProtocolError as exc:
        response = {"schema_version": protocol.SCHEMA_VERSION, "status": "error",
                    "error": exc.envelope(), "artifacts": []}
    except Exception:
        response = {"schema_version": protocol.SCHEMA_VERSION, "status": "error",
                    "error": {"code": "INTERNAL_ERROR", "message": "unexpected worker failure",
                              "field": "$", "retriable": False}, "artifacts": []}
    if response["status"] == "error":
        response.update({key: request[key] for key in
                         ("request_id", "project_id", "variant_id", "run_id", "source_digest")})
    (output / "response.json").write_text(json.dumps(response, sort_keys=True, allow_nan=False) + "\n")
    return 0 if response["status"] == "ok" else 20


if __name__ == "__main__":
    raise SystemExit(main())
