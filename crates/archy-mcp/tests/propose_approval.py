"""Live proposal transaction: accepted history, replay, conflicts, and no approval projection."""
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import subprocess
import tempfile

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[3]
TARGET = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
if not TARGET.is_absolute():
    TARGET = Path.cwd() / TARGET
MCP = TARGET / 'debug/archy-mcp'
SCHEMA = Draft202012Validator(json.loads((ROOT / 'schemas/mcp/propose_approval.schema.json').read_text())['output'])
META = {'io.modelcontextprotocol/protocolVersion': '2026-07-28',
        'io.modelcontextprotocol/clientInfo': {'name': 'proposal-contract', 'version': '0'},
        'io.modelcontextprotocol/clientCapabilities': {}}


def call(proc, name, args, ident):
    frame = {'jsonrpc': '2.0', 'id': ident, 'method': 'tools/call',
             'params': {'name': name, 'arguments': args, '_meta': META}}
    proc.stdin.write(json.dumps(frame).encode() + b'\n')
    proc.stdin.flush()
    assert select.select([proc.stdout], [], [], 30)[0], 'MCP timeout'
    reply = json.loads(proc.stdout.readline())
    assert reply['id'] == ident, reply
    if 'result' not in reply:
        return reply
    value = reply['result']['structuredContent']
    assert reply['result']['isError'] == (not value['ok'])
    assert json.loads(reply['result']['content'][0]['text']) == value
    if name == 'propose_approval':
        SCHEMA.validate(value)
    return value


