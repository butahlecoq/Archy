# A15 MCP first read slice review, round 3

**Verdict: ISSUES.** The round-two mismatch is fixed for the valid Unicode values exercised across all affected fields. The published schemas still accept lone surrogate code points that the wire parser discards, leaving the client without a response. The verdict is limited to this frozen first read slice.

## P3. Unicode text schemas accept lone surrogates that the server discards

The last branch of the Unicode string schema in `crates/archy-mcp/src/tools.rs` limits length but does not exclude unpaired surrogate code points. The Python JSON Schema validator therefore accepts `"\ud800"` or `"\udfff"` as an objective. The actual JSON-RPC parser rejects the corresponding escaped JSON text without sending a response for that request; a following valid request succeeds. The ASCII identifier schemas reject these values. This is a schema/wire contract mismatch for malformed Unicode, not evidence that the server executes the malformed request.

From the worktree, after the build listed below:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 - <<'PY'
import pathlib, tempfile, shutil, subprocess, json
from jsonschema import Draft202012Validator
root=pathlib.Path.cwd()
binary=pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a15-parallel-review/debug/archy-mcp')
schema=json.loads((root/'schemas/mcp/compare_variants.schema.json').read_text())['input']
validator=Draft202012Validator(schema)
meta={'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientInfo':{'name':'probe','version':'0'},'io.modelcontextprotocol/clientCapabilities':{}}
def request(i,value):
    return {'jsonrpc':'2.0','id':i,'method':'tools/call','params':{'name':'compare_variants','arguments':{'project_id':'courtyard-demo','run_ids':['r'],'objective':value},'_meta':meta}}
with tempfile.TemporaryDirectory() as d:
    project=pathlib.Path(d)/'project.yaml'
    shutil.copyfile(root/'examples/courtyard-housing/project.yaml',project)
    for label,value in [('high','\ud800'),('low','\udfff'),('valid-pair','😀')]:
        bad=request(1,value)
        wire=(json.dumps(bad)+'\n'+json.dumps(request(2,'area'))+'\n').encode()
        result=subprocess.run([binary,'--project',project],input=wire,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=5)
        replies=[json.loads(line) for line in result.stdout.splitlines()]
        print(label,'schema',validator.is_valid(bad['params']['arguments']),'response_ids',[r.get('id') for r in replies],'exit',result.returncode)
PY
```

Observed:

```text
high schema True response_ids [2] exit 0
low schema True response_ids [2] exit 0
valid-pair schema True response_ids [1, 2] exit 0
```

Align the advertised Unicode constraints with scalar values and cover both isolated surrogate halves and valid supplementary characters. A separate malformed raw UTF-8 probe likewise produced no response for the malformed request and did respond to the following valid request.

## String bounds and documented policy

For `compare_variants.run_ids`, 64, 65, and 128 copies of `é` were schema-invalid and returned `-32602`; 128 ASCII `x` characters were schema-valid and reached `BACKEND_UNAVAILABLE`, and 129 were invalid in both places. For `objective`, 64 copies of `é` were accepted by both, while 65 and 128 were rejected; 128/129 ASCII characters were accepted/rejected respectively. These checks used actual `tools/call` responses.

The following independent matrix checks every occurrence of each affected field in all nine input schemas and calls the actual server. It includes the 64/65/128 `é` cases, ASCII 128/129, each field's own ASCII limit, two-, three-, and four-byte tier boundaries, and mixed text. It printed `cases 476 schema/wire mismatches 0 schema-valid byte bound violations 0` for valid scalar inputs.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 - <<'PY'
import json,pathlib,subprocess,tempfile,shutil,select
from jsonschema import Draft202012Validator
root=pathlib.Path.cwd();binary=pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a15-parallel-review/debug/archy-mcp')
base={'project_id':'courtyard-demo','source_run':'r','variant_id':'v','idempotency_key':'k','expected_project_revision':'0','destination':'runs','count':1,'seed':'1','preset':'p','run_ids':['r'],'objective':'area','reason':'review','cursor':'0'}
fields={'get_project':['project_id'],'validate_project':['project_id'],'list_variants':['project_id','source_run','cursor'],'generate_variants':['project_id','idempotency_key','destination'],'evaluate_variant':['project_id','source_run','variant_id','idempotency_key','destination'],'render_variant':['project_id','source_run','variant_id','preset','idempotency_key','destination'],'compare_variants':['project_id','run_ids','objective','cursor'],'export_ifc':['project_id','source_run','variant_id','idempotency_key','destination'],'propose_approval':['project_id','source_run','variant_id','reason','idempotency_key','destination']}
needed={'get_project':['project_id'],'validate_project':['project_id'],'list_variants':['project_id'],'generate_variants':['project_id','count','seed','idempotency_key','expected_project_revision','destination'],'evaluate_variant':['project_id','source_run','variant_id','idempotency_key','expected_project_revision','destination'],'render_variant':['project_id','source_run','variant_id','preset','idempotency_key','expected_project_revision','destination'],'compare_variants':['project_id','run_ids','objective'],'export_ifc':['project_id','source_run','variant_id','idempotency_key','expected_project_revision','destination'],'propose_approval':['project_id','source_run','variant_id','reason','idempotency_key','expected_project_revision','destination']}
meta={'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientInfo':{'name':'matrix','version':'0'},'io.modelcontextprotocol/clientCapabilities':{}}
free={'preset':128,'objective':128,'destination':512,'reason':4096};count=0;mismatches=[];oversize=[]
with tempfile.TemporaryDirectory() as d:
    project=pathlib.Path(d)/'project.yaml';shutil.copyfile(root/'examples/courtyard-housing/project.yaml',project)
    server=subprocess.Popen([binary,'--project',project],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    for tool,fs in fields.items():
        schema=json.loads((root/'schemas/mcp'/f'{tool}.schema.json').read_text())['input'];validator=Draft202012Validator(schema)
        for field in fs:
            bound=free.get(field,256 if field=='cursor' else 128)
            values=['x'*128,'x'*129,'é'*64,'é'*65,'é'*128,'x'*bound,'x'*(bound+1),'é'*(bound//2),'é'*(bound//2+1),'中'*(bound//3),'中'*(bound//3+1),'😀'*(bound//4),'😀'*(bound//4+1),'x'*(bound//4)+'😀']
            for value in values:
                args={k:base[k] for k in needed[tool]};args[field]=[value] if field=='run_ids' else value
                valid=validator.is_valid(args);req={'jsonrpc':'2.0','id':count+1,'method':'tools/call','params':{'name':tool,'arguments':args,'_meta':meta}}
                server.stdin.write(json.dumps(req).encode()+b'\n');server.stdin.flush()
                assert select.select([server.stdout],[],[],10)[0],(tool,field,count)
                reply=json.loads(server.stdout.readline());accepted=reply.get('error',{}).get('code')!=-32602
                if valid!=accepted:mismatches.append((tool,field,len(value),valid,reply.get('error')))
                if valid and len(value.encode())>bound:oversize.append((tool,field,len(value.encode()),bound))
                count+=1
        print(tool,'fields',','.join(fs),'checked')
    server.stdin.close();server.wait(timeout=5);assert server.returncode==0
print('cases',count,'schema/wire mismatches',len(mismatches),'schema-valid byte bound violations',len(oversize))
print('mismatches',mismatches[:5]);print('oversize',oversize[:5]);assert not mismatches and not oversize
PY
```

