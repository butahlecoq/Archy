"""Live stdio comparison of accepted history against the CLI's verified report."""
import hashlib
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
SOURCE = ROOT / 'examples/courtyard-housing/project.yaml'
META = {'io.modelcontextprotocol/protocolVersion': '2026-07-28',
        'io.modelcontextprotocol/clientInfo': {'name': 'comparison-test', 'version': '0'},
        'io.modelcontextprotocol/clientCapabilities': {}}
OBJECTIVE = 'absolute_target_gfa_delta'


def send(process, method, params, identifier):
    frame = {'jsonrpc': '2.0', 'id': identifier, 'method': method, 'params': params}
    process.stdin.write(json.dumps(frame, separators=(',', ':')).encode() + b'\n')
    process.stdin.flush()
    assert select.select([process.stdout], [], [], 10)[0], method
    line = process.stdout.readline()
    assert line, (method, process.poll())
    return json.loads(line)


def call(process, arguments, identifier):
    reply = send(process, 'tools/call', {'name': 'compare_variants',
                                        'arguments': arguments, '_meta': META}, identifier)
    assert 'result' in reply, reply
    result = reply['result']
    assert json.loads(result['content'][0]['text']) == result['structuredContent']
    SCHEMA.validate(result['structuredContent'])
    return result


def inventory(root):
    return {str(path.relative_to(root)): (path.stat().st_mtime_ns,
            hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None)
            for path in root.rglob('*')}


def start(project):
    process = subprocess.Popen([str(MCP), '--project', str(project)], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
    discovery = send(process, 'server/discover', {'_meta': META}, 1)
    assert 'result' in discovery, discovery
    return process


def finish(process):
    process.stdin.close()
    process.wait(timeout=10)
    assert process.returncode == 0, process.stderr.read()


SCHEMA = Draft202012Validator(json.loads(
    (ROOT / 'schemas/mcp/compare_variants.schema.json').read_text())['output'])

with tempfile.TemporaryDirectory(prefix='archy-mcp-compare-') as td:
    root = pathlib.Path(td)
    project = root / 'project.yaml'
    shutil.copyfile(SOURCE, project)
    runs = []
    for seed in (42, 43):
        completed = subprocess.run([str(CLI), 'generate', '--project', str(project),
                                    '--count', '3', '--seed', str(seed), '--json'],
                                   capture_output=True, check=True)
        runs.append(json.loads(completed.stdout)['run_id'])
    before = inventory(root)
    process = start(project)
    args = {'project_id': 'courtyard-demo', 'run_ids': runs, 'objective': OBJECTIVE, 'limit': 1}
    first = call(process, args, 2)
    assert not first['isError'], first
    page = first['structuredContent']['data']
    assert page['revision'] == '2' and len(page['comparisons']) == 1 and page['next_cursor']
    assert page['comparisons'][0]['source_run_id'] == runs[0]
    cli = subprocess.run([str(CLI), 'compare', '--project', str(project), '--run', runs[0], '--json'],
                         capture_output=True, check=True)
    assert page['comparisons'][0] == json.loads(cli.stdout)
    second = call(process, {**args, 'cursor': page['next_cursor']}, 3)
    assert not second['isError'], second
    assert second['structuredContent']['data']['comparisons'][0]['source_run_id'] == runs[1]
    assert second['structuredContent']['data']['next_cursor'] is None
    for selected in (['missing'], [runs[0], runs[0]]):
        bad = call(process, {**args, 'run_ids': selected}, 4)
        assert bad['isError'] and bad['structuredContent']['data'] is None
    unsupported = call(process, {**args, 'objective': 'cost'}, 5)
    assert unsupported['isError'] and unsupported['structuredContent']['error']['code'] == 'INVALID_SCHEMA'
    wrong = call(process, {**args, 'project_id': 'other'}, 6)
    assert wrong['isError'] and wrong['structuredContent']['error']['code'] == 'PERMISSION_DENIED'
    stale = call(process, {**args, 'cursor': page['next_cursor'], 'run_ids': list(reversed(runs))}, 7)
    assert stale['isError'] and stale['structuredContent']['error']['code'] == 'REVISION_CONFLICT'
    assert inventory(root) == before
    evaluated = json.loads(subprocess.run([str(CLI), 'evaluate', '--project', str(project),
                                           '--run', runs[1], '--all', '--json'],
                                          capture_output=True, check=True).stdout)
    before = inventory(root)
    wrong_operation = call(process, {**args, 'run_ids': [evaluated['run_id']]}, 8)
    assert wrong_operation['isError'] and wrong_operation['structuredContent']['error']['code'] == 'INVALID_SCHEMA'
    changed_head = call(process, {**args, 'cursor': page['next_cursor']}, 9)
    assert changed_head['isError'] and changed_head['structuredContent']['error']['code'] == 'REVISION_CONFLICT'
    finish(process)
    assert inventory(root) == before

    # A consistently rehashed metric forgery must fail semantic verification.
    revision = root / '.archy/revisions/00000000000000000002.json'
    record = json.loads(revision.read_bytes())
    run_dir = root / record['receipt']['path']
    manifest_file = run_dir / 'manifest.json'
    manifest = json.loads(manifest_file.read_bytes())
    variant_id = manifest['outcomes'][0]['variant_id']
    artifact_file = run_dir / f'evaluations/{variant_id}.json'
    evaluation = json.loads(artifact_file.read_bytes())
    evaluation['metrics'][0]['value'] = 999999.0
    altered = json.dumps(evaluation, separators=(',', ':')).encode()
    artifact_file.write_bytes(altered)
    for artifact in manifest['artifacts']:
        if artifact['path'] == f'evaluations/{variant_id}.json':
            artifact['sha256'] = hashlib.sha256(altered).hexdigest()
            artifact['byte_len'] = len(altered)
    manifest_bytes = json.dumps(manifest, separators=(',', ':')).encode()
    manifest_file.write_bytes(manifest_bytes)
    record['receipt']['manifest_sha256'] = hashlib.sha256(manifest_bytes).hexdigest()
    revision.write_bytes(json.dumps(record, separators=(',', ':')).encode())
    before = inventory(root)
    process = start(project)
    forged = call(process, {**args, 'run_ids': [runs[0]]}, 8)
    assert forged['isError'] and forged['structuredContent']['error']['code'] == 'ARTIFACT_INVALID'
    finish(process)
    assert inventory(root) == before

with tempfile.TemporaryDirectory(prefix='archy-mcp-compare-fixed-') as td:
    root = pathlib.Path(td)
    project = root / 'project.yaml'
    shutil.copyfile(SOURCE, project)
    receipt = json.loads(subprocess.run([str(CLI), 'generate', '--project', str(project),
                                         '--count', '1', '--seed', '42', '--json'],
                                        capture_output=True, check=True).stdout)
    process = start(project)
    replacement = root / 'replacement.yaml'
    shutil.copyfile(project, replacement)
    replacement.replace(project)
    before = inventory(root)
    changed = call(process, {'project_id': 'courtyard-demo', 'run_ids': [receipt['run_id']],
                             'objective': OBJECTIVE}, 9)
    assert changed['isError'] and changed['structuredContent']['error']['code'] == 'REVISION_CONFLICT'
    finish(process)
    assert inventory(root) == before

print('MCP verified comparisons, CLI parity, paging, tamper refusal, fixed source, no writes: PASS')
