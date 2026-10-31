"""Exercise the generated schemas and the actual Rust serde/domain boundary."""
import copy
import json
import subprocess
from pathlib import Path

from jsonschema import Draft202012Validator

root = Path(__file__).resolve().parents[1]
digest = "a" * 64
evidence = dict(project_id="courtyard", variant_id="variant-" + digest,
                source_run_id="00000000-0000-4000-8000-000000000000",
                expected_project_revision=1, expected_head_sha256=digest,
                input_sha256=digest, normalized_sha256=digest, variant_sha256=digest,
                evaluation_sha256=digest, comparison_sha256=digest, constraints=[],
                checks=[dict(constraint_id="site_containment", result="PASS", measured=2.0,
                             limit=0.0, explanation="site", required=True)])
proposal = dict(schema_version="archy-proposal-v1", evidence=evidence, suggested_reason="review")
decision = dict(schema_version="archy-decision-record-v1", evidence=evidence,
                actor=dict(principal="alice", mechanism="claim"),
                timestamp=dict(seconds=1, nanos=0), decision="select", reason="review")
cases = []

def add(kind, value, expected):
    cases.append((kind, value, expected))

for kind, base in [("proposal", proposal), ("decision", decision)]:
    add(kind, copy.deepcopy(base), True)
    reason = "suggested_reason" if kind == "proposal" else "reason"
    for value, expected in [(" ", False), ("\u200b", False), ("\u200b\u200d", False),
                            ("\u200e\u2066", False), ("\u0301", False),
                            ("工程 🏠", True), ("e\u0301", True),
                            ("é" * 4096, True), ("é" * 4097, False)]:
        item = copy.deepcopy(base)
        item[reason] = value
        add(kind, item, expected)
    for field, value in [("project_id", "../project"), ("project_id", "project\n"),
                         ("project_id", "p" * 129), ("variant_id", "variant-abc"),
                         ("source_run_id", "../run"), ("expected_project_revision", 0),
                         ("variant_sha256", "bad"), ("evaluation_sha256", "A" * 64)]:
        item = copy.deepcopy(base)
        item["evidence"][field] = value
        add(kind, item, False)
    for revision, expected in [(1, True), (2**64 - 1, True), (2**64, False)]:
        item = copy.deepcopy(base)
        item["evidence"]["expected_project_revision"] = revision
        add(kind, item, expected)
    item = copy.deepcopy(base)
    item["schema_version"] += "\n"
    add(kind, item, False)

item = copy.deepcopy(proposal)
item["actor"] = "human"
add("proposal", item, False)
item = copy.deepcopy(decision)
item["timestamp"]["nanos"] = 1_000_000_000
add("decision", item, False)
for seconds, expected in [(-1, False), (0, True), (2**64 - 1, True), (2**64, False)]:
    item = copy.deepcopy(decision)
    item["timestamp"]["seconds"] = seconds
    add("decision", item, expected)

schemas = {kind: Draft202012Validator(json.loads((root / "schemas" / f"{name}.schema.json").read_text()))
           for kind, name in [("proposal", "proposal"), ("decision", "decision-record")]}
results = json.loads(subprocess.check_output(
    ["cargo", "run", "--locked", "--offline", "--quiet", "-p", "archy-core", "--example", "decision_contract_probe"],
    cwd=root, input=json.dumps([dict(kind=k, value=v) for k, v, _ in cases]), text=True))
for (kind, value, expected), actual in zip(cases, results, strict=True):
    assert actual == expected, (kind, value, "serde/domain", actual, expected)
    schema_result = schemas[kind].is_valid(value)
    assert schema_result == expected, (kind, value, "schema", schema_result, expected)
print(f"{len(cases)} serde/domain and schema cases passed")
