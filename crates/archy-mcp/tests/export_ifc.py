"""Observable MCP IFC export contract against the actual CLI transaction and service gate."""
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
    TARGET = pathlib.Path.cwd() / TARGET
MCP = TARGET / 'debug/archy-mcp'
CLI = TARGET / 'debug/archy'
SCHEMA = Draft202012Validator(json.loads(
    (ROOT / 'schemas/mcp/export_ifc.schema.json').read_text())['output'])
META = {'io.modelcontextprotocol/protocolVersion': '2026-07-28',
        'io.modelcontextprotocol/clientInfo': {'name': 'ifc-contract', 'version': '0'},
        'io.modelcontextprotocol/clientCapabilities': {}}


def frame(name, args, ident):
    return json.dumps({'jsonrpc': '2.0', 'id': ident, 'method': 'tools/call',
                       'params': {'name': name, 'arguments': args, '_meta': META}}).encode() + b'\n'


def calls(process, requests):
    process.stdin.write(b''.join(frame(*request) for request in requests))
    process.stdin.flush()
    answers = {}
    for _ in requests:
        assert select.select([process.stdout], [], [], 30)[0], 'MCP timeout'
        answer = json.loads(process.stdout.readline())
        answers[answer['id']] = answer
    return answers


def result(answer):
    assert 'result' in answer, answer
    reply = answer['result']
    value = reply['structuredContent']
    SCHEMA.validate(value)
    assert json.loads(reply['content'][0]['text']) == value
    assert reply['isError'] == (not value['ok'])
    return value


