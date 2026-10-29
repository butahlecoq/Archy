# MCP backpressure repair: independent round-two review

29 October 2026. Worktree `/home/butahlecoq/work/archy/.worktrees/mcp-backpressure-repair-20261029`; HEAD `8e36e11d1399dee37dc2b85a8e2225b473571602`. **Verdict: changes requested for the original backpressure requirement.** The live test is a useful producer stall and recovery check, and its 80 reply checks passed. Its oracle can also pass when both plateaus are deliberately caused by stopping the real child before the pipe is full. Therefore the test does not establish that a child output write was blocked. This is an oracle flaw, not evidence that the MCP transport loses replies or that ordinary CI runs were scheduler starved.

## Scope and source integrity

I read `AGENTS.md`, the round-two handoff, round-one review, freeze file, the complete `parse_errors.py` and its HEAD diff, and `main.rs`/`framing.rs`. Commands, from this worktree:

```sh
git rev-parse HEAD
sha256sum -c docs/plans/mcp-backpressure-repair-freeze.sha256
git diff -- crates/archy-mcp/tests/parse_errors.py
git diff --check
git show ff7f6bc --stat --oneline
```

All eight frozen files matched their recorded SHA-256 values. The only tracked source diff is `crates/archy-mcp/tests/parse_errors.py`; `git diff --check` exited 0. Integration commit `ff7f6bc` changes the separate `crates/archy-adapters/src/process.rs` fixture and its documents; I did not modify it. The freeze covers the pre-review handoff and round-one review. This new review is intentionally outside that earlier freeze.

## Decisive two-window counterexample

This deterministic fault injection executes the current test **in memory** with the actual MCP binary and actual child output. It inserts `SIGSTOP` before each existing stall wait, once unread output appears, and `SIGCONT` only after the test has drained that output. The existing `wait_for_stalled_output`, resumed-output assertion, frame reader, and all 80 reply assertions remain in place. No repository source or reply bytes are edited.

```sh
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B - <<'PY'
import pathlib
path = pathlib.Path('crates/archy-mcp/tests/parse_errors.py')
s = path.read_text().replace('import select\n', 'import select\nimport signal\n', 1)
s = s.replace('stalled = wait_for_stalled_output(read_fd, process, deadline)', '''while unread(read_fd) == 0:
                time.sleep(0.001)
            before_stop = unread(read_fd)
            os.kill(process.pid, signal.SIGSTOP)
            print('INJECT_STOP', cycle, before_stop, 'capacity', capacity, flush=True)
            stalled = wait_for_stalled_output(read_fd, process, deadline)''', 1)
s = s.replace('reader.pending.extend(os.read(read_fd, stalled))',
              'reader.pending.extend(os.read(read_fd, stalled))\n            os.kill(process.pid, signal.SIGCONT)', 1)
ns = {'__name__': 'probe', '__file__': str(path.resolve())}
try:
    exec(compile(s, str(path), 'exec'), ns)
finally:
    for p in ns.get('PROCESSES', []):
        if p.poll() is None:
            p.kill()
            p.wait()
PY
```

Observed exit 0:

```text
INJECT_STOP 0 76 capacity 4096
INJECT_STOP 1 836 capacity 4096
MCP stdout stall/resume (unread bytes): [(76, 836), (836, 304)]
MCP stdio parse errors, fragmentation, bounds, concurrency, legacy, EOF and nonmutation: PASS
```

The child's inability to run during each 250 ms window fully explains both flat unread counts. Its resumed writes are real but are caused by `SIGCONT` following the drain. The small occupancies do not themselves prove that no large write was already blocked; the experiment proves the assertion accepts an alternative causal explanation. An unmodified run instead reported `[(1216, 4096), (4096, 4096)]`. The full second pipe and repeated recovery are strong behavioral evidence of practical backpressure, but a finite plateau and subsequent output do not identify a blocked syscall. The test cannot make the original backpressure claim as written.

## Previous oracles reproduced

I reproduced the old byte threshold against the real binary with 64 `tools/call` requests, a 65,536-byte stdout pipe, no stdout drain during a 250 ms occupancy plateau, then full drain. Exact command:

```sh
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B - <<'PY'
import os, fcntl, termios, array, select, time, threading, subprocess, json, tempfile, pathlib, shutil
root = pathlib.Path.cwd()
binary = pathlib.Path(os.environ['CARGO_TARGET_DIR'])/'debug/archy-mcp'
def unread(fd):
    a = array.array('i', [0]); fcntl.ioctl(fd, termios.FIONREAD, a, True); return a[0]
with tempfile.TemporaryDirectory() as td:
    project = pathlib.Path(td)/'project.yaml'
    shutil.copyfile(root/'examples/courtyard-housing/project.yaml', project)
    r,w = os.pipe()
    p = subprocess.Popen([str(binary),'--project',str(project)], stdin=subprocess.PIPE,
                         stdout=w, stderr=subprocess.PIPE, bufsize=0)
    os.close(w)
    try:
        meta = {'io.modelcontextprotocol/protocolVersion':'2026-07-28',
                'io.modelcontextprotocol/clientInfo':{'name':'probe','version':'0'},
                'io.modelcontextprotocol/clientCapabilities':{}}
        payload = b''.join(json.dumps({'jsonrpc':'2.0','id':i,'method':'tools/call',
            'params':{'name':'get_project','arguments':{'project_id':'courtyard-demo'},
                      '_meta':meta}}, separators=(',',':')).encode()+b'\n' for i in range(64))
        sender = threading.Thread(target=lambda:(p.stdin.write(payload),p.stdin.flush()), daemon=True)
        sender.start()
        cap = fcntl.fcntl(r, fcntl.F_GETPIPE_SZ)
        threshold = cap-os.fpathconf(r,'PC_PIPE_BUF')
        last = -1; since = time.monotonic(); deadline = since+10
        while time.monotonic() < deadline:
            n = unread(r); now = time.monotonic()
            if n != last: last = n; since = now
            if n and now-since >= .25: break
            time.sleep(.01)
        print('capacity',cap,'old_threshold',threshold,'plateau',last,
              'old_assertion',last>=threshold,'sender_alive',sender.is_alive())
        out = bytearray(); p.stdin.close()
        while len(out.splitlines()) < 64:
            assert select.select([r],[],[],10)[0]
            chunk = os.read(r,65536); assert chunk; out.extend(chunk)
        replies = [json.loads(s) for s in out.splitlines()]
        p.wait(timeout=10)
        print('replies',len(replies),'unique_ids',len({x['id'] for x in replies}),
              'exit',p.returncode,'stderr',p.stderr.read()[-200:])
    finally:
        if p.poll() is None: p.kill(); p.wait()
        os.close(r)
PY
```

