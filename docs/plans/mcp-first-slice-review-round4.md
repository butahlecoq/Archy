# A15 MCP first read slice review, round 4

**Verdict: ISSUES.** The round-three scalar-schema fix passes the checks below. The pinned stdio transport still drops malformed JSON without the JSON-RPC parse-error response. The existing stdio test explicitly expects that drop. The slice should not be treated as complete against the requested wire-protocol requirement.

## P2. Malformed input is silently dropped instead of producing a parse error

After a successful request, malformed JSON, a lone-surrogate escape, and invalid UTF-8 each produced no response during a 0.4-second observation; the same server then answered a subsequent valid request. A client waiting for the malformed request can remain pending until it times out or disconnects. That consequence is an inference from the missing response and the SDK's continuing receive loop; I did not measure an indefinite wait.

Reproduce from the worktree after the build below:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 - <<'PY'
import json,pathlib,select,shutil,subprocess,tempfile
root=pathlib.Path.cwd()
binary=pathlib.Path('/home/butahlecoq/work/archy/.tools/targets/a15-parallel-review/debug/archy-mcp')
meta={'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientInfo':{'name':'wire-probe','version':'0'},'io.modelcontextprotocol/clientCapabilities':{}}
def frame(i,objective='area'):
    return json.dumps({'jsonrpc':'2.0','id':i,'method':'tools/call','params':{'name':'compare_variants','arguments':{'project_id':'courtyard-demo','run_ids':['r'],'objective':objective},'_meta':meta}}).encode()+b'\n'
invalid={'malformed-json':b'{"jsonrpc":"2.0","id":41,\n','lone-surrogate':frame(41,'\ud800'),'invalid-utf8':frame(41,'REPLACE').replace(b'REPLACE',b'\xff')}
with tempfile.TemporaryDirectory() as d:
    project=pathlib.Path(d)/'project.yaml';shutil.copyfile(root/'examples/courtyard-housing/project.yaml',project)
    for label,bad in invalid.items():
        p=subprocess.Popen([binary,'--project',project],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0)
        p.stdin.write(frame(40));p.stdin.flush();assert select.select([p.stdout],[],[],5)[0];assert json.loads(p.stdout.readline())['id']==40
        p.stdin.write(bad);p.stdin.flush();ready=bool(select.select([p.stdout],[],[],0.4)[0]);early=json.loads(p.stdout.readline()) if ready else None
        p.stdin.write(frame(42,'😀'));p.stdin.flush();assert select.select([p.stdout],[],[],5)[0];after=json.loads(p.stdout.readline())
        print(label,'error within 0.4s',early,'following response id',after.get('id'),'following code',after.get('result',{}).get('structuredContent',{}).get('error',{}).get('code'))
        p.stdin.close();p.wait(timeout=5);assert p.returncode==0