with tempfile.TemporaryDirectory(prefix='archy-proposal-') as directory:
    root = Path(directory)
    project = root / 'project.json'
    shutil.copyfile(ROOT / 'tests/fixtures/briefs/courtyard-v2.json', project)
    brief = json.loads(project.read_text())
    brief['brief']['target_gfa_m2'] = 1200
    brief['brief']['target_gfa_hard'] = True
    project.write_text(json.dumps(brief))
    original = project.read_bytes()
    proc = subprocess.Popen([str(MCP), '--project', str(project)], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
    generation_args = {'project_id': 'courtyard-demo', 'count': 2, 'seed': '42',
                       'idempotency_key': 'proposal-generation', 'expected_project_revision': '0',
                       'destination': 'runs'}
    generated = call(proc, 'generate_variants', generation_args, 1)
    assert generated['ok'], generated
    assert generated['data']['all_required_checks_passed'] is False
    source = generated['data']['run_id']
    variant = generated['data']['variant_ids'][0]
    evaluated = call(proc, 'evaluate_variant', {'project_id': 'courtyard-demo',
        'source_run': source, 'variant_id': variant, 'idempotency_key': 'before-proposal',
        'expected_project_revision': '1', 'destination': 'runs'}, 30)
    assert evaluated['ok'] and evaluated['data']['revision'] == '2', evaluated
    args = {'project_id': 'courtyard-demo', 'source_run': source, 'variant_id': variant,
            'reason': 'Review this candidate, including every failed or unknown check.',
            'idempotency_key': 'proposal-one', 'expected_project_revision': '2',
            'destination': 'runs'}
    before = sorted(str(p.relative_to(root)) for p in root.rglob('*'))
    for ident, changes, code in [
        (2, {'project_id': 'wrong'}, 'PERMISSION_DENIED'),
        (3, {'variant_id': 'missing'}, 'INVALID_SCHEMA'),
        (4, {'expected_project_revision': '1'}, 'REVISION_CONFLICT'),
        (5, {'destination': '../escape'}, 'PERMISSION_DENIED'),
        (6, {'reason': '  '}, 'INVALID_SCHEMA'),
    ]:
        failure = call(proc, 'propose_approval', {**args, **changes,
            'idempotency_key': f'bad-{ident}'}, ident)
        assert failure['error']['code'] == code, failure
        assert sorted(str(p.relative_to(root)) for p in root.rglob('*')) == before
    for ident, field in enumerate(('idempotency_key', 'expected_project_revision', 'destination'), 10):
        malformed = {**args}
        del malformed[field]
        assert call(proc, 'propose_approval', malformed, ident)['error']['code'] == -32602
    assert call(proc, 'propose_approval', {**args, 'actor': 'human'}, 14)['error']['code'] == -32602
    assert call(proc, 'propose_approval', {**args, 'decision': 'select'}, 15)['error']['code'] == -32602
    first = call(proc, 'propose_approval', args, 20)
    assert first['ok'], first
    data = first['data']
    assert set(data) == {'project_id', 'run_id', 'revision', 'path', 'manifest_sha256',
                         'proposal_path', 'proposal_sha256', 'source_run_id', 'variant_id'}
    assert data['revision'] == '3' and data['source_run_id'] == source and data['variant_id'] == variant
    proposal_bytes = (root / data['proposal_path']).read_bytes()
    assert hashlib.sha256(proposal_bytes).hexdigest() == data['proposal_sha256']
    proposal = json.loads(proposal_bytes)
    assert set(proposal) == {'schema_version', 'evidence', 'suggested_reason'}
    assert proposal['evidence']['expected_project_revision'] == 2
    prehead = root / '.archy' / 'revisions' / '00000000000000000002.json'
    assert proposal['evidence']['expected_head_sha256'] == hashlib.sha256(prehead.read_bytes()).hexdigest()
    assert proposal['evidence']['source_run_id'] == source
    assert proposal['evidence']['variant_id'] == variant
    assert any(check['result'] != 'PASS' for check in proposal['evidence']['checks'])
    assert 'actor' not in proposal and 'decision' not in proposal
    manifest = json.loads((root / data['path'] / 'manifest.json').read_text())
    assert manifest['operation'] == 'propose' and manifest['parent_run_id'] == source
    assert manifest['parent_variant_id'] == variant
    assert {a['path'] for a in manifest['artifacts']} == {
        'input.json', 'normalized.json', 'profile.json', 'comparison.json', 'proposal.json',
        f'variants/{variant}.json', f'evaluations/{variant}.json'}
    assert call(proc, 'get_project', {'project_id': 'courtyard-demo'}, 21)['data']['revision'] == '3'
    state = sorted(str(p.relative_to(root)) for p in root.rglob('*'))
    assert call(proc, 'propose_approval', args, 22) == first
    frames = [json.dumps({'jsonrpc': '2.0', 'id': ident, 'method': 'tools/call',
        'params': {'name': 'propose_approval', 'arguments': args, '_meta': META}}).encode() + b'\n'
        for ident in (28, 29)]
    proc.stdin.write(b''.join(frames))
    proc.stdin.flush()
    replies = {}
    for _ in (28, 29):
        assert select.select([proc.stdout], [], [], 30)[0]
        answer = json.loads(proc.stdout.readline())
        replies[answer['id']] = answer['result']['structuredContent']
    assert replies == {28: first, 29: first}
    (root / data['proposal_path']).write_bytes(b'{"forged":"APPROVED"}')
    assert call(proc, 'propose_approval', args, 27)['error']['code'] == 'ARTIFACT_INVALID'
    (root / data['proposal_path']).write_bytes(proposal_bytes)
    assert call(proc, 'propose_approval', {**args, 'reason': 'changed'}, 23)['error']['code'] == 'REVISION_CONFLICT'
    assert call(proc, 'propose_approval', {**args, 'idempotency_key': 'fresh-stale'}, 24)['error']['code'] == 'REVISION_CONFLICT'
    assert sorted(str(p.relative_to(root)) for p in root.rglob('*')) == state
    next_args = {**generation_args, 'idempotency_key': 'later-generation', 'seed': '43',
                 'expected_project_revision': '3'}
    assert call(proc, 'generate_variants', next_args, 25)['ok']
    assert call(proc, 'propose_approval', args, 26) == first
    assert project.read_bytes() == original
    project.write_bytes(original + b'\n')
    assert call(proc, 'propose_approval', args, 31)['error']['code'] == 'REVISION_CONFLICT'
    project.write_bytes(original)
    assert call(proc, 'propose_approval', args, 32) == first
    assert not any('APPROVED' in p.read_text(errors='ignore') for p in root.rglob('*.json'))
    proc.stdin.close()
    assert proc.wait(timeout=10) == 0, proc.stderr.read()
print('MCP proposal accepted source/evidence, replay, conflicts, stale and no approval: PASS')