with tempfile.TemporaryDirectory(prefix='archy-mcp-ifc-') as temp:
    root = pathlib.Path(temp)
    project = root / 'project.json'
    shutil.copyfile(ROOT / 'tests/fixtures/briefs/courtyard-v2.json', project)
    original = project.read_bytes()
    process = subprocess.Popen([str(MCP), '--project', str(project)], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
    generation = calls(process, [('generate_variants', {
        'project_id': 'courtyard-demo', 'count': 1, 'seed': '42',
        'idempotency_key': 'ifc-generate', 'expected_project_revision': '0',
        'destination': 'runs'}, 1)])[1]['result']['structuredContent']['data']
    args = {'project_id': 'courtyard-demo', 'source_run': generation['run_id'],
            'variant_id': generation['variant_ids'][0], 'idempotency_key': 'ifc-one',
            'expected_project_revision': '1', 'destination': 'runs'}
    outside = root / 'outside'
    outside.mkdir()
    (root / 'link').symlink_to(outside, target_is_directory=True)
    before = sorted(str(path.relative_to(root)) for path in root.rglob('*'))
    failures = [
        (2, {'project_id': 'other'}, 'PERMISSION_DENIED'),
        (3, {'source_run': 'bogus'}, 'INVALID_SCHEMA'),
        (4, {'variant_id': 'absent'}, 'INVALID_SCHEMA'),
        (5, {'expected_project_revision': '0'}, 'REVISION_CONFLICT'),
        (6, {'destination': '../outside'}, 'PERMISSION_DENIED'),
        (7, {'destination': 'link'}, 'PERMISSION_DENIED'),
        (8, {'destination': '/tmp/outside'}, 'PERMISSION_DENIED'),
    ]
    for ident, changes, code in failures:
        value = result(calls(process, [('export_ifc', {
            **args, **changes, 'idempotency_key': f'ifc-bad-{ident}'}, ident)])[ident])
        assert value['error']['code'] == code, value
        assert sorted(str(path.relative_to(root)) for path in root.rglob('*')) == before
    for ident, field in enumerate(('idempotency_key', 'expected_project_revision', 'destination'), 20):
        invalid = {**args}
        invalid.pop(field)
        assert calls(process, [('export_ifc', invalid, ident)])[ident]['error']['code'] == -32602
    for ident, changes in [(24, {'idempotency_key': ''}),
                           (25, {'expected_project_revision': '01'}),
                           (26, {'destination': 'x' * 513})]:
        assert calls(process, [('export_ifc', {**args, **changes}, ident)])[ident]['error']['code'] == -32602
    assert sorted(str(path.relative_to(root)) for path in root.rglob('*')) == before
    reused = result(calls(process, [('export_ifc', {
        **args, 'idempotency_key': 'ifc-generate'}, 27)])[27])
    assert reused['error']['code'] == 'REVISION_CONFLICT', reused
    assert sorted(str(path.relative_to(root)) for path in root.rglob('*')) == before

    answers = calls(process, [('export_ifc', args, 30), ('export_ifc', args, 31)])
    first, second = result(answers[30]), result(answers[31])
    if not first['ok']:
        assert first['error']['code'] == second['error']['code'] == 'BACKEND_UNAVAILABLE', (first, second)
        assert first['data'] is second['data'] is None
        cli = subprocess.run([str(CLI), 'export-ifc', '--project', str(project),
                              '--run', args['source_run'], '--variant', args['variant_id'],
                              '--idempotency-key', args['idempotency_key'],
                              '--expected-project-revision', '1', '--destination', 'runs', '--json'],
                             capture_output=True)
        assert cli.returncode != 0 and json.loads(cli.stdout) == first['error'], cli
        assert sorted(str(path.relative_to(root)) for path in root.rglob('*')) == before
        assert result(calls(process, [('export_ifc', args, 32)])[32])['error']['code'] == 'BACKEND_UNAVAILABLE'
        print('MCP IFC: actual absent backend returned errors; no fake receipt or revision: PASS')
    else:
        assert first == second and first['ok'], (first, second)
        data = first['data']
        assert data['approval_state'] == 'DRAFT' and data['scope'] == 'SCHEMATIC_MASSING_ONLY'
        assert data['project_id'] == args['project_id'] and data['source_run_id'] == args['source_run']
        assert data['variant_id'] == args['variant_id'] and data['revision'] == '2'
        assert (root / data['path'] / 'ifc/model.ifc').is_file()
        assert (root / data['path'] / 'design-manifest.json').is_file()
        cli = subprocess.run([str(CLI), 'export-ifc', '--project', str(project),
                              '--run', args['source_run'], '--variant', args['variant_id'],
                              '--idempotency-key', args['idempotency_key'],
                              '--expected-project-revision', '1', '--destination', 'runs', '--json'],
                             capture_output=True, check=True)
        receipt = json.loads(cli.stdout)
        assert all(data[field] == str(receipt[field]) for field in
                   ('run_id', 'revision', 'path', 'manifest_sha256'))
        for ident, changes in [(33, {'variant_id': 'different'}),
                               (34, {'source_run': '00000000-0000-0000-0000-000000000000'}),
                               (35, {'destination': 'other'})]:
            assert result(calls(process, [('export_ifc', {**args, **changes}, ident)])[ident])['error']['code'] == 'REVISION_CONFLICT'
        assert result(calls(process, [('export_ifc', {**args, 'idempotency_key': 'ifc-stale'}, 36)])[36])['error']['code'] == 'REVISION_CONFLICT'
        assert result(calls(process, [('export_ifc', args, 37)])[37]) == first
        print('MCP IFC: verified receipt, CLI replay equivalence and conflicts: PASS')
    assert project.read_bytes() == original and list(outside.iterdir()) == []
    moved = root / 'moved-project.json'
    project.rename(moved)
    shutil.copyfile(moved, project)
    rebound = result(calls(process, [('export_ifc', {
        **args, 'idempotency_key': 'ifc-rebound'}, 38)])[38])
    assert rebound['error']['code'] == 'REVISION_CONFLICT', rebound
    process.stdin.close()
    process.wait(timeout=5)
    assert process.returncode == 0, process.stderr.read().decode()
