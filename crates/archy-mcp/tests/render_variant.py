"""Live MCP render transaction against the fixed project and installed service gate."""
import json
import os
import pathlib
import select
import shutil
import subprocess
import tempfile

from jsonschema import Draft202012Validator

ROOT = pathlib.Path(__file__).resolve().parents[3]
TARGET = pathlib.Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
if not TARGET.is_absolute():
    TARGET = ROOT / TARGET
META = {'io.modelcontextprotocol/protocolVersion': '2026-07-28',
        'io.modelcontextprotocol/clientInfo': {'name': 'render-contract', 'version': '0'},
        'io.modelcontextprotocol/clientCapabilities': {}}


def call(process, name, arguments, ident):
    frame = {'jsonrpc': '2.0', 'id': ident, 'method': 'tools/call',
             'params': {'name': name, 'arguments': arguments, '_meta': META}}
    process.stdin.write(json.dumps(frame).encode() + b'\n')
    process.stdin.flush()
    assert select.select([process.stdout], [], [], 30)[0], 'MCP response timeout'
    answer = json.loads(process.stdout.readline())
    assert answer['id'] == ident, answer
    return answer


def result(process, arguments, ident, schema):
    answer = call(process, 'render_variant', arguments, ident)
    assert 'result' in answer, answer
    reply = answer['result']
    value = reply['structuredContent']
    schema.validate(value)
    assert json.loads(reply['content'][0]['text']) == value
    assert reply['isError'] == (not value['ok'])
    return value


