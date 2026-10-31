"""Compare source-derived schemas and validate actual Rust serializer output."""
import json
import hashlib
import copy
import os
import subprocess
import sys
import uuid
import tempfile
from pathlib import Path
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]

contracts = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-core', '--example', 'contracts'],
    cwd=ROOT, text=True))
task02 = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-generate', '--example', 'task02_contracts'],
    cwd=ROOT, text=True))
storage = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-core', '--example', 'storage_contracts'],
    cwd=ROOT, text=True))
backend = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-adapters', '--example', 'backend_contracts'],
    cwd=ROOT, text=True))
report = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-report', '--example', 'report_contracts'],
    cwd=ROOT, text=True))
desktop = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-cli', '--example', 'desktop_contracts'],
    cwd=ROOT, text=True))
all_schemas = {**contracts['schemas'], **task02['schemas'], **storage['schemas'],
               **backend['schemas'], **report['schemas'], **desktop['schemas']}

mcp_schemas = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-mcp', '--example', 'contracts'],
    cwd=ROOT, text=True))
assert len(mcp_schemas) == 9
for name, pair in mcp_schemas.items():
    assert set(pair) == {'input', 'output'}
    for schema in pair.values():
        Draft202012Validator.check_schema(schema)
    assert pair['input']['additionalProperties'] is False
    assert len(pair['output']['oneOf']) == 2
    assert all(branch['additionalProperties'] is False for branch in pair['output']['oneOf'])
    read_path = ROOT / 'schemas' / 'mcp' / f'{name}.schema.json'
    assert json.loads(read_path.read_text()) == pair, name
    output = Draft202012Validator(pair['output'])
    for ok in (True, False):
        assert not output.is_valid({'ok': ok, 'data': None, 'error': None}), (name, ok)
    error = {'code': 'BACKEND_UNAVAILABLE', 'field': 'tool', 'message': 'Unavailable', 'retriable': False}
    assert output.is_valid({'ok': False, 'data': None, 'error': error}), name
    assert not output.is_valid({'ok': True, 'data': None, 'error': error}), name
assert {p.stem.removesuffix('.schema') for p in (ROOT / 'schemas' / 'mcp').glob('*.schema.json')} == set(mcp_schemas)
compare_input = Draft202012Validator(mcp_schemas['compare_variants']['input'])
compare_args = {'project_id': 'courtyard-demo', 'run_ids': ['x' * 128], 'objective': 'area'}
assert compare_input.is_valid(compare_args)
compare_args['run_ids'] = ['x' * 129]
assert not compare_input.is_valid(compare_args), 'run ID item length must match wire validation'
print('nine MCP input/output schema pairs match SDK descriptors')

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

for path in sorted((ROOT / 'schemas').glob('*.schema.json')):
    Draft202012Validator.check_schema(read(path))
    if path.name != 'doctor.schema.json':
        name = path.name.removesuffix('.schema.json')
        assert name in all_schemas, f'unregistered schema owned by another slice: {path.name}'
        assert read(path) == all_schemas[name], path
    print(f'schema valid: {path.name}')
for fixture, schema in [('courtyard-v2.json', 'project'), ('courtyard-v2.normalized.json', 'normalized-project')]:
    Draft202012Validator(read(ROOT / 'schemas' / f'{schema}.schema.json')).validate(read(ROOT / 'tests/fixtures/briefs' / fixture))
    print(f'fixture conforms: {fixture}')
for code in read(ROOT / 'schemas/error.schema.json')['$defs']['ErrorCode']['enum']:
    Draft202012Validator(read(ROOT / 'schemas/error.schema.json')).validate(dict(code=code, message='Fixture error', field='site.width', retriable=False))
print('all nine stable error code fixtures conform')
for key, schema in [('project', 'project'), ('normalized', 'normalized-project')]:
    Draft202012Validator(read(ROOT / 'schemas' / f'{schema}.schema.json')).validate(contracts[key])
