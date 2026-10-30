"""Actual SDK stdio protocol and read-only contract; run after cargo build -p archy-mcp."""
import json, os, pathlib, select, shutil, subprocess, tempfile
from jsonschema import Draft202012Validator
ROOT = pathlib.Path(__file__).resolve().parents[3]
TARGET = pathlib.Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
if not TARGET.is_absolute(): TARGET = pathlib.Path.cwd() / TARGET
BIN = TARGET / 'debug/archy-mcp'
SOURCE = ROOT / 'examples/courtyard-housing/project.yaml'
META = {'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientInfo':{'name':'archy-contract','version':'0'},'io.modelcontextprotocol/clientCapabilities':{}}
NAMES = ['get_project','validate_project','list_variants','generate_variants','evaluate_variant','render_variant','compare_variants','export_ifc','propose_approval']

def send(p, method, params, id):
    p.stdin.write(json.dumps({'jsonrpc':'2.0','id':id,'method':method,'params':params},separators=(',',':')).encode()+b'\n'); p.stdin.flush()
    assert select.select([p.stdout],[],[],5)[0], (method,'timeout')
    line=p.stdout.readline(); assert line, (method,p.poll())
    return json.loads(line)

def start(project): return subprocess.Popen([str(BIN),'--project',str(project)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
def finish(p):
    p.stdin.close(); p.wait(timeout=5)
    assert p.returncode == 0, p.stderr.read().decode()

def call(p,name,args,id,modern=True):
    params={'name':name,'arguments':args}
    if modern: params['_meta']=META
    return send(p,'tools/call',params,id)

with tempfile.TemporaryDirectory(prefix='archy-mcp-stdio-') as td:
    project=pathlib.Path(td)/'project.yaml'; shutil.copyfile(SOURCE,project)
    original=project.read_bytes()
    p=start(project)
    assert send(p,'server/discover',{'_meta':META},1)['result']['supportedVersions']==['2025-11-25','2026-07-28']
    listed=send(p,'tools/list',{'_meta':META},2)['result']['tools']
    assert [v['name'] for v in listed]==NAMES
    for tool in listed:
        file=json.loads((ROOT/'schemas/mcp'/f'{tool["name"]}.schema.json').read_text())
        assert tool['inputSchema']==file['input'] and tool['outputSchema']==file['output']
    project_args={'project_id':'courtyard-demo'}
    schemas={tool['name']: Draft202012Validator(tool['inputSchema']) for tool in listed}
    mutation={'source_run':'r','variant_id':'v','idempotency_key':'k','expected_project_revision':'0','destination':'runs'}
    compare={**project_args,'run_ids':['r'],'objective':'area'}
    cases=[
        ('get_project', project_args, 'project_id', 128, True),
        ('evaluate_variant', {**project_args,**mutation}, 'source_run', 128, True),
        ('evaluate_variant', {**project_args,**mutation}, 'variant_id', 128, True),
        ('evaluate_variant', {**project_args,**mutation}, 'idempotency_key', 128, True),
        ('list_variants', project_args, 'cursor', 256, True),
        ('compare_variants', compare, 'run_ids', 128, True),
        ('compare_variants', compare, 'objective', 128, False),
        ('render_variant', {**project_args,**mutation,'preset':'draft'}, 'preset', 128, False),
        ('evaluate_variant', {**project_args,**mutation}, 'destination', 512, False),
        ('propose_approval', {**project_args,**mutation,'reason':'review'}, 'reason', 4096, False),
    ]
    for tool,base,field,limit,ascii_only in cases:
        values=[('a'*limit,True),('a'*(limit+1),False)]
        if ascii_only:
            values += [('é'*64,False),('é'*65,False),('a\u2028',False),('a\n',True)]
        else:
            for char,size in [('é',2),('€',3),('😀',4)]:
                values.extend([(char*(limit//size),True),(char*(limit//size+1),False)])
                values.extend([('a'*(limit//size-1)+char,True),('a'*(limit//size)+char,False)])
        for index,(value,accepted) in enumerate(values):
            args={**base,field: [value] if field=='run_ids' else value}
            assert schemas[tool].is_valid(args)==accepted,(tool,field,len(value),accepted,'schema')
            reply=call(p,tool,args,100+index)
            if accepted:
                assert 'result' in reply,(tool,field,len(value),reply)
            else:
                assert reply.get('error',{}).get('code')==-32602,(tool,field,len(value),reply)
    print('MCP bounded string schemas match live ASCII and Unicode inputs: PASS')
    for i,name in enumerate(NAMES[:3],3):
        r=call(p,name,project_args,i)['result']; assert r['isError'] is False,(name,r)
        assert r['structuredContent']['ok'] is True
        assert json.loads(r['content'][0]['text'])==r['structuredContent']
        Draft202012Validator(json.loads((ROOT/'schemas/mcp'/f'{name}.schema.json').read_text())['output']).validate(r['structuredContent'])
    assert call(p,'get_project',{'project_id':'courtyard-demo','revision':'0'},7)['result']['structuredContent']['data']['revision']=='0'
    assert call(p,'get_project',{'project_id':'courtyard-demo','revision':None},23)['result']['structuredContent']['ok']
    assert call(p,'get_project',{'project_id':'courtyard-demo','revision':0},8)['error']['code']==-32602
    assert call(p,'get_project',{'project_id':'other'},9)['result']['structuredContent']['error']['code']=='PERMISSION_DENIED'
    assert call(p,'list_variants',{'project_id':'courtyard-demo','limit':101},10)['error']['code']==-32602
    assert call(p,'list_variants',{'project_id':'courtyard-demo','cursor':'wrong:1:id'},11)['result']['structuredContent']['error']['code']=='REVISION_CONFLICT'
    for i,(name,extra) in enumerate(zip(NAMES[5:],[
        {'source_run':'r','variant_id':'v','preset':'draft','idempotency_key':'k','expected_project_revision':'0','destination':'runs'},
        {'run_ids':['r'],'objective':'area'},
        {'source_run':'r','variant_id':'v','idempotency_key':'k','expected_project_revision':'0','destination':'runs'},
        {'source_run':'r','variant_id':'v','reason':'review','idempotency_key':'k','expected_project_revision':'0','destination':'runs'}],strict=True),12):
        r=call(p,name,{**project_args,**extra},i)['result']; assert r['isError'] is True,(name,r)
        expected='INVALID_SCHEMA' if name in ('render_variant','compare_variants','export_ifc','propose_approval') else 'BACKEND_UNAVAILABLE'
        assert r['structuredContent']['error']['code']==expected,name
        Draft202012Validator(json.loads((ROOT/'schemas/mcp'/f'{name}.schema.json').read_text())['output']).validate(r['structuredContent'])
    assert call(p,'unknown',{},20)['error']['code']==-32601
    assert call(p,'generate_variants',{**project_args,'count':1001,'seed':'1','idempotency_key':'k','expected_project_revision':'0','destination':'runs'},22)['error']['code']==-32602
    assert call(p,'compare_variants',{**project_args,'run_ids':['x'*129],'objective':'area'},24)['error']['code']==-32602
    p.stdin.write(b' '*(1_048_576+4096)+b'\n'); p.stdin.flush()
    assert call(p,'validate_project',project_args,21)['result']['structuredContent']['ok']
    finish(p)
    assert project.read_bytes()==original and sorted(x.name for x in pathlib.Path(td).iterdir())==['project.yaml']
    p=start(project)
    init=send(p,'initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'test','version':'0'}},1)
    assert init['result']['protocolVersion']=='2025-11-25'
    p.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n');p.stdin.flush()
    assert call(p,'validate_project',project_args,2,False)['result']['structuredContent']['ok']
    finish(p)
    p=start(project)
    init=send(p,'initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'test','version':'0'}},1)
    assert init['error']['code']<0 and 'unsupported' in init['error']['message'].lower(),init
    p.stdin.close(); p.wait(timeout=5)
    assert p.returncode == 1 and b"MCP transport startup failed" in p.stderr.read()
    assert sorted(x.name for x in pathlib.Path(td).iterdir())==['project.yaml']
print('MCP actual modern/legacy, nine schemas, three base reads, four typed errors, bounds and no writes: PASS')
with tempfile.TemporaryDirectory(prefix='archy-mcp-history-') as td:
    project=pathlib.Path(td)/'project.yaml'; shutil.copyfile(SOURCE,project)
    cli=TARGET/'debug/archy'
    for seed in (42,43):
        made=subprocess.run([str(cli),'generate','--project',str(project),'--count','3','--seed',str(seed),'--json'],capture_output=True,check=True)
    def inventory():
        return {str(p.relative_to(td)):(p.stat().st_mtime_ns,p.read_bytes() if p.is_file() else None) for p in pathlib.Path(td).rglob('*')}
    before=inventory()
    p=start(project)
    first=call(p,'list_variants',{'project_id':'courtyard-demo','limit':2},1,True)['result']['structuredContent']['data']
    assert len(first['variants'])==2 and first['next_cursor']
    second=call(p,'list_variants',{'project_id':'courtyard-demo','limit':2,'cursor':first['next_cursor']},2,True)['result']['structuredContent']['data']
    assert len(second['variants'])==2 and set(v['variant_id'] for v in first['variants']).isdisjoint(v['variant_id'] for v in second['variants'])
    bad_cursor=first['next_cursor'].rsplit(':',1)[0]+':forged'
    bad=call(p,'list_variants',{'project_id':'courtyard-demo','limit':2,'cursor':bad_cursor},3,True)['result']
    assert bad['isError'] and bad['structuredContent']['error']['code']=='REVISION_CONFLICT'
    assert first['revision']=='2'
    finish(p)
    assert inventory()==before
    subprocess.run([str(cli),'generate','--project',str(project),'--count','1','--seed','44','--json'],capture_output=True,check=True)
    before=inventory()
    p=start(project)
    stale=call(p,'list_variants',{'project_id':'courtyard-demo','limit':2,'cursor':first['next_cursor']},1,True)['result']
    assert stale['isError'] and stale['structuredContent']['error']['code']=='REVISION_CONFLICT'
    finish(p)
    assert inventory()==before
print('MCP verified multi-variant paging, stale cursor and no read mutation: PASS')
with tempfile.TemporaryDirectory(prefix='archy-mcp-corrupt-') as td:
    project=pathlib.Path(td)/'project.yaml'; shutil.copyfile(SOURCE,project)
    archy=pathlib.Path(td)/'.archy'; archy.mkdir(mode=0o700)
    p=start(project)
    bad=call(p,'get_project',{'project_id':'courtyard-demo'},1)['result']
    assert bad['isError'] and bad['structuredContent']['error']['code']=='ARTIFACT_INVALID'
    finish(p)
    assert sorted(x.name for x in archy.iterdir())==[]
    archy.rmdir()
    archy.symlink_to(td,target_is_directory=True)
    p=start(project)
    bad=call(p,'validate_project',{'project_id':'courtyard-demo'},1)['result']
    assert bad['isError'] and bad['structuredContent']['error']['code'] in ('PERMISSION_DENIED','ARTIFACT_INVALID')
    finish(p)
    assert archy.is_symlink()
print('MCP corrupt and symlink history refusal without repair: PASS')
with tempfile.TemporaryDirectory(prefix='archy-mcp-swap-') as td:
    project=pathlib.Path(td)/'project.yaml'; shutil.copyfile(SOURCE,project)
    p=start(project)
    assert call(p,'validate_project',{'project_id':'courtyard-demo'},1)['result']['structuredContent']['ok']
    replacement=pathlib.Path(td)/'replacement.yaml'; shutil.copyfile(SOURCE,replacement); replacement.replace(project)
    conflict=call(p,'get_project',{'project_id':'courtyard-demo'},2)['result']
    assert conflict['isError'] and conflict['structuredContent']['error']['code']=='REVISION_CONFLICT'
    finish(p)
    assert sorted(x.name for x in pathlib.Path(td).iterdir())==['project.yaml']
print('MCP live pinned-source replacement refusal: PASS')
with tempfile.TemporaryDirectory(prefix='archy-mcp-revision-zero-') as td:
    project=pathlib.Path(td)/'project.yaml'; shutil.copyfile(SOURCE,project)
    old_bytes=project.read_bytes()
    old_digest=__import__('hashlib').sha256(old_bytes).hexdigest()
    subprocess.run([str(TARGET/'debug/archy'),'generate','--project',str(project),'--count','1','--seed','42','--json'],capture_output=True,check=True)
    project.write_bytes(old_bytes.replace(b'  width_m: 20',b'  width_m: 21'))
    new_digest=__import__('hashlib').sha256(project.read_bytes()).hexdigest()
    assert old_digest != new_digest
    p=start(project)
    zero=call(p,'get_project',{'project_id':'courtyard-demo','revision':'0'},1)['result']
    assert zero['isError'] and zero['structuredContent']['error']['code']=='REVISION_CONFLICT'
    assert zero['structuredContent']['data'] is None
    first=call(p,'get_project',{'project_id':'courtyard-demo','revision':'1'},2)['result']['structuredContent']['data']
    assert first['revision']=='1' and first['source_sha256']==old_digest
    current=call(p,'get_project',{'project_id':'courtyard-demo'},3)['result']['structuredContent']['data']
    assert current['revision']=='1' and current['source_sha256']==new_digest
    finish(p)
    assert __import__('hashlib').sha256(project.read_bytes()).hexdigest()==new_digest
print('MCP historical zero refuses without genesis snapshot; current source stays current: PASS')

with tempfile.TemporaryDirectory(prefix='archy-mcp-v2-history-') as td:
    project=pathlib.Path(td)/'project.json'
    shutil.copyfile(ROOT/'tests/fixtures/briefs/courtyard-v2.json',project)
    p=start(project)
    current=call(p,'validate_project',{'project_id':'courtyard-demo'},1)['result']['structuredContent']['data']['normalized']
    assert current['schema_version']=='0.2.0' and current['normalization_version']=='si-rectangle-v2'
    assert current['id']=='courtyard-demo' and current['constraints'][0]['bound']>0
    finish(p)
    subprocess.run([str(TARGET/'debug/archy'),'generate','--project',str(project),'--count','1','--seed','42','--json'],capture_output=True,check=True)
    p=start(project)
    historical=call(p,'get_project',{'project_id':'courtyard-demo','revision':'1'},2)['result']['structuredContent']['data']['normalized']
    assert historical==current
    finish(p)
print('MCP current v2 and verified historical v2 normalized profiles: PASS')

with tempfile.TemporaryDirectory(prefix='archy-mcp-v1-marker-history-') as td:
    capture=ROOT/'tests/fixtures/legacy-preference-run'
    project=pathlib.Path(td)/'project.json'
    shutil.copyfile(capture/'project.json',project)
    shutil.copytree(capture/'.archy',pathlib.Path(td)/'.archy')
    shutil.copytree(capture/'archived-runs',pathlib.Path(td)/'runs')
    for directory in [pathlib.Path(td)/'.archy',pathlib.Path(td)/'runs']:
        for child in [directory,*directory.rglob('*')]:
            if child.is_dir(): child.chmod(0o700)
    project.write_bytes(b'{bad json')
    p=start(project)
    historical=call(p,'get_project',{'project_id':'courtyard-demo','revision':'1'},3)['result']['structuredContent']['data']['normalized']
    assert historical['schema_version']=='0.1.0' and historical['normalization_version']=='si-rectangle-v1'
    rejected=call(p,'validate_project',{'project_id':'courtyard-demo'},4)['result']['structuredContent']
    assert rejected['ok'] is False and rejected['error']['code']=='INVALID_SCHEMA'
    finish(p)
print('MCP authentic v1 history survives malformed current source without claiming current validity: PASS')

# A lone UTF-16 surrogate is not a Unicode scalar. The input adapter reports
# a parse error, and schemas reject it while retaining valid pairs.
with tempfile.TemporaryDirectory(prefix='archy-mcp-unicode-') as td:
    project=pathlib.Path(td)/'project.yaml'; shutil.copyfile(SOURCE,project)
    original=project.read_bytes()
    mutation={'project_id':'courtyard-demo','source_run':'r','variant_id':'v',
              'idempotency_key':'k','expected_project_revision':'0','destination':'runs'}
    fields=[('compare_variants','objective',{'project_id':'courtyard-demo','run_ids':['r'],'objective':'area'}),
            ('generate_variants','destination',{'project_id':'wrong-project','count':1,'seed':'1',
                                               'idempotency_key':'k','expected_project_revision':'0','destination':'runs'}),
            ('evaluate_variant','destination',mutation),
            ('render_variant','preset',{**mutation,'preset':'draft'}),
            ('render_variant','destination',{**mutation,'preset':'draft'}),
            ('export_ifc','destination',mutation),
            ('propose_approval','destination',{**mutation,'reason':'review'}),
            ('propose_approval','reason',{**mutation,'reason':'review'})]
    def request(identifier,tool,args):
        return {'jsonrpc':'2.0','id':identifier,'method':'tools/call',
                'params':{'name':tool,'arguments':args,'_meta':META}}
    def run_frames(first,second):
        result=subprocess.run([str(BIN),'--project',str(project)],input=first+b'\n'+second+b'\n',
                              stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=5,check=False)
        assert result.returncode==0,(result.returncode,result.stderr)
        return {reply['id']:reply for reply in map(json.loads,result.stdout.splitlines())}
    for tool,field,base in fields:
        pair=json.loads((ROOT/'schemas/mcp'/f'{tool}.schema.json').read_text())
        validator=Draft202012Validator(pair['input'])
        for label,value,valid in [('high-min','\ud800',False),('high-max','\udbff',False),
                                  ('low-min','\udc00',False),('low-max','\udfff',False),
                                  ('high-in-text','a\ud800b',False),('reversed','\udfff\ud800',False),
                                  ('pair-min','\ud800\udc00',True),('pair-max','\udbff\udfff',True),
                                  ('paired','\ud83d\ude00',True),('scalar','😀',True)]:
            args={**base,field:value}
            assert validator.is_valid(args)==valid,(tool,field,label,'schema')
            first=json.dumps(request(1,tool,args)).encode()
            second=json.dumps(request(2,tool,base)).encode()
            replies=run_frames(first,second)
            assert set(replies)==({1,2} if valid else {None,2}),(tool,field,label,replies)
            expected=('INVALID_SCHEMA' if tool in ('compare_variants','evaluate_variant','render_variant','export_ifc','propose_approval') else
                      'PERMISSION_DENIED' if tool=='generate_variants' else 'BACKEND_UNAVAILABLE')
            assert replies[2]['result']['structuredContent']['error']['code']==expected
            if valid:
                assert replies[1]['result']['structuredContent']['error']['code']==expected
                Draft202012Validator(pair['output']).validate(replies[1]['result']['structuredContent'])
            else:
                assert replies[None]=={'jsonrpc':'2.0','id':None,'error':{'code':-32700,'message':'Parse error'}}
        raw_args={**base,field:'😀'}
        raw=json.dumps(request(1,tool,raw_args),ensure_ascii=False).encode('utf-8')
        replies=run_frames(raw,json.dumps(request(2,tool,base)).encode())
        assert sorted(replies)==[1,2],(tool,field,'raw UTF-8',replies)
    tool,field,base=fields[0]
    malformed=json.dumps(request(1,tool,base)).encode().replace(b'"area"',b'"\xff"')
    replies=run_frames(malformed,json.dumps(request(2,tool,base)).encode())
    assert set(replies)=={None,2} and replies[None]['error']['code']==-32700,('malformed UTF-8',replies)
    replies=run_frames(b'{"jsonrpc":"2.0",',json.dumps(request(2,tool,base)).encode())
    assert set(replies)=={None,2} and replies[None]['error']['code']==-32700,('malformed JSON',replies)
    lone=json.dumps(request(1,tool,{**base,field:'\ud800'})).encode()+b'\n'
    replies=run_frames(json.dumps(request(2,tool,base)).encode(),lone.rstrip(b'\n'))
    assert set(replies)=={None,2} and replies[None]['error']['code']==-32700,('malformed frame after valid request',replies)
    stopped=subprocess.run([str(BIN),'--project',str(project)],input=lone,stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE,timeout=5,check=False)
    assert stopped.returncode==1
    assert json.loads(stopped.stdout)=={'jsonrpc':'2.0','id':None,'error':{'code':-32700,'message':'Parse error'}}
    assert stopped.stderr==b'MCP transport startup failed\n'
    assert project.read_bytes()==original and sorted(p.name for p in pathlib.Path(td).iterdir())==['project.yaml']
print('MCP Unicode scalar schemas and malformed-frame response/shutdown: PASS')