PY
```

Observed:

```text
malformed-json error within 0.4s None following response id 42 following code BACKEND_UNAVAILABLE
lone-surrogate error within 0.4s None following response id 42 following code BACKEND_UNAVAILABLE
invalid-utf8 error within 0.4s None following response id 42 following code BACKEND_UNAVAILABLE
```

I separately initialized the legacy `2025-11-25` session, sent malformed JSON, and observed no response during the same interval followed by a successful request. The code path is shared by both versions.

The [JSON-RPC 2.0 specification](https://www.jsonrpc.org/specification#error_object) assigns `-32700` to invalid JSON, requires a response with an error, and requires `id: null` when the ID cannot be determined; its invalid-JSON example shows that exact shape. Both [MCP 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/basic) and [MCP 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic) say messages follow JSON-RPC 2.0. This is a JSON-RPC error-handling gap in the MCP facade. The [MCP stdio binding](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) also requires the client to send valid UTF-8 MCP messages: the malformed probes are outside compliant client behavior, so they do not establish failure for ordinary valid requests. [RFC 8259 §8.2](https://www.rfc-editor.org/rfc/rfc8259#section-8.2) permits unpaired surrogate escapes in the JSON grammar but identifies them as non-interoperable; the pinned parser rejects them, so they must be discussed separately from unquestionably malformed JSON.

The SDK's `rmcp-3.8.0/src/transport/async_rw.rs` `receive()` classifies serde syntax and EOF failures and explicitly continues without writing. It handles parseable invalid message shapes through its existing locked framed writer. The handler cannot repair frames already discarded by the SDK. A narrow SDK-compatible remediation is an upstream or locally pinned SDK patch at that existing receive branch: for server-side syntax/EOF parse failures, send a bounded `-32700` error through the same serialized writer, then continue; preserve the bounded reader, depth guard, output guard, and handling of well-formed notifications. Add actual stdio regression cases for all three malformed inputs, a valid following request, and EOF. Avoid a second unsynchronized stdout writer or a separate general JSON-RPC stack. The SDK's own test `receive_ignores_parse_error` and this slice's malformed-frame assertions must change with the intended behavior.

There is a serialization detail that prevents describing this as a one-line fix: `rmcp`'s `JsonRpcError.id: Option<RequestId>` uses `skip_serializing_if = Option::is_none`, so `JsonRpcMessage::error(ErrorData::parse_error(...), None)` emits **no `id`**, not JSON-RPC's `id: null`. I confirmed the related current invalid-shape path emits `{"jsonrpc":"2.0","error":{"code":-32600,"message":"Invalid request"}}`. A strict JSON-RPC fix must also represent and serialize a null error ID, with compatibility tests. Both MCP versions describe an unreadable error ID as optional and type it as string or number; the SDK follows that shape. This creates a specification/interop tension with JSON-RPC's explicit null requirement, especially before a malformed request's protocol version is readable. Resolve and test that policy explicitly rather than assuming `None` serializes as null. The proposed patch is a design recommendation, not an implemented or verified fix.

## Scalar schemas and previous fixes

The updated schema adds a surrogate-pair-aware fourth width-tier pattern. Python's `Draft202012Validator` and an independent ECMAScript property-schema evaluator each passed 80 checks over all eight Unicode field occurrences: isolated high/low endpoints, reversed/broken pairs rejected; minimum/maximum valid pairs, emoji, and mixed valid text accepted. The checked-in ECMAScript pattern test also passed:

```sh
node crates/archy-mcp/tests/unicode_regex.js
```

```text
MCP Unicode patterns pass ECMAScript UTF-16 checks for 8 fields
```

The ECMAScript check uses Node's native regex engine and a small evaluator for the affected string properties; no independent full ECMAScript JSON Schema 2020-12 library was installed or tested. Python used `jsonschema`'s 2020-12 validator. The checked-in live test also exercises all eight field occurrences with escaped pairs and raw supplementary UTF-8.

The independent property checks were:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 - <<'PY'
import json,pathlib
from jsonschema import Draft202012Validator
fields=[('compare_variants','objective'),('generate_variants','destination'),('evaluate_variant','destination'),('render_variant','preset'),('render_variant','destination'),('export_ifc','destination'),('propose_approval','destination'),('propose_approval','reason')]
cases=[('high','\ud800',False),('high-end','\udbff',False),('low','\udc00',False),('low-end','\udfff',False),('reversed','\udc00\ud800',False),('broken','\ud800x\udc00',False),('min-pair',json.loads(r'"\ud800\udc00"'),True),('max-pair',json.loads(r'"\udbff\udfff"'),True),('emoji','😀',True),('mixed','x😀é',True)]
count=0
for tool,field in fields:
    schema=json.loads((pathlib.Path('schemas/mcp')/f'{tool}.schema.json').read_text())['input']['properties'][field]
    validator=Draft202012Validator(schema)
    for name,value,expected in cases:
        assert validator.is_valid(value)==expected,(tool,field,name);count+=1
print('Python JSON Schema property checks:',count,'passed across',len(fields),'fields')
PY
node - <<'JS'
const fs=require('fs');
const fields=[['compare_variants','objective'],['generate_variants','destination'],['evaluate_variant','destination'],['render_variant','preset'],['render_variant','destination'],['export_ifc','destination'],['propose_approval','destination'],['propose_approval','reason']];
const cases=[['high','\uD800',false],['high-end','\uDBFF',false],['low','\uDC00',false],['low-end','\uDFFF',false],['reversed','\uDC00\uD800',false],['broken','\uD800x\uDC00',false],['min-pair','\uD800\uDC00',true],['max-pair','\uDBFF\uDFFF',true],['emoji','😀',true],['mixed','x😀é',true]];
function validate(s,x){const len=[...x].length;return (!s.type||s.type==='string') && (s.minLength===undefined||len>=s.minLength) && (s.maxLength===undefined||len<=s.maxLength) && (!s.pattern||new RegExp(s.pattern).test(x)) && (!s.anyOf||s.anyOf.some(b=>validate(b,x)));}
let n=0;for(const [tool,field] of fields){const s=JSON.parse(fs.readFileSync(`schemas/mcp/${tool}.schema.json`)).input.properties[field];for(const [name,x,expected] of cases){if(validate(s,x)!==expected)throw Error(`${tool}.${field} ${name}`);n++}}
console.log('ECMAScript property schema checks:',n,'passed across',fields.length,'fields');
JS
```