assert contracts['normalized'] == read(ROOT / 'tests/fixtures/briefs/courtyard-v2.normalized.json')
for name in ('project', 'normalized-project'):
    archived = read(ROOT / 'schemas/archive/v1' / f'{name}.schema.json')
    assert archived == contracts['schemas'][f'{name}-v1']
    Draft202012Validator.check_schema(archived)
assert {'id', 'schema_version', 'site', 'brief', 'constraints'} <= set(contracts['normalized'])
assert {'id', 'kind', 'bound', 'unit', 'source', 'authority'} <= set(contracts['normalized']['constraints'][0])
assert 'project_id' not in contracts['normalized']
assert 'value_m' not in contracts['normalized']['constraints'][0]
project_schema = read(ROOT / 'schemas/project.schema.json')
normalized_schema = read(ROOT / 'schemas/normalized-project.schema.json')
assert project_schema['properties']['preferences']['maxItems'] == 8
assert project_schema['properties']['notes']['maxItems'] == 4
assert normalized_schema['properties']['preferences']['maxItems'] == 8
assert normalized_schema['properties']['notes']['maxItems'] == 4
assert normalized_schema['$defs']['MeterUnit']['enum'] == ['m']
assert {'id', 'kind', 'bound', 'unit', 'source', 'authority'} <= set(
    normalized_schema['$defs']['NormalizedConstraint']['required'])
for error in contracts['errors']:
    Draft202012Validator(read(ROOT / 'schemas/error.schema.json')).validate(error)
for name in ('generation', 'evaluation'):
    schema = read(ROOT / 'schemas' / f'{name}.schema.json')
    value = task02[name]
    Draft202012Validator(schema).validate(value)
    assert value == read(ROOT / 'tests/fixtures' / name / 'courtyard.seed42.json')
for variant in task02['generation']['variants']:
    Draft202012Validator(read(ROOT / 'schemas/variant.schema.json')).validate(variant)

def validate_storage_payload(payload):
    assert set(payload['serialized']) == {'run-request', 'run-manifest', 'revision'}
    values = {}
    for name, raw in payload['serialized'].items():
        value = json.loads(raw)
        Draft202012Validator(read(ROOT / 'schemas' / f'{name}.schema.json')).validate(value)
        values[name] = value
    request, manifest, revision = (values[name] for name in ('run-request', 'run-manifest', 'revision'))
    receipt = payload['receipt']
    assert revision['receipt'] == receipt
    assert manifest['run_id'] == receipt['run_id']
    assert uuid.UUID(receipt['run_id']).version == 4
    assert receipt['path'] == f"runs/{receipt['run_id']}"
    assert receipt['path'] == f"{request['context']['destination']}/{receipt['run_id']}", 'request destination binding'
    assert revision['revision'] == receipt['revision'] == 1
    assert hashlib.sha256(payload['serialized']['run-manifest'].encode()).hexdigest() == receipt['manifest_sha256']
    assert request['schema_version'] == manifest['request_schema_version'] == revision['request_schema_version'] == 'archy-run-request-v1'
    assert manifest['schema_version'] == 'archy-run-v1'
    assert revision['schema_version'] == 'archy-revision-v1'
    assert request['context']['idempotency_key'] == manifest['idempotency_key'] == revision['idempotency_key']
    assert request['context']['expected_project_revision'] == manifest['expected_project_revision'] == 0
    assert request['operation'] == manifest['operation'] == 'storage-schema-fixture'
    request_fingerprint = hashlib.sha256(b'archy-run-request-fingerprint-v1\0' +
                                         payload['serialized']['run-request'].encode('utf-8')).hexdigest()
    assert manifest['request_sha256'] == revision['request_sha256'] == request_fingerprint, 'request fingerprint'
    source = b'storage schema fixture\n'
    source_digest = hashlib.sha256(source).hexdigest()
    assert request['input_sha256'] == manifest['input_sha256'] == source_digest
    assert manifest['artifacts'] == [dict(path='input.txt', sha256=source_digest,
                                           media_type='text/plain', producer='storage-contracts', byte_len=len(source))]
    assert manifest['outcomes'] == []

