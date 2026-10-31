"""Live stdio transactions against the built archy-mcp binary."""
import json
import hashlib
import os
import pathlib
import select
import shutil
import subprocess
import tempfile
import time

from jsonschema import Draft202012Validator

ROOT = pathlib.Path(__file__).resolve().parents[3]
TARGET = pathlib.Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
if not TARGET.is_absolute():
    TARGET = pathlib.Path.cwd() / TARGET
BIN = TARGET / 'debug/archy-mcp'
SOURCE = ROOT / 'examples/courtyard-housing/project.yaml'
META = {'io.modelcontextprotocol/protocolVersion': '2026-07-28',
        'io.modelcontextprotocol/clientInfo': {'name': 'mutation-test', 'version': '0'},
        'io.modelcontextprotocol/clientCapabilities': {}}
SCHEMAS = {name: Draft202012Validator(json.loads((ROOT / 'schemas/mcp' / f'{name}.schema.json').read_text())['output'])
           for name in ('generate_variants', 'evaluate_variant', 'get_project', 'compare_variants')}


def start(project):
    return subprocess.Popen([str(BIN), '--project', str(project)], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)


def frame(name, args, ident):
    return json.dumps({'jsonrpc': '2.0', 'id': ident, 'method': 'tools/call',
                       'params': {'name': name, 'arguments': args, '_meta': META}},
                      separators=(',', ':')).encode() + b'\n'


def replies(p, requests, timeout=15):
    p.stdin.write(b''.join(frame(*request) for request in requests))
    p.stdin.flush()
    out = {}
    for _ in requests:
        assert select.select([p.stdout], [], [], timeout)[0], 'MCP response timeout'
        answer = json.loads(p.stdout.readline())
        assert answer['id'] not in out, answer
        out[answer['id']] = answer
    return out


def call(p, name, args, ident, timeout=15):
    return result(name, replies(p, [(name, args, ident)], timeout)[ident])


def result(name, answer):
    assert 'result' in answer, answer
    response = answer['result']
    data = response['structuredContent']
    SCHEMAS[name].validate(data)
    assert json.loads(response['content'][0]['text']) == data
    assert response['isError'] == (not data['ok'])
    return data


def finish(p):
    p.stdin.close()
    p.wait(timeout=5)
    assert p.returncode == 0, p.stderr.read().decode()


def generate(key='generate-1', revision='0', destination='runs', seed='42', project_id='courtyard-demo'):
    return {'project_id': project_id, 'count': 2, 'seed': seed,
            'idempotency_key': key, 'expected_project_revision': revision,
            'destination': destination}


def evaluate(source_run, variant_id, key='evaluate-1', revision='1', destination='runs', project_id='courtyard-demo'):
    return {'project_id': project_id, 'source_run': source_run, 'variant_id': variant_id,
            'idempotency_key': key, 'expected_project_revision': revision,
            'destination': destination}


def error(data, code):
    assert data['ok'] is False and data['data'] is None, data
    assert data['error']['code'] == code, data


