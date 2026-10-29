"""Validate schema syntax and curated JSON fixtures, independently of Rust.

This does not execute the Rust parser or prove runtime/schema equivalence.
"""
import json
from pathlib import Path
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

for path in sorted((ROOT / 'schemas').glob('*.schema.json')):
    Draft202012Validator.check_schema(read(path))
    print(f'schema valid: {path.name}')
for fixture, schema in [('courtyard.json', 'project'), ('courtyard.normalized.json', 'normalized-project')]:
    Draft202012Validator(read(ROOT / 'schemas' / f'{schema}.schema.json')).validate(read(ROOT / 'tests/fixtures/briefs' / fixture))
    print(f'fixture conforms: {fixture}')
for code in read(ROOT / 'schemas/error.schema.json')['properties']['code']['enum']:
    Draft202012Validator(read(ROOT / 'schemas/error.schema.json')).validate(dict(code=code, message='Fixture error', field='site.width', retriable=False))
print('all nine stable error code fixtures conform')