validate_storage_payload(storage)
saved_storage = read(ROOT / 'tests/fixtures/storage/transaction.json')
validate_storage_payload(saved_storage)

def probe_storage_relationships(payload):
    altered = copy.deepcopy(payload)
    request = json.loads(altered['serialized']['run-request'])
    request['request_bytes'][0] = (request['request_bytes'][0] + 1) % 256
    altered['serialized']['run-request'] = json.dumps(request, separators=(',', ':'))
    try:
        validate_storage_payload(altered)
    except AssertionError as exc:
        assert str(exc) == 'request fingerprint', exc
    else:
        raise AssertionError('tampered request bytes accepted')

    altered = copy.deepcopy(payload)
    request = json.loads(altered['serialized']['run-request'])
    request['context']['destination'] = 'different-runs'
    altered['serialized']['run-request'] = json.dumps(request, separators=(',', ':'))
    fingerprint = hashlib.sha256(b'archy-run-request-fingerprint-v1\0' +
                                 altered['serialized']['run-request'].encode('utf-8')).hexdigest()
    manifest = json.loads(altered['serialized']['run-manifest'])
    revision = json.loads(altered['serialized']['revision'])
    manifest['request_sha256'] = revision['request_sha256'] = fingerprint
    altered['serialized']['run-manifest'] = json.dumps(manifest, separators=(',', ':'))
    altered['receipt']['manifest_sha256'] = hashlib.sha256(
        altered['serialized']['run-manifest'].encode('utf-8')).hexdigest()
    revision['receipt'] = altered['receipt']
    altered['serialized']['revision'] = json.dumps(revision, separators=(',', ':'))
    try:
        validate_storage_payload(altered)
    except AssertionError as exc:
        assert str(exc) == 'request destination binding', exc
    else:
        raise AssertionError('tampered request destination accepted')

probe_storage_relationships(storage)
probe_storage_relationships(saved_storage)

backend_schema = read(ROOT / 'schemas/backend.schema.json')
for name, wire_type, response_type, script in [
    ('blender', 'BlenderWire', 'BlenderResponse',
     "import sys; sys.path.insert(0,'workers/blender'); import protocol; protocol.parse_request(sys.stdin.buffer.read())"),
    ('ifc', 'IfcWire', 'IfcResponse',
     "import sys; sys.path.insert(0,'workers/ifc'); from archy_ifc_worker import protocol; protocol.parse_request(sys.stdin.buffer.read())")]:
    raw = backend['serialized'][name]
    assert raw == (ROOT / 'tests/fixtures/backend' / f'{name}-request.json').read_text().rstrip('\n')
    sub_schema = {'$schema': backend_schema['$schema'], '$defs': backend_schema['$defs'],
                  '$ref': f'#/$defs/{wire_type}'}
    Draft202012Validator(sub_schema).validate(json.loads(raw))
    subprocess.run(['python3', '-c', script], input=raw.encode(), cwd=ROOT,
                   capture_output=True, check=True)
    response_schema = {'$schema': backend_schema['$schema'], '$defs': backend_schema['$defs'],
                       '$ref': f'#/$defs/{response_type}'}
    Draft202012Validator(response_schema).validate(
        read(ROOT / 'tests/fixtures/backend' / f'{name}-response.json'))

report_schema = read(ROOT / 'schemas/report.schema.json')
Draft202012Validator(report_schema).validate(report['report'])
assert report == read(ROOT / 'crates/archy-report/tests/fixtures/report-contracts-v2.json')
old_report = read(ROOT / 'schemas/archive/v1/report.schema.json')
Draft202012Validator.check_schema(old_report)
assert hashlib.sha256((ROOT / 'schemas/archive/v1/report.schema.json').read_bytes()).hexdigest() == '1ac061bf8c854800c07bfc64a4f928893b3507f0a2b90f82d37d7a77b135cc59'
legacy_report = read(ROOT / 'tests/fixtures/legacy-preference-run/archived-runs/5248af90-8dfe-4743-b056-7d3385c62c97/report.json')
Draft202012Validator(old_report).validate(legacy_report)
assert {'schema_version', 'context', 'spec', 'variants', 'uncomputed', 'professional_limits', 'preference_status'} <= set(report['report'])
assert report['report']['schema_version'] == 'archy-report-v2'
current_v1_fixture = read(ROOT / 'crates/archy-report/tests/fixtures/report-contracts.json')
assert report['schemas']['report-v1'] == current_v1_fixture['schemas']['report']
assert report['report_v1'] == current_v1_fixture['report']