with tempfile.TemporaryDirectory(prefix='archy-mcp-mutate-') as td:
    root = pathlib.Path(td)
    project = root / 'project.yaml'
    shutil.copyfile(SOURCE, project)
    original = project.read_bytes()
    p = start(project)
    initial = replies(p, [('generate_variants', generate(), 1),
                          ('generate_variants', generate(), 2)])
    first = result('generate_variants', initial[1])
    duplicate = result('generate_variants', initial[2])
    assert first['ok'] and duplicate == first
    made = first['data']
    assert made['revision'] == '1' and len(made['variant_ids']) == 2
    assert pathlib.Path(td, made['path']).is_dir()
    assert call(p, 'generate_variants', generate(), 3) == first
    error(call(p, 'generate_variants', generate(seed='43'), 4), 'REVISION_CONFLICT')
    error(call(p, 'generate_variants', generate(key='stale', revision='0'), 5), 'REVISION_CONFLICT')
    error(call(p, 'generate_variants', generate(key='wrong', revision='1', project_id='other'), 6), 'PERMISSION_DENIED')
    for index, field in enumerate(('seed', 'expected_project_revision'), 30):
        invalid = {**generate(key=f'overflow-{index}', revision='1'), field: '18446744073709551616'}
        assert replies(p, [('generate_variants', invalid, index)])[index]['error']['code'] == -32602
    error(call(p, 'generate_variants', generate(key='max-seed', revision='0', seed='18446744073709551615'), 34), 'REVISION_CONFLICT')
    for index, destination in enumerate(('../escape', '/tmp/escape', '.archy/x', 'runs/../escape', 'runs//escape'), 7):
        error(call(p, 'generate_variants', generate(key=f'bad-{index}', revision='1', destination=destination), index), 'PERMISSION_DENIED')
    outside = root / 'outside'
    outside.mkdir()
    (root / 'link').symlink_to(outside, target_is_directory=True)
    error(call(p, 'generate_variants', generate(key='symlink', revision='1', destination='link'), 35), 'PERMISSION_DENIED')
    assert list(outside.iterdir()) == []
    evaluated = evaluate(made['run_id'], made['variant_ids'][0])
    error(call(p, 'evaluate_variant', evaluate(made['run_id'], made['variant_ids'][0],
                                              key='bad-eval-path', destination='link'), 36), 'PERMISSION_DENIED')
    assert list(outside.iterdir()) == []
    pair = replies(p, [('evaluate_variant', evaluated, 20), ('evaluate_variant', evaluated, 21)])
    evaluation = result('evaluate_variant', pair[20])
    assert evaluation['ok'] and result('evaluate_variant', pair[21]) == evaluation
    assert evaluation['data']['revision'] == '2'
    assert evaluation['data']['variant_ids'] == [made['variant_ids'][0]]
    historical = call(p, 'get_project', {'project_id': 'courtyard-demo', 'revision': '1'}, 42)
    assert historical['ok'] and historical['data']['revision'] == '1'
    assert historical['data']['source_run'] == made['run_id']
    compared = call(p, 'compare_variants', {'project_id': 'courtyard-demo',
                      'run_ids': [made['run_id']], 'objective': 'absolute_target_gfa_delta'}, 43)
    assert compared['ok'] and compared['data']['revision'] == '2'
    assert len(compared['data']['comparisons']) == 1
    assert compared['data']['comparisons'][0]['source_run_id'] == made['run_id']
    error(call(p, 'evaluate_variant', evaluate(made['run_id'], made['variant_ids'][1]), 22), 'REVISION_CONFLICT')
    error(call(p, 'evaluate_variant', evaluate(made['run_id'], made['variant_ids'][0], key='stale-eval'), 23), 'REVISION_CONFLICT')
    assert call(p, 'evaluate_variant', evaluated, 24) == evaluation
    staging_before = set((root / '.archy/staging').iterdir())
    error(call(p, 'evaluate_variant', evaluate(made['run_id'], 'variant-absent',
                                               key='bad-selector', revision='2'), 40), 'INVALID_SCHEMA')
    assert set((root / '.archy/staging').iterdir()) == staging_before
    assert len(list((root / '.archy/revisions').glob('*.json'))) == 2
    invalid_count = generate(key='bad-count', revision='2')
    invalid_count['count'] = 0
    assert replies(p, [('generate_variants', invalid_count, 41)])[41]['error']['code'] == -32602
    assert set((root / '.archy/staging').iterdir()) == staging_before
    finish(p)
    assert project.read_bytes() == original

    p = start(project)
    assert call(p, 'generate_variants', generate(), 25) == first
    assert call(p, 'evaluate_variant', evaluated, 26) == evaluation
    finish(p)

    revision = root / '.archy/revisions/00000000000000000002.json'
    revision.write_bytes(revision.read_bytes().replace(b'"revision":2', b'"revision":3'))
    p = start(project)
    error(call(p, 'generate_variants', generate(key='after-tamper', revision='2'), 27), 'ARTIFACT_INVALID')
    finish(p)
    assert len(list((root / '.archy/revisions').glob('*.json'))) == 2

with tempfile.TemporaryDirectory(prefix='archy-mcp-orphan-') as td:
    root = pathlib.Path(td)
    project = root / 'project.yaml'
    shutil.copyfile(SOURCE, project)
    p = start(project)
    original = call(p, 'generate_variants', generate(key='crash-window'), 1)
    assert original['ok']
    finish(p)
    # Model interruption after the run rename but before revision publication.
    # The run and manifest are produced by the real MCP transaction.
    next((root / '.archy/revisions').glob('*.json')).unlink()
    p = start(project)
    recovered = call(p, 'generate_variants', generate(key='crash-window'), 2)
    assert recovered == original
    assert len(list((root / 'runs').iterdir())) == 1
    assert len(list((root / '.archy/revisions').glob('*.json'))) == 1
    finish(p)

with tempfile.TemporaryDirectory(prefix='archy-mcp-duplicate-id-') as td:
    root = pathlib.Path(td)
    project = root / 'project.yaml'
    shutil.copyfile(SOURCE, project)
    p = start(project)
    made = call(p, 'generate_variants', generate(key='first'), 1)['data']
    finish(p)
    manifest_path = root / made['path'] / 'manifest.json'
    manifest = json.loads(manifest_path.read_bytes())
    manifest['outcomes'][1] = manifest['outcomes'][0]
    changed = json.dumps(manifest, separators=(',', ':')).encode()
    manifest_path.write_bytes(changed)
    revision_path = root / '.archy/revisions/00000000000000000001.json'
    record = json.loads(revision_path.read_bytes())
    record['receipt']['manifest_sha256'] = hashlib.sha256(changed).hexdigest()
    revision_path.write_text(json.dumps(record, separators=(',', ':')))
    p = start(project)
    error(call(p, 'generate_variants', generate(key='second', revision='1'), 2), 'ARTIFACT_INVALID')
    finish(p)
    assert len(list((root / '.archy/revisions').glob('*.json'))) == 1

