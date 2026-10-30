#!/usr/bin/env python3
"""Measure installed audit and worker work under a disposable container's limits."""
import json
import os
from pathlib import Path
import pwd
import resource
import shutil
import subprocess
import sys
import time

if os.geteuid() != 0 or not Path('/.dockerenv').exists():
    raise SystemExit('requires disposable root container')

runtime = Path('/usr/lib/archy/runtime')
gate = Path('/usr/lib/archy/archy-service-gate')
shutil.copyfile(sys.argv[1], gate)
gate.chmod(0o755)


def cgroup(name):
    return (Path('/sys/fs/cgroup') / name).read_text().strip()


def record(**values):
    print(json.dumps(values, sort_keys=True), flush=True)


def measure(name, command):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    stat_before = cgroup('cpu.stat')
    start = time.monotonic()
    try:
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                timeout=300, env={**os.environ, 'LC_ALL': 'C'})
    except subprocess.TimeoutExpired:
        record(stage=name, error='timeout', limit_seconds=300,
               wall_seconds=round(time.monotonic() - start, 3))
        raise
    elapsed = time.monotonic() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    record(stage=name, wall_seconds=round(elapsed, 3), exit_code=result.returncode,
           user_cpu_seconds=round(after.ru_utime - before.ru_utime, 3),
           system_cpu_seconds=round(after.ru_stime - before.ru_stime, 3),
           cpu_stat_before=stat_before, cpu_stat_after=cgroup('cpu.stat'),
           stdout_tail=result.stdout[-500:].decode(errors='replace'),
           stderr_tail=result.stderr[-500:].decode(errors='replace'))
    if result.returncode:
        raise SystemExit(f'{name} failed: {result.returncode}')
    return elapsed


record(stage='environment', cpu_max=cgroup('cpu.max'), memory_max=cgroup('memory.max'),
       memory_swap_max=cgroup('memory.swap.max'), pids_max=cgroup('pids.max'),
       load_average=os.getloadavg(), budget_seconds=180,
       limitations='Direct worker after audit; no systemd, bwrap, publication or Omarchy proof. Cache state uncontrolled.')

project = Path('/tmp/archy-budget-ifc')
(project / 'runs/run-1').mkdir(parents=True)
request = project / 'request.json'
request.write_bytes(Path('/ifc-request.json').read_bytes())
nobody = pwd.getpwnam('nobody')
for path in (project, project / 'runs', project / 'runs/run-1', request):
    os.chown(path, nobody.pw_uid, nobody.pw_gid)

audit_ifc = measure('ifc_audit', [str(gate), '--audit-runtime=ifc'])
worker_ifc = measure('ifc_worker', ['/usr/sbin/runuser', '-u', 'nobody', '--',
                     str(runtime / 'python/bin/python3.12'), '-B', '-I', '-m',
                     'archy_ifc_worker', '--project-root', str(project), '--request', str(request)])
response = json.loads((project / 'runs/run-1/ifc/response.json').read_text())
assert response['status'] == 'ok' and response['validation']['fresh_process_reopen'] is True
assert response['validation']['express_validation']['exit_code'] == 0
record(stage='ifc_sum', seconds=round(audit_ifc + worker_ifc, 3), within_180=audit_ifc + worker_ifc < 180)

audit_blender = measure('blender_audit', [str(gate), '--audit-runtime=blender'])
worker_blender = measure('blender_worker', ['/usr/sbin/runuser', '-u', 'nobody', '--',
                         'python3', '-B', '/blender-probe.py'])
record(stage='blender_sum', seconds=round(audit_blender + worker_blender, 3),
       within_180=audit_blender + worker_blender < 180)