for name, schema in [('capabilities', 'desktop-capabilities'), ('status', 'desktop-status'),
                     ('accepted_status', 'desktop-status')]:
    value = desktop[name]
    Draft202012Validator(read(ROOT / 'schemas' / f'{schema}.schema.json')).validate(value)
    if name != 'capabilities':
        assert value == read(ROOT / 'tests/fixtures/desktop' / f'{name}.json')
print('source schemas match; actual Rust project/normalized/error/generation/variant/evaluation/storage/backend/report serializers conform')

subprocess.run(['cargo', 'build', '--locked', '-p', 'archy-cli'], cwd=ROOT, check=True)
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'], cwd=ROOT, text=True))
binary = Path(metadata['target_directory']) / 'debug' / ('archy.exe' if os.name == 'nt' else 'archy')
def cli(path):
    result = subprocess.run([str(binary), 'validate', '--json', '--project', str(path)],
                            capture_output=True, text=True, check=False)
    assert not result.stderr, result.stderr
    return result.returncode, json.loads(result.stdout)

project_validator = Draft202012Validator(project_schema)
base_project = read(ROOT / 'tests/fixtures/briefs/courtyard-v2.json')
legitimate = copy.deepcopy(base_project)
legitimate['preferences'] = [
    {'kind': 'massing_character', 'value': 'low_rise'},
    {'kind': 'architectural_intent', 'value': 'warm brick facade'},
    {'kind': 'landscape_intent', 'value': 'shaded garden'},
]
assert project_validator.is_valid(legitimate)
with tempfile.TemporaryDirectory(prefix='archy-a02-legitimate-') as temp:
    path = Path(temp) / 'project.json'
    path.write_text(json.dumps(legitimate))
    status, normalized_legitimate = cli(path)
assert status == 0 and normalized_legitimate['preferences'] == legitimate['preferences']
def probe(mutator, field, schema_rejects=True):
    value = copy.deepcopy(base_project)
    mutator(value)
    assert bool(list(project_validator.iter_errors(value))) == schema_rejects, (field, value)
    with tempfile.TemporaryDirectory(prefix='archy-a02-schema-') as temp:
        path = Path(temp) / 'project.json'
        path.write_text(json.dumps(value))
        status, error = cli(path)
    assert status != 0 and error['field'] == field, (field, error)

# Local, representable rules must agree with the actual parser.
probe(lambda p: p.update(id='a'*129), 'id')
probe(lambda p: p['constraints'][0].update(bound=-1), 'constraints[0].bound')
probe(lambda p: p['brief'].update(storeys=0), 'brief.storeys')
probe(lambda p: p['generation'].update(count=1001), 'generation.count')
probe(lambda p: p['generation'].update(shapes=[]), 'generation.shapes')
probe(lambda p: p.update(preferences=p['preferences']*5), 'preferences')
probe(lambda p: p['preferences'][0].update(value='<script>alert(1)</script>'), 'preferences[0].value')
probe(lambda p: p['preferences'][0].update(value='quiet\u0085courtyard'), 'preferences[0].value')
probe(lambda p: p['preferences'][0].update(kind='execute'), 'preferences[0].kind')
probe(lambda p: p['preferences'][0].update(value='x'*257), 'preferences[0].value')
probe(lambda p: p.update(notes=['x'*513]), 'notes[0]')
probe(lambda p: p['constraints'][0].update(id='site_containment'), 'constraints[0].id')
probe(lambda p: p['site'].update(unknown_dimension=1), 'site.unknown_dimension')
probe(lambda p: p.update(schema_version='0.1.0'), 'schema_version')