with tempfile.TemporaryDirectory(prefix='archy-mcp-wrong-source-') as td:
    root = pathlib.Path(td)
    project = root / 'project.yaml'
    shutil.copyfile(SOURCE, project)
    p = start(project)
    old = call(p, 'generate_variants', generate(key='old-project'), 1)['data']
    finish(p)
    project.write_bytes(project.read_bytes().replace(b'project_id: courtyard-demo', b'project_id: alternate-demo'))
    cli = subprocess.run([str(TARGET / 'debug/archy'), 'generate', '--project', str(project),
                          '--count', '1', '--seed', '43', '--json'], capture_output=True)
    assert cli.returncode == 0, cli.stderr.decode()
    p = start(project)
    wrong = evaluate(old['run_id'], old['variant_ids'][0], key='wrong-source',
                     revision='2', project_id='alternate-demo')
    error(call(p, 'evaluate_variant', wrong, 2), 'ARTIFACT_INVALID')
    finish(p)
    assert len(list((root / '.archy/revisions').glob('*.json'))) == 2

for replace_root in (False, True):
    with tempfile.TemporaryDirectory(prefix='archy-mcp-replace-') as td:
        parent = pathlib.Path(td)
        root = parent / 'bound'
        root.mkdir()
        project = root / 'project.yaml'
        shutil.copyfile(SOURCE, project)
        p = start(project)
        assert replies(p, [('validate_project', {'project_id': 'courtyard-demo'}, 1)])[1]['result']['structuredContent']['ok']
        if replace_root:
            moved = parent / 'original'
            root.rename(moved)
            root.mkdir()
            shutil.copyfile(SOURCE, root / 'project.yaml')
        else:
            replacement = parent / 'replacement.yaml'
            shutil.copyfile(SOURCE, replacement)
            replacement.replace(project)
        error(call(p, 'generate_variants', generate(), 2), 'REVISION_CONFLICT')
        finish(p)
        assert not (root / '.archy').exists()
        if replace_root:
            assert not (moved / '.archy').exists()

for replace_root in (False, True):
    with tempfile.TemporaryDirectory(prefix='archy-mcp-swap-active-') as td:
        parent = pathlib.Path(td)
        root = parent / 'bound'
        root.mkdir()
        project = root / 'project.yaml'
        shutil.copyfile(SOURCE, project)
        p = start(project)
        request = generate(key='active-swap')
        request['count'] = 100
        p.stdin.write(frame('generate_variants', request, 1))
        p.stdin.flush()
        staging = root / '.archy/staging'
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline and not (staging.exists() and any(staging.iterdir())):
            assert not select.select([p.stdout], [], [], 0)[0], 'mutation finished before staging probe'
            time.sleep(0.001)
        assert staging.exists() and any(staging.iterdir()), 'staging phase not reached'
        if replace_root:
            moved = parent / 'moved'
            root.rename(moved)
            root.mkdir()
            shutil.copyfile(SOURCE, project)
            authoritative = moved
        else:
            replacement = parent / 'replacement.yaml'
            replacement.write_bytes(project.read_bytes().replace(b'project_id: courtyard-demo',
                                                                 b'project_id: alternate-demo'))
            replacement.replace(project)
            authoritative = root
        assert select.select([p.stdout], [], [], 60)[0]
        answer = result('generate_variants', json.loads(p.stdout.readline()))
        assert not answer['ok'] and answer['error']['code'] in ('REVISION_CONFLICT', 'INTERNAL_ERROR'), answer
        if replace_root:
            assert not (root / '.archy/revisions').exists()
        assert len(list((authoritative / '.archy/revisions').glob('*.json'))) <= 1
        finish(p)
        if replace_root:
            shutil.rmtree(root)
            moved.rename(root)
        else:
            project.write_bytes(SOURCE.read_bytes())
        p = start(project)
        retry = call(p, 'generate_variants', request, 2, timeout=60)
        assert retry['ok'], retry
        assert len(list((root / '.archy/revisions').glob('*.json'))) == 1
        assert call(p, 'generate_variants', request, 3, timeout=60) == retry
        finish(p)

with tempfile.TemporaryDirectory(prefix='archy-mcp-partial-history-') as td:
    root = pathlib.Path(td)
    project = root / 'project.yaml'
    shutil.copyfile(SOURCE, project)
    managed = root / '.archy'
    managed.mkdir(mode=0o700)
    p = start(project)
    error(call(p, 'generate_variants', generate(), 1), 'ARTIFACT_INVALID')
    finish(p)
    assert list(managed.iterdir()) == []

print('MCP live generate/evaluate/compare, historical read, replay, conflict, scope, tamper and replacement: PASS')
