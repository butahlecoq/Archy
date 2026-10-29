# Decision contract integration

The independently reviewed A14 contract was merged with the historical v1
normalization boundary on top of `5bfeca9`. The only shared source merge was
the core module export: it retains the v1 parser export and adds the decision
contract and schema exports. The isolated integration worktree reproduced the
reviewed A02 and A14 source identities and tested the combined tree.

On this integration tree, `cargo fmt --all -- --check`, strict workspace
Clippy, the focused core and evaluator decision tests, report tests, golden
artifact test, `tests/decision_contract_schema.py`, and
`scripts/check-schemas.py` passed. Logs are under
`.tools/a14-integration-5bfeca9-logs` in the workspace wrapper.
The regular schema checker now invokes the decision schema/serde test; the
registered checker passed and printed all 50 boundary cases. The integration
copy matched all 14 entries in the isolated A14 merge hash manifest.

The contract expresses untrusted proposals and decision claims. Matching a
digest and required-check inventory does not authenticate the source or
prove an evaluation was recomputed; the independent review demonstrates that
a rehashed forged result can still create a passing claim. No human approval,
decision persistence, production broker, or A14 acceptance is established.
