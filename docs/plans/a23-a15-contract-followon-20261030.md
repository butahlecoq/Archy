# A23 and A15 contract reconciliation

Base: `9eeb99f2b6bfc2d45f04b0abdb372248760e6855`.

1. Update the existing A23 tool contract check to retain the unavailable proposal guard and validate the live IFC input and output schema, plus the operator guidance for draft status and missing backends.
2. Correct the Archy agent skill and Omarchy playbook to describe the selected IFC export, transaction fields, fixed service gate, and draft-only receipt. Preserve the human approval boundary.
3. Compare the collected A23 test names with the base, run all 27 tests and all MCP schema and stdio checks, then check formatting and the diff. Keep the pinned acceptance matrix and integrity guards unchanged. No dependencies are added.

Review focus: a live source contract does not prove an installed backend or native target acceptance; unavailable export must remain an error, and a draft IFC cannot grant approval.

## Verification and handoff

- The revised `test_tool_and_schema_contract` first failed because the old skill did not describe `DRAFT`; it passed after both guides were corrected. It preserves the proposal's unavailable guard and adds the required IFC fields and receipt checks.
- `PYTHONDONTWRITEBYTECODE=1 unshare --user --map-current-user --pid --fork --mount-proc python3 -B -m unittest discover -s tests/acceptance/omarchy -p 'test_*.py' -v`: 27 passed. The private PID namespace contains the temporary GPG agents used by the signed-review fixtures.
- An AST comparison with `9eeb99f` found the same 27 A23 test methods: 26 unchanged and `test_tool_and_schema_contract` updated. No test method was removed or renamed.
- `cargo build --locked -p archy-mcp -p archy-cli`, `cargo test --locked -p archy-mcp`, and `python3 -B scripts/check-schemas.py`: exit 0. The schema checker compared all nine MCP input/output schema pairs with SDK descriptors, ran all six MCP stdio scripts, and reported the actual absent IFC backend without a success receipt. The MCP Rust suite passed six bounds and three framing tests.
- `node crates/archy-mcp/tests/unicode_regex.js`, `cargo fmt --all -- --check`, and `git diff --check`: exit 0. Rust and schema commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a23-a15-contract-followon-20261030`, `CARGO_BUILD_JOBS=2`, and `PYTHONDONTWRITEBYTECODE=1` where relevant.
- Logs are under `/home/butahlecoq/work/archy/.tools/a23-a15-contract-followon-20261030-evidence`; the SHA-256 of `SHA256SUMS` is `31234694b296f530697106677c4a347342957203bc7e57f563791e7df269f541`.

No installed successful IFC export or native target acceptance was exercised. No separate agent pressure scenario was run because this session has no subagent tool; request independent review of the updated skill and test contract. The pending runtime candidate overlaps `crates/archy-cli/src/export_ifc.rs` and must preserve both implementation changes when integrated.
