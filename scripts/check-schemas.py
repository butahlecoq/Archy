"""Compare source-derived schemas and validate actual Rust serializer output."""
import json
import os
import subprocess
from pathlib import Path
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]

contracts = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-core', '--example', 'contracts'],
    cwd=ROOT, text=True))
task02 = json.loads(subprocess.check_output(
    ['cargo', 'run', '--locked', '--quiet', '-p', 'archy-generate', '--example', 'task02_contracts'],
    cwd=ROOT, text=True))
all_schemas = {**contracts['schemas'], **task02['schemas']}
def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

for path in sorted((ROOT / 'schemas').glob('*.schema.json')):
    Draft202012Validator.check_schema(read(path))
    assert read(path) == all_schemas[path.name.removesuffix('.schema.json')], path
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
    value = task02[name]
    Draft202012Validator(read(ROOT / 'schemas' / f'{name}.schema.json')).validate(value)
    assert value == read(ROOT / 'tests/fixtures' / name / 'courtyard.seed42.json')
for variant in task02['generation']['variants']:
    Draft202012Validator(read(ROOT / 'schemas/variant.schema.json')).validate(variant)
print('source schemas match; actual Rust project/normalized/error/generation/variant/evaluation serializers conform')

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
