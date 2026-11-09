#!/usr/bin/env python3
"""Disposable user services: manager inheritance and pre-main preload behavior."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid


def run(command, env, *, check=False):
    result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=20)
    if check and result.returncode != 0:
        raise RuntimeError(f"{command[0]}: {result.stderr}")
    return result


def service(env, *arguments):
    unit = "archy-env-probe-" + uuid.uuid4().hex + ".service"
    command = ["/usr/bin/systemd-run", "--user", "--pipe", "--wait", "--collect", "--quiet",
               "--service-type=exec", "--expand-environment=no", "--unit=" + unit, *arguments]
    result = run(command, env)
    state = run(["/usr/bin/systemctl", "--user", "show", unit, "-pActiveState", "-pLoadState"], env, check=True).stdout
    assert "ActiveState=inactive" in state and "LoadState=not-found" in state, state
    return result


def main(gate):
    env = {"LC_ALL": "C", "PATH": "/usr/bin", "XDG_RUNTIME_DIR": f"/run/user/{os.getuid()}"}
    key = "ARCHY_MANAGER_PROBE_" + uuid.uuid4().hex.upper()
    value = "manager-" + uuid.uuid4().hex
    try:
        run(["/usr/bin/systemctl", "--user", "set-environment", f"{key}={value}"], env, check=True)
        inherited = service(env, "--", "/usr/bin/env")
        assert inherited.returncode == 0, inherited.stderr
        assert f"{key}={value}" in inherited.stdout.splitlines(), inherited.stdout
        print("manager variable inherited by disposable user service")
    finally:
        run(["/usr/bin/systemctl", "--user", "unset-environment", key], env, check=True)
    with tempfile.TemporaryDirectory(prefix="archy-preload-probe-") as tmp:
        root = Path(tmp)
        library = root / "preload.so"
        program = root / "check-main"
        marker = root / "preloaded.txt"
        source = root / "preload.c"
        source.write_text('#include <stdio.h>\n#include <stdlib.h>\n__attribute__((constructor)) static void before_main(void) { const char *p=getenv("ARCHY_PROBE_MARKER"); if (p) { FILE *f=fopen(p,"w"); if (f) { fputs("constructor\\n",f); fclose(f); } } }\n')
        run(["/usr/bin/cc", "-shared", "-fPIC", "-o", str(library), str(source)], env, check=True)
        source.write_text('#include <stdio.h>\n#include <stdlib.h>\n#include <unistd.h>\nint main(void) { const char *p=getenv("ARCHY_PROBE_MARKER"); if (p && access(p,F_OK)==0) { puts("preloaded-before-main"); return 0; } return 9; }\n')
        run(["/usr/bin/cc", "-o", str(program), str(source)], env, check=True)
        options = [f"--property=Environment=LD_PRELOAD={library}",
                   f"--property=Environment=ARCHY_PROBE_MARKER={marker}"]
        dynamic = service(env, *options, "--", str(program))
        assert dynamic.returncode == 0 and dynamic.stdout.strip() == "preloaded-before-main", (dynamic.stdout, dynamic.stderr)
        assert marker.read_text() == "constructor\n"
        print("unit-scoped LD_PRELOAD constructor ran before dynamic program main")
        marker.unlink()
        static = service(env, *options, "--", str(Path(gate).resolve()))
        assert static.returncode != 0 and not marker.exists(), (static.stdout, static.stderr)
        assert "BACKEND_UNAVAILABLE" in static.stderr, static.stderr
        print("static gate rejected invalid invocation without loading unit-scoped LD_PRELOAD")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: service_gate_environment_probe.py /path/to/static-gate")
    main(sys.argv[1])