For valid Unicode scalars, the schema branches bound one-byte text to `N` characters, text of width at most two to `floor(N/2)`, text of width at most three to `floor(N/3)`, and all other text to `floor(N/4)`. Thus no branch can admit more than the prior `N` UTF-8 bytes; the runtime also checks the actual bytes. The matrix sampled each boundary, rather than exhaustively enumerating all strings.

The ASCII policy is consistent with the core project-ID and idempotency-key validators and generated UUID run IDs. The documented Unicode policy deliberately excludes some strings below the former byte ceiling: for example, an objective consisting of 32 ASCII characters and one four-byte character is 36 bytes but is rejected because its 33 characters exceed the four-byte tier of 32. `docs/mcp/README.md` explicitly describes that tradeoff; the tests confirm schema and wire agree for the exercised mixed cases. I found no additional silent narrowing beyond that documented rule in the inspected bounds. The user-facing suitability of conservative rejection for future mutation tools remains a product decision.

## Regression and source identity

Worktree `/home/butahlecoq/work/archy/.worktrees/a15-mcp`, branch `codex/a15-mcp`, base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`, with uncommitted author changes. Before and after the probes, all 27 entries passed `sha256sum -c docs/plans/mcp-first-slice-freeze.sha256`, and `git apply --reverse --check /tmp/archy-a15-worktree-review.patch` succeeded for the 29-file patch. Manifest SHA-256: `605920c695d09834cb7050e25404f29108345abeb93a2422a829b767322a3e99`. Patch SHA-256: `5859a65ab20cc446a3c8b39e8448e10aac5b78b12e9fc20244809c134d852686`. Both hashes matched before and after. The status was unchanged before this report was added.

All following commands ran in the worktree with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-parallel-review PYTHONDONTWRITEBYTECODE=1`:

- `cargo build --locked -p archy-mcp -p archy-cli`: passed.
- `cargo test --locked -p archy-mcp --test bounds`: four passed.
- `cargo test --locked -p archy-cli --test desktop_readers`: seven passed, including fixed root/source replacement and noncreating reads.
- `python3 crates/archy-mcp/tests/stdio_contract.py`: passed the live bounded-input matrix, modern and legacy negotiation, nine descriptors, read-only inventories, source/root checks, cursor checks, size recovery, and revision-zero/old-revision/current-source checks.
- `python3 scripts/check-schemas.py`: passed schema and fixture checks. Separately validating both impossible null envelopes against all nine checked-in output schemas returned `False` for every case.

The output-size conclusion is based on the guard tests and source inspection; I did not generate a worst-case oversized live response. Concurrent in-place source edits during a read and external MCP clients remain unverified. No full A15 or target runtime approval is implied. The pstack interrogate approach was applied without child agents; no product code, other worktree, commit, push, or host installation was changed.