The independent 476-case valid-scalar schema/wire matrix from `docs/plans/mcp-first-slice-review-round3.md` was rerun unchanged: zero schema/wire mismatches and zero schema-valid UTF-8 byte-bound overruns. Its 64/65/128 `é`, ASCII 128/129, and two-, three-, four-byte and mixed boundaries cover all affected field occurrences. The fixed-root read path and 1 MiB guards remain in place. The prior explicit revision-zero/old-revision/current-source regression and both impossible envelope cases across all nine schemas pass in the stdio and schema checks below. The width-tier policy remains conservative for mixed text as documented in `docs/mcp/README.md`.

## Evidence and limits

Worktree `/home/butahlecoq/work/archy/.worktrees/a15-mcp`, branch `codex/a15-mcp`, base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`, with uncommitted author changes. Before and after tests, all **28** manifest entries passed `sha256sum -c docs/plans/mcp-first-slice-freeze.sha256`, and the **30**-file patch passed `git apply --reverse --check /tmp/archy-a15-worktree-review.patch`. Manifest SHA-256 was `3172e8b9e025d298f1c3b1f44a0c8f84d4afc970592a2464249a0fc309bee836`; patch SHA-256 was `52254ea1489f77784f2c40a6e520d9befb49790da6ab93d8b5f0b47ac3d01492`. The hashes matched before and after. The status was unchanged before adding this report.

Commands ran from that worktree with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-parallel-review PYTHONDONTWRITEBYTECODE=1`:

- `cargo build --locked -p archy-mcp -p archy-cli`: passed.
- `cargo test --locked -p archy-mcp --test bounds`: four passed.
- `cargo test --locked -p archy-cli --test desktop_readers`: seven passed.
- `python3 crates/archy-mcp/tests/stdio_contract.py`: passed its schema, protocol, history, no-write and guard checks, including its current **silent-drop** expectations; that last assertion demonstrates the gap above, not a passing parse-error test.
- `python3 scripts/check-schemas.py`: passed the nine schema pairs, existing fixtures, and CLI checks.
- `node crates/archy-mcp/tests/unicode_regex.js`: passed for all eight Unicode field occurrences. The independent Python and ECMAScript property checks described above also passed.

The output-size conclusion uses the guard tests and source inspection; a worst-case oversized live output was not generated. Concurrent in-place edits during an individual read and third-party MCP clients were not tested. This review makes no full A15 or target runtime approval. It applied the interrogate approach without child agents and made no product edits, commits, pushes, installations, or changes to other worktrees.
