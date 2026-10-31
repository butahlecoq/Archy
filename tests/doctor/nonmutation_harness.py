import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT / "target/debug/archy"


def inventory(root):
    result = {}
    for path in sorted(root.rglob("*")):
        relative = str(path.relative_to(root))
        stat = path.lstat()
        if path.is_symlink():
            result[relative] = ("symlink", os.readlink(path), stat.st_mode)
        elif path.is_file():
            result[relative] = ("file", hashlib.sha256(path.read_bytes()).hexdigest(), stat.st_mode)
        else:
            result[relative] = ("directory", None, stat.st_mode)
    return result


def run(argv):
    return subprocess.run(argv, cwd=ROOT, capture_output=True, text=True, timeout=15, check=False)


def audit_traces(call_text, binary, bwrap, shim):
    traces = {int(Path(path).name.rsplit(".", 1)[1]): text for path, text in call_text.items()}
    executed = re.findall(r'execve\("([^"]+)"', "\n".join(traces.values()))
    assert all(path in {bwrap, binary, "/usr/bin/pacman"} for path in executed), executed
    assert shim not in executed
    active = {}
    pending = []
    for pid, text in traces.items():
        lines = text.splitlines()
        for index, line in enumerate(lines):
            if line.startswith(f'execve("{binary}"') and line.rstrip().endswith("= 0"):
                active[pid] = "\n".join(lines[index + 1:])
                pending.append(pid)
                break
    assert active, "doctor process was not traced after successful execve"
    visited = set(pending)
    while pending:
        pid = pending.pop()
        segment = active[pid]
        for child in re.findall(r'(?m)^(?:clone|clone3|fork|vfork)\([^\n]*\)\s+=\s+(\d+)', segment):
            child_pid = int(child)
            if child_pid in traces and child_pid not in visited:
                visited.add(child_pid)
                active[child_pid] = traces[child_pid]
                pending.append(child_pid)
    for segment in active.values():
        assert "socket(" not in segment and "connect(" not in segment, "doctor or descendant attempted network syscall"
        assert not re.search(r'(?m)^(?:open|openat|openat2)\([^\n]*(?:O_WRONLY|O_RDWR|O_CREAT|O_TRUNC)', segment), "doctor or descendant attempted writable open"
        assert not re.search(r'(?m)^(?:creat|unlink|unlinkat|rename|renameat|renameat2|mkdir|mkdirat|rmdir|symlink|symlinkat|link|linkat|chmod|fchmod|fchmodat|chown|fchown|fchownat|truncate|ftruncate)\(', segment), "doctor or descendant attempted filesystem mutation"
    return len(visited)


def main():
    if not BINARY.is_file():
        print("UNAVAILABLE: build the CLI before running the nonmutation harness")
        return 77
    bwrap = shutil.which("bwrap")
    if bwrap is None:
        print("UNAVAILABLE: bubblewrap is not installed")
        return 77
    with tempfile.TemporaryDirectory(prefix="archy-doctor-fixture-") as fixture_tmp, tempfile.TemporaryDirectory(prefix="archy-doctor-trace-") as trace_tmp:
        fixture = Path(fixture_tmp)
        home = fixture / "home"
        config = home / ".config/archy"
        project = fixture / "project"
        path_dir = fixture / "hostile path"
        for path in (config, project, path_dir):
            path.mkdir(parents=True)
        (home / "sentinel").write_bytes(b"home sentinel")
        (config / "sentinel").write_bytes(b"config sentinel")
        (project / "sentinel").write_bytes(b"project sentinel")
        shim = path_dir / "pacman"
        shim.write_text("#!/bin/sh\necho shim-ran > executed\n")
        shim.chmod(0o755)
        before = inventory(fixture)
        sandbox = [bwrap, "--unshare-net", "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev", "--chdir", str(fixture),
                   "--setenv", "HOME", str(home), "--setenv", "XDG_CONFIG_HOME", str(config), "--setenv", "PATH", str(path_dir),
                   "--setenv", "PYTHONPATH", str(path_dir), "--setenv", "MISE_DATA_DIR", str(path_dir)]
        denial = "import pathlib,socket,sys; denied_write=True; denied_net=False" \
                 "\nfor name in sys.argv[1:]:" \
                 "\n try: pathlib.Path(name).write_bytes(b'changed'); denied_write=False" \
                 "\n except OSError: pass" \
                 "\ntry: socket.create_connection(('1.1.1.1',53),timeout=0.2)\nexcept OSError: denied_net=True" \
                 "\nprint('write_denied',denied_write,'network_denied',denied_net); sys.exit(0 if denied_write and denied_net else 1)"
        preflight = run(sandbox + [sys.executable, "-I", "-c", denial, str(home / "sentinel"), str(config / "sentinel"), str(project / "sentinel")])
        if preflight.returncode != 0:
            print(f"UNAVAILABLE: sandbox denial preflight failed: {preflight.stderr.strip()} {preflight.stdout.strip()}")
            return 77
        tracer = shutil.which("strace")
        trace_prefix = Path(trace_tmp) / "calls"
        command = sandbox + [str(BINARY), "doctor", "--json"]
        if tracer:
            command = [tracer, "-ff", "-o", str(trace_prefix), "-e", "trace=process,network,file"] + command
        output = run(command)
        assert output.returncode in (0, 1), (output.returncode, output.stderr)
        assert not output.stderr, output.stderr
        report = json.loads(output.stdout)
        assert report["target_acceptance"] == "deferred"
        assert before == inventory(fixture), "home, config, project or hostile PATH fixture changed"
        assert not (fixture / "executed").exists()
        if tracer is None:
            print("UNAVAILABLE: sandbox denial and inventories passed; process-call recording unavailable because strace is not installed")
            return 77
        traces = list(Path(trace_tmp).glob("calls.*"))
        assert traces, "strace produced no process-call records"
        call_text = {path: path.read_text(errors="replace") for path in traces}
        doctor_processes = audit_traces(call_text, str(BINARY), bwrap, str(shim))
        print(f"PASS: sandbox denied writes/network; home/config/project inventories unchanged; {doctor_processes} doctor process/thread traces audited across {len(traces)} total traces")
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
