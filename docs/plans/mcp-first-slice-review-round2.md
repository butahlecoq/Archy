# A15 MCP first read slice review, round 2

**Verdict: ISSUES.** The two round-one P2 defects and the previously reported ASCII run-ID boundary reproduce as fixed. One P3 advertised input contract mismatch remains for multibyte strings. This verdict covers the frozen MCP first read slice identified below; it is not a verdict on full A15 or target execution.

## P3. Input schemas count characters while the runtime limits UTF-8 bytes

`crates/archy-mcp/src/tools.rs:211` advertises each `compare_variants.run_ids` item with JSON Schema `maxLength: 128`, which counts Unicode characters. `crates/archy-mcp/src/server.rs:368` uses Rust string `len()`, which counts UTF-8 bytes. Thus a schema-valid argument containing 65 `é` characters is rejected with JSON-RPC `-32602`, while 64 reach the unavailable handler. The same character-versus-byte distinction exists in other fields where the schema uses `maxLength` and the runtime uses `len()`; the run-ID case below is the one exercised here.

Run from the frozen worktree after building the binaries:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 - <<'PY'
import json,pathlib,shutil,subprocess,tempfile
from jsonschema import Draft202012Validator
root=pathlib.Path.cwd()
binary=pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a15-parallel-review/debug/archy-mcp')
schema=json.loads((root/'schemas/mcp/compare_variants.schema.json').read_text())['input']
meta={'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientInfo':{'name':'probe','version':'0'},'io.modelcontextprotocol/clientCapabilities':{}}
with tempfile.TemporaryDirectory() as d:
    project=pathlib.Path(d)/'project.yaml'
    shutil.copyfile(root/'examples/courtyard-housing/project.yaml',project)
    server=subprocess.Popen([binary,'--project',project],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
    for i,value in enumerate(['é'*64,'é'*65,'é'*128],1):
        args={'project_id':'courtyard-demo','run_ids':[value],'objective':'area'}
        req={'jsonrpc':'2.0','id':i,'method':'tools/call','params':{'name':'compare_variants','arguments':args,'_meta':meta}}
        server.stdin.write(json.dumps(req).encode()+b'\n');server.stdin.flush()
        reply=json.loads(server.stdout.readline())
        status=reply['error']['code'] if 'error' in reply else reply['result']['structuredContent']['error']['code']
        print('characters',len(value),'UTF-8 bytes',len(value.encode()),'schema',Draft202012Validator(schema).is_valid(args),'runtime',status)
    server.stdin.close();server.wait(timeout=5)
PY
```

Observed twice, including a separate probe of the 128-character case:

```text
characters 64 UTF-8 bytes 128 schema True runtime BACKEND_UNAVAILABLE
characters 65 UTF-8 bytes 130 schema True runtime -32602
characters 128 UTF-8 bytes 256 schema True runtime -32602
```

A client can therefore send an argument the published schema accepts and receive an invalid-parameters response before the intended prerequisite result. Align the published constraint and runtime rule, and cover multibyte values at the chosen boundary. Preserve the independent 1 MiB frame bound.

## Round-one fixes rechecked

- With one accepted run and the project source subsequently changed before server startup, `get_project` revision `0` returned `REVISION_CONFLICT`. Revision `1` returned the old source digest; omitted revision returned revision `1` with the changed current source digest. I independently drove the actual stdio server and compared SHA-256 values. The checked-in `crates/archy-mcp/tests/stdio_contract.py` also exercises this case and genesis revision zero.
- `Draft202012Validator` rejected both `{ok:true,data:null,error:null}` and `{ok:false,data:null,error:null}` for each of the nine checked-in output schemas. The live stdio suite validates actual success and error responses against descriptors, and the schema checker compared all descriptors with the checked-in schemas.
- For ASCII `run_ids`, 128 characters were schema-valid and reached `BACKEND_UNAVAILABLE`; 129 were schema-invalid and the wire returned `-32602`. The multibyte case above leaves the general length contract unresolved.

## Source identity and verification

Reviewed `/home/butahlecoq/work/archy/.worktrees/a15-mcp`, branch `codex/a15-mcp`, base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`, with the author's uncommitted changes. User-provided turn context identifies the original author model as `gpt-6-sol medium`; I did not independently establish model provenance.

Before and after the probes, all 27 entries in `docs/plans/mcp-first-slice-freeze.sha256` passed `sha256sum -c`, and `git apply --reverse --check /tmp/archy-a15-worktree-review.patch` succeeded. The 29-file patch SHA-256 was `e44b25818e71571a2193da1f6d2687ec071255e9f3fd6af4a27ae9c86a3800ca` both times; the manifest SHA-256 was `f52b6f7535cd2e8dce01733c863b8e5e1948f49dc756875c1ec60e3e597de6fb` both times. The worktree status was unchanged before this report was added. I inspected the updated source and compared the server, tools, pinned reader, and bounds with the previous captured snapshot.

All commands below ran from the frozen worktree with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-parallel-review PYTHONDONTWRITEBYTECODE=1`:

- `cargo build --locked -p archy-mcp -p archy-cli`: passed.
- `cargo test --locked -p archy-mcp --test bounds`: four passed, including oversized output rejection and depth guard checks.
- `cargo test --locked -p archy-cli --test desktop_readers`: seven passed, including pinned source/root replacement, symlink refusal, corrupt history, and no-repair behavior.
- `python3 crates/archy-mcp/tests/stdio_contract.py`: passed modern and legacy protocol checks, unsupported initialization, descriptors and live results, no-write inventories, oversized input recovery, paging and stale cursors, corrupt/symlink history, source replacement, and historical source semantics.
- `python3 scripts/check-schemas.py`: passed the nine MCP schema pairs and existing schema, fixture, and CLI checks. Its doctor check reported target acceptance deferred; that is not target evidence.

The fixed-root reader, cursor binding, supported versions, response packing and transport guards were unchanged from the earlier snapshot. The checks above found no weakening in the exercised paths. The output-size claim is supported by source inspection and guard tests; I did not generate a worst-case oversized live response. Concurrent in-place edits during an individual read, third-party MCP clients, and target runtime behavior remain unverified. No full workspace test suite was rerun for this scoped review.

The pstack interrogate method was applied without child agents, with Boundary Discipline and Prove It Works guiding the wire and schema probes. No source fix, commit, push, install, or other worktree edit was made.