with tempfile.TemporaryDirectory(prefix='archy-mcp-render-') as temp:
    root = pathlib.Path(temp)
    project = root / 'project.json'
    shutil.copyfile(ROOT / 'tests/fixtures/briefs/courtyard-v2.json', project)
    project_bytes = project.read_bytes()
    process = subprocess.Popen([str(TARGET / 'debug/archy-mcp'), '--project', str(project)],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    generated = call(process, 'generate_variants', {
        'project_id': 'courtyard-demo', 'count': 1, 'seed': '42',
        'idempotency_key': 'render-generate', 'expected_project_revision': '0',
        'destination': 'runs'}, 1)['result']['structuredContent']['data']
    evaluated = call(process, 'evaluate_variant', {
        'project_id': 'courtyard-demo', 'source_run': generated['run_id'],
        'variant_id': generated['variant_ids'][0], 'idempotency_key': 'render-evaluate',
        'expected_project_revision': '1', 'destination': 'runs'}, 2)['result']['structuredContent']['data']
    args = {'project_id': 'courtyard-demo', 'source_run': evaluated['run_id'],
            'variant_id': generated['variant_ids'][0], 'preset': 'draft',
            'idempotency_key': 'render-one', 'expected_project_revision': '2',
            'destination': 'runs'}
    schema = Draft202012Validator(json.loads((ROOT / 'schemas/mcp/render_variant.schema.json').read_text())['output'])
    outside = root / 'outside'
    outside.mkdir()
    (root / 'link').symlink_to(outside, target_is_directory=True)
    inventory = lambda: sorted(str(p.relative_to(root)) for p in root.rglob('*'))
    before = inventory()
    for ident, changes, code in [
        (3, {'project_id': 'other'}, 'PERMISSION_DENIED'),
        (4, {'preset': 'unknown'}, 'INVALID_SCHEMA'),
        (5, {'variant_id': 'absent'}, 'INVALID_SCHEMA'),
        (6, {'destination': '../escape'}, 'PERMISSION_DENIED'),
        (7, {'expected_project_revision': '1'}, 'REVISION_CONFLICT'),
        (15, {'destination': 'link'}, 'PERMISSION_DENIED'),
    ]:
        value = result(process, {**args, **changes, 'idempotency_key': f'bad-{ident}'}, ident, schema)
        assert value['error']['code'] == code, value
        assert inventory() == before, (ident, inventory())
    missing = {**args}
    missing.pop('idempotency_key')
    assert call(process, 'render_variant', missing, 8)['error']['code'] == -32602
    assert inventory() == before
    gate_available = pathlib.Path('/usr/lib/archy/archy-service-gate').exists()
    for ident in (9, 10):
        frame = {'jsonrpc': '2.0', 'id': ident, 'method': 'tools/call',
                 'params': {'name': 'render_variant', 'arguments': args, '_meta': META}}
        process.stdin.write(json.dumps(frame).encode() + b'\n')
    process.stdin.flush()
    concurrent = {}
    for _ in range(2):
        assert select.select([process.stdout], [], [], 30)[0], 'concurrent MCP timeout'
        answer = json.loads(process.stdout.readline())
        concurrent[answer['id']] = answer['result']['structuredContent']
    first = concurrent[9]
    assert concurrent[10] == first
    schema.validate(first)
    assert first['ok'] and first['data']['revision'] == '3', first
    variant = first['data']['variants'][0]
    assert variant['variant_id'] == args['variant_id']
    assert first['data']['all_verified'] is (variant['result']['status'] == 'verified')
    if gate_available:
        assert variant['result']['status'] in ('verified', 'failed'), variant
        if variant['result']['status'] == 'verified':
            assert variant['result']['artifacts']
            for artifact in variant['result']['artifacts']:
                assert (root / first['data']['path'] / artifact['path']).is_file()
    else:
        assert variant['result']['status'] == 'failed'
        assert variant['result']['error']['code'] == 'BACKEND_UNAVAILABLE'
        assert first['data']['all_verified'] is False
    assert result(process, args, 13, schema) == first
    changed = result(process, {**args, 'variant_id': 'different'}, 11, schema)
    assert changed['error']['code'] == 'REVISION_CONFLICT', changed
    changed_source = result(process, {**args, 'source_run': '00000000-0000-0000-0000-000000000000'}, 16, schema)
    assert changed_source['error']['code'] == 'REVISION_CONFLICT', changed_source
    stale = result(process, {**args, 'idempotency_key': 'new-stale'}, 12, schema)
    assert stale['error']['code'] == 'REVISION_CONFLICT', stale
    assert project.read_bytes() == project_bytes
    assert list(outside.iterdir()) == []
    record = root / first['data']['path'] / 'render.json'
    record.write_bytes(b'{}')
    damaged = result(process, args, 14, schema)
    assert damaged['error']['code'] == 'ARTIFACT_INVALID', damaged
    process.stdin.close()
    process.wait(timeout=5)
    assert process.returncode == 0, process.stderr.read().decode()
print('actual MCP render: scoped receipt, replay, stale, escaping, no writes on input failure: PASS')

with tempfile.TemporaryDirectory(prefix='archy-mcp-render-v1-') as temp:
    root = pathlib.Path(temp) / 'historical'
    shutil.copytree(ROOT / 'tests/fixtures/historical-claimed-v1', root)
    for directory in [root / '.archy', root / 'runs']:
        for child in [directory, *directory.rglob('*')]:
            if child.is_dir():
                child.chmod(0o700)
    (root / '.archy/lock').chmod(0o600)
    process = subprocess.Popen([str(TARGET / 'debug/archy-mcp'), '--project', str(root / 'project.json')],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    source_run = 'a112afe0-ab7b-414d-8f35-11d5f97814c9'
    variant_id = 'variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b'
    arguments = {'project_id': 'courtyard-demo', 'source_run': source_run,
                 'variant_id': variant_id, 'preset': 'draft',
                 'idempotency_key': 'historical-v1-render',
                 'expected_project_revision': '2', 'destination': 'runs'}
    reply = result(process, arguments, 1, schema)
    assert reply['ok'], reply
    assert reply['data']['revision'] == '3'
    assert reply['data']['source_run_id'] == source_run
    assert reply['data']['all_verified'] is (reply['data']['variants'][0]['result']['status'] == 'verified')
    assert not (root / reply['data']['path'] / 'profile.json').exists()
    assert result(process, arguments, 2, schema) == reply
    process.stdin.close()
    process.wait(timeout=5)
    assert process.returncode == 0, process.stderr.read().decode()
print('actual MCP historical v1 evaluated source and replay: PASS')
