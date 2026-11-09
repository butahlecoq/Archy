# A15 proposal transaction review handoff

Base: published `8e6c8987431a3c613563345ee38f95f530181b12`. Candidate is confined to the new A15 proposal implementation worktree. This handoff does not claim independent review, human approval, service activation, or Omarchy acceptance.

The new `BoundProjectRead::propose` transaction stores a typed A14 `Proposal` as a `propose` accepted run. It selects one verified generation/edit source and variant, recomputes its evaluation and comparison, and binds those bytes plus constraint provenance to the project revision and head that existed immediately before proposal publication. The selected source may be older than that head. The verifier rechecks both histories, exact canonical artifacts and fixed inventory. It returns no variants for the proposal run and creates no actor, decision, approval state, or authority-service call. Nonpassing checks remain in the proposal for visible review and never imply approval.

The storage transaction uses explicit project-relative destination, key, and expected revision, with the core lock and checked orphan validation before recovered publication. Exact replay verifies the historical proposal and its original pre-proposal head, including after later revisions. A changed request with the same key conflicts. The MCP receipt names the proposal run, revision, artifact path/hash, selected source and variant. There is no final approval capability.

Focused protocol coverage creates a nonpassing v2 candidate, inserts an evaluation run between source and proposal, checks the pre-proposal revision hash, acceptance and historical replay, two queued same-key calls, changed-request and stale conflicts, malformed/forbidden fields, path scope, tamper refusal, source-byte swap, and no approved artifact. `scripts/check-schemas.py` includes this live test. The A23 skill/playbook contract was updated for the proposal receipt without changing its pinned 27-gate matrix.

Checks on the candidate:

- `cargo test --locked --offline -p archy-cli --lib`: 22 passed.
- `cargo test --locked --offline -p archy-core --test storage --test decision_contract`: 21 storage and 5 A14 contract tests passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --offline -p archy-core -p archy-cli -p archy-mcp --all-targets -- -D warnings`: passed.
- `PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest discover -s tests/acceptance/omarchy -p 'test_*.py' -q`: 27 passed.
- `python3 scripts/validate-omarchy.py --dry-run`: 27 gates `deferred`; target acceptance `deferred`.
- `python3 crates/archy-mcp/tests/propose_approval.py`: passed.
- `python3 scripts/check-schemas.py`: exit 0; all nine live MCP schema pairs, proposal protocol, existing mutation/render/IFC contracts, 12 demo tests, and source schema/fixture checks passed.
- `git diff --check`: passed.

Review focus: semantic orphan recovery has the same checked callback as the existing storage transaction, but this candidate has no fault-injected proposal-specific orphan fixture. The local accepted chain has no independent trust anchor and cannot authorize a human decision. No host service or target backend was exercised.
