"""Compare source-derived schemas and validate actual Rust serializer output."""
import json
import hashlib
import copy
import os
import subprocess
import sys
import uuid
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

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

for path in sorted((ROOT / 'schemas').glob('*.schema.json')):
    Draft202012Validator.check_schema(read(path))
    if path.name != 'doctor.schema.json':
        name = path.name.removesuffix('.schema.json')
        assert name in all_schemas, f'unregistered schema owned by another slice: {path.name}'
        assert read(path) == all_schemas[name], path
    print(f'schema valid: {path.name}')
for fixture, schema in [('courtyard.json', 'project'), ('courtyard.normalized.json', 'normalized-project')]:
    Draft202012Validator(read(ROOT / 'schemas' / f'{schema}.schema.json')).validate(read(ROOT / 'tests/fixtures/briefs' / fixture))
    print(f'fixture conforms: {fixture}')
for code in read(ROOT / 'schemas/error.schema.json')['$defs']['ErrorCode']['enum']:
    Draft202012Validator(read(ROOT / 'schemas/error.schema.json')).validate(dict(code=code, message='Fixture error', field='site.width', retriable=False))
print('all nine stable error code fixtures conform')
for key, schema in [('project', 'project'), ('normalized', 'normalized-project')]:
    Draft202012Validator(read(ROOT / 'schemas' / f'{schema}.schema.json')).validate(contracts[key])
assert contracts['normalized'] == read(ROOT / 'tests/fixtures/briefs/courtyard.normalized.json')
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
assert report == read(ROOT / 'crates/archy-report/tests/fixtures/report-contracts.json')
for name, schema in [('capabilities', 'desktop-capabilities'), ('status', 'desktop-status'),
                     ('accepted_status', 'desktop-status')]:
    value = desktop[name]
    Draft202012Validator(read(ROOT / 'schemas' / f'{schema}.schema.json')).validate(value)
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

status, value = cli(ROOT / 'examples/courtyard-housing/project.yaml')
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
    assert value == desktop[name], name
print('actual CLI desktop capabilities and genesis status conform')

subprocess.run(['python3', 'tests/decision_contract_schema.py'], cwd=ROOT, check=True)

demo_env = dict(os.environ, ARCHY_BIN=str(binary))
subprocess.run([sys.executable, '-m', 'unittest', 'discover', '-s', 'tests/demo', '-v'],
               cwd=ROOT, env=demo_env, check=True)