Output: `capacity 65536 old_threshold 61440 plateau 33046 old_assertion False sender_alive False`; after drain, `replies 64 unique_ids 64 exit 0 stderr b''`. Thus the old condition rejects live, complete output.

The false `!POLLOUT` oracle reproduces with a kernel pipe alone:

```sh
python3 -B - <<'PY'
import os, fcntl, select
r,w = os.pipe()
print('capacity', fcntl.fcntl(r, fcntl.F_SETPIPE_SZ, 4096))
os.write(w, b'x'*1024)
p = select.poll(); p.register(w, select.POLLOUT)
print('POLLOUT', p.poll(0), 'small_write', os.write(w, b'y'))
os.close(r); os.close(w)
PY
```

Output: `capacity 4096`, `POLLOUT [] small_write 1`. An empty `POLLOUT` poll is not proof that the next small producer write cannot succeed.

## Behavioral coverage and bounds

```sh
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B crates/archy-mcp/tests/parse_errors.py
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B scripts/check-schemas.py
```

Both exited 0. The first reported `[(1216, 4096), (4096, 4096)]` and PASS. Concurrent command:

```sh
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/mcp-backpressure-repair-20261029 python3 -B - <<'PY'
import concurrent.futures, os, subprocess
cmd = ['python3','-B','crates/archy-mcp/tests/parse_errors.py']
def run(i):
    p = subprocess.run(cmd, capture_output=True, text=True, env=os.environ, timeout=60)
    lines = [s for s in p.stdout.splitlines() if 'stdout stall/resume' in s or 'PASS' in s]
    return i,p.returncode,lines,p.stderr
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
    results = list(pool.map(run, range(24)))
assert all(code == 0 and len(lines) == 2 and 'PASS' in lines[-1] and not err
           for _,code,lines,err in results), results
print('TOTAL 24/24 PASS')
PY
```

Observed 24/24 exited 0, with empty stderr and the same two occupancy pairs in every run. The schema command exited 0, including the live MCP stdio suites, nine SDK schema pairs, 50 decision cases, and six demo tests.

The batch parses 80 complete JSON replies, requires each ID 1000–1063 exactly once, 16 exact parse errors, no marker echo, and sender completion. Earlier sequential cases assert malformed-frame recovery order, legacy behavior, frame/depth bounds, EOF, and project nonmutation. The concurrent batch checks membership and uniqueness, **not response order**; no concurrent ordering guarantee is inferred. The 10-second stall/resume deadline and aggregate 20-second drain deadline are bounded, with separate 10-second join/wait limits.

For failure cleanup, I executed the source in memory with only the batch call to `wait_for_stalled_output` replaced by `raise AssertionError('FORCED_AFTER_SENDER_START')`, then separately by `time.sleep(11); stalled = wait_for_stalled_output(read_fd, process, deadline)`. Results: forced assertion after 0.48 s, four children reaped and sender thread dead after 100 ms; forced delay failed with `(0, 'stdout producer did not stall')` after 11.45 s, again four children reaped and sender dead. The 11-second injection intentionally exceeds the 10-second deadline. These probes exercise the batch failure path; they do not prove every possible I/O failure path.

## Next fixer: practical test-only oracles

1. **Preferred:** instrument the actual stdout write boundary in a test-only transport adapter and send an event on a separate control channel when a nonblocking write of queued reply bytes returns `EAGAIN`. Keep the real child, same requests, and real stdout reply bytes. Require the event before each controlled drain, then require a later successful write and all 80 replies. The adapter must report a kernel `EAGAIN` from stdout, rather than a generic async `Pending` that could mean lock or scheduler delay. Verify that the adapter follows the production framing path.
2. Linux tracing of the actual child stdout `write`/`writev` return value can supply the same evidence if an available, authorized tracer captures `EAGAIN` without changing transport behavior. `strace` and `bpftrace` were unavailable here, so this option is untested and may add CI dependencies or permissions.
3. A test-only hook immediately after a real stdout `EAGAIN` can signal the parent through a dedicated fd, avoiding timing inference. The parent must still keep stdout unread until it receives the signal and must validate every original reply after release. Hooking `FrameWriter`'s mutex wait or a generic `Poll::Pending` would not establish kernel backpressure.

Whichever option is chosen, replay the two-`SIGSTOP` injection: it must fail the backpressure oracle while the ordinary real-child case passes. Preserve the full 80-reply accounting and cleanup. No source edits, commits, pushes, installs, other worktree edits, or child agents were made during this review.
