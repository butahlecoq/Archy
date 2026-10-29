# A15 MCP first read slice review, round 1

The goal is a fixed-root MCP reader that gives agents verified project data and explicit unavailable results for unfinished tools. **Verdict: ISSUES.** The live stdio checks pass, but two read-contract gaps survived adversarial probes against the frozen worktree.

## P2. Explicit revision zero returns the current source

`get_project` treats `revision: "0"` as a historical selector but fills it from `snapshot.spec()` and `snapshot.source_sha256()` in `crates/archy-mcp/src/server.rs`. Those values come from the manifest at server start. They need not be the source that existed when revision zero was current.

I ran this probe in the A15 worktree. It edits only a disposable project copy:

```sh
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-mcp-review CARGO_BUILD_JOBS=2 python3 - <<'PY'
import tempfile, pathlib, shutil, subprocess, json, hashlib
root = pathlib.Path.cwd()
bin = pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a15-mcp-review/debug')
with tempfile.TemporaryDirectory(prefix='archy-a15-rev-probe-') as d:
    project = pathlib.Path(d) / 'project.yaml'
    shutil.copyfile(root / 'examples/courtyard-housing/project.yaml', project)
    old = hashlib.sha256(project.read_bytes()).hexdigest()
    subprocess.run([bin/'archy', 'generate', '--project', project, '--count', '1', '--seed', '42', '--json'], check=True, stdout=subprocess.DEVNULL)
    project.write_bytes(project.read_bytes().replace(b'  width_m: 20', b'  width_m: 21'))
    new = hashlib.sha256(project.read_bytes()).hexdigest()
    server = subprocess.Popen([bin/'archy-mcp', '--project', project], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    for i, revision in enumerate(['0', '1', None], 1):
        args = {'project_id': 'courtyard-demo'}
        if revision is not None: args['revision'] = revision
        req = {'jsonrpc':'2.0','id':i,'method':'tools/call','params':{'name':'get_project','arguments':args,'_meta':{'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientInfo':{'name':'probe','version':'0'},'io.modelcontextprotocol/clientCapabilities':{}}}}
        server.stdin.write(json.dumps(req).encode() + b'\n'); server.stdin.flush()
        data = json.loads(server.stdout.readline())['result']['structuredContent']['data']
        print('requested', revision, 'returned', data['revision'], 'digest', data['source_sha256'], 'old?', data['source_sha256']==old, 'new?', data['source_sha256']==new)
    server.stdin.close(); server.wait(timeout=5)
PY
```

Observed output:

```text
requested 0 returned 0 digest bb5a46027883c7632a714b979d2c9c6cc3b61794950f366e5dbd6f78052808d6 old? False new? True
requested 1 returned 1 digest dbd0c3debe37cd7c725903d3492c991ae3451bf0ae8832be41c97b23f1a2b358 old? True new? False
requested None returned 1 digest bb5a46027883c7632a714b979d2c9c6cc3b61794950f366e5dbd6f78052808d6 old? False new? True
```

The explicit revision-zero response therefore identifies changed source bytes as revision zero. A historical selector should return the original source or refuse when no genesis snapshot exists. The default current-source read can continue to use the pinned manifest and its separate source digest. Add a source-edit regression for explicit `"0"` so a future client cannot mistake current bytes for historical provenance.

## P2. The advertised output schema admits impossible envelopes

The nine output schemas derive from `Envelope<T>` with optional `data` and `error` fields. They require all three keys but do not bind `ok` to the populated branch. This command against the frozen `get_project` schema returned `True` for both invalid states:

```sh
python3 - <<'PY'
import json
from jsonschema import Draft202012Validator
s = json.load(open('schemas/mcp/get_project.schema.json'))['output']
for value in ({'ok': True, 'data': None, 'error': None},
              {'ok': False, 'data': None, 'error': None}):
    print(Draft202012Validator(s).is_valid(value))
PY
```

```text
True
True
```

Expected output is `False` for each. The server currently constructs valid pairs, so this is an advertised-contract defect rather than a reproduced bad server response. Use distinct success and error schema branches with fixed `ok` values, a nonnull `data` in success, and a nonnull `error` in failure. Regenerate all nine checked-in pairs and test both invalid envelopes.

## Verified scope and limits

`sha256sum -c docs/plans/mcp-first-slice-freeze.sha256` passed for all 27 source and schema files in `/home/butahlecoq/work/archy/.worktrees/a15-mcp`, based on `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`. I reviewed `/tmp/archy-a15-worktree-review.patch` and the live source. I built `archy-mcp` and `archy-cli` with `RUSTUP_AUTO_INSTALL=0`, offline Cargo, two jobs, and `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-mcp-review`. The actual stdio contract script passed modern and legacy protocol calls, nine descriptors, three reads, six unavailable tools, paging, stale cursors, corrupt history, source replacement, and no-write checks. `cargo test --locked -p archy-mcp --test bounds` passed four transport tests. The author's 145 workspace tests, format, Clippy, and schema gates are handoff evidence, not my reruns.

I found no reproduced root escape or mutation path in this review. I did not exercise concurrent in-place source edits after a snapshot, external MCP clients, or Omarchy runtime behavior. Those are unverified risks, not findings.

The installed pstack guidance shaped this review. **Boundary Discipline** kept the findings at the MCP selector and schema boundary. **Prove It Works** led to direct stdio and schema probes rather than relying on the handoff's passing checks. I used the interrogate approach without subagents because this assignment forbids child agents.