# UTF-8 byte totals, arithmetic overflow, and geometric relationships cannot be
# expressed faithfully by Draft 2020-12 local schema keywords.
probe(lambda p: p.update(notes=['é'*300]), 'notes[0]', schema_rejects=False)
probe(lambda p: p.update(notes=['a'*400]*3), 'notes', schema_rejects=False)
probe(lambda p: p['preferences'][0].update(value='🎨'*200), 'preferences[0].value', schema_rejects=False)
probe(lambda p: p['brief'].update(floor_to_floor_m=1e308, storeys=4294967295), 'brief.floor_to_floor_m', schema_rejects=False)

status, value = cli(ROOT / 'examples/courtyard-housing/project-v2.yaml')
assert status == 0 and value == contracts['normalized']
Draft202012Validator(read(ROOT / 'schemas/normalized-project.schema.json')).validate(value)
cases = read(ROOT / 'tests/fixtures/briefs/negative-cases.json')
for case in cases:
    status, error = cli(ROOT / 'tests/fixtures/briefs' / case['file'])
    assert status != 0 and error['code'] == case['code'], (case, error)
    if case['field'] is not None:
        assert error['field'] == case['field'], (case, error)
    Draft202012Validator(read(ROOT / 'schemas/error.schema.json')).validate(error)
print(f'actual CLI: golden normalized output and all {len(cases)} negative fixtures conform')

doctor = subprocess.run([str(binary), 'doctor', '--json'], cwd=ROOT,
                        capture_output=True, text=True, timeout=15, check=False)
assert doctor.returncode in (0, 1) and not doctor.stderr, (doctor.returncode, doctor.stderr)
doctor_report = json.loads(doctor.stdout)
Draft202012Validator(read(ROOT / 'schemas/doctor.schema.json')).validate(doctor_report)
assert doctor_report['target_acceptance'] == 'deferred'
print(f'actual CLI doctor report conforms (exit {doctor.returncode}; target acceptance deferred)')

for name, args, schema in [
    ('capabilities', ['desktop', 'capabilities', '--json'], 'desktop-capabilities'),
    ('status', ['desktop', 'status', '--project', str(ROOT / 'examples/courtyard-housing/project.yaml'), '--json'], 'desktop-status'),
]:
    result = subprocess.run([str(binary), *args], cwd=ROOT, capture_output=True, text=True, check=False)
    assert result.returncode == 0 and not result.stderr, (name, result.returncode, result.stderr)
    value = json.loads(result.stdout)
    Draft202012Validator(read(ROOT / 'schemas' / f'{schema}.schema.json')).validate(value)
    if name != 'capabilities':
        assert value == desktop[name], name
    else:
        assert [x['id'] for x in value['operations']] == ['project_status', 'run_status', 'open_report', 'open_preview', 'open_blender']
        assert value['operations'][0]['available'] and value['operations'][1]['available']
print('actual CLI desktop capabilities and genesis status conform')

subprocess.run(['python3', 'tests/decision_contract_schema.py'], cwd=ROOT, check=True)
subprocess.run(['cargo', 'build', '--locked', '-p', 'archy-mcp'], cwd=ROOT, check=True)
for test in ('stdio_contract.py', 'parse_errors.py', 'compare_stdio.py', 'mutations.py', 'render_variant.py', 'export_ifc.py', 'propose_approval.py'):
    subprocess.run([sys.executable, str(ROOT / 'crates' / 'archy-mcp' / 'tests' / test)],
                   cwd=ROOT, timeout=360 if test == 'mutations.py' else 120, check=True)

demo_env = dict(os.environ, ARCHY_BIN=str(binary))
subprocess.run([sys.executable, '-m', 'unittest', 'discover', '-s', 'tests/demo', '-v'],
               cwd=ROOT, env=demo_env, check=True)
