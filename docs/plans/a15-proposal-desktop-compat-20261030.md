# A15 proposal desktop compatibility

Base: `fa1546242ccb77518b851cca675e496fd6dd94a3`. Preserve verified, read-only desktop status across accepted proposal runs, including pagination and explicit selection. Make the schema, plugin validator and presentation agree on the `propose` operation and avoid implying a human decision or that the proposal itself evaluated the design. Verify the historical v1 claimed-regulation restriction and make the agent instructions explicit. No dependencies or authority services are added.

Run the live MCP-to-CLI-to-desktop-schema/plugin regression, existing desktop reader and plugin tests, proposal protocol, focused format/clippy checks, and the 27 A23 contracts using a private `TMPDIR` under the workspace. Freeze the result for independent review; the A15 transaction review remains separate.

## Handoff

The baseline live regression created a proposal, then `archy desktop status` returned `ARTIFACT_INVALID` / `unsupported run operation`. The desktop reader now summarizes the verified `propose` run; its schema and plugin validator admit the operation while requiring one selected variant and a false `all_required_checks_passed` value. The plugin presents it as a review proposal rather than describing that conservative value as a failed design check. The live test validates the CLI output against the schema and plugin model, checks pagination and selection, and confirms that desktop reads leave project files unchanged.

The proposal test also copies a committed run into a disposable project, removes the last revision record to model an orphan, and alters the proposal reason while rehashing its manifest. Recovery refuses the mismatch before publishing a revision; restoring the original bytes permits recovery and exact replay. This simulates the post-publication state rather than injecting a storage fault. An authentic historical v1 claimed-regulation fixture rejects comparison and proposal without writes; the existing linked-edit migration test confirms the v2 authority profile retains `UNKNOWN`.

With `TMPDIR=/home/butahlecoq/work/archy/.tools/tmp-a15-proposal-desktop-compat-fa15462-20261030`:

- `cargo test --locked --offline -p archy-cli --test desktop_readers`: 8 passed.
- `cargo test --locked --offline -p archy-cli --test golden_artifacts new_children_of_authentic_claimed_v1_history_migrate_authority_profile`: 1 passed.
- `python3 crates/archy-mcp/tests/propose_approval.py`: passed, including desktop, orphan recovery, replay, conflicts, nonpassing checks and historical v1 refusal.
- `node --test tests/omarchy/integration/desktop_model.test.js`: 12 passed; desktop status schema tests: 5 passed; plugin contract tests: 4 passed.
- `PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest discover -s tests/acceptance/omarchy -p 'test_*.py' -q`: 27 passed.
- `cargo fmt --all -- --check` and `cargo clippy --locked --offline -p archy-cli --all-targets -- -D warnings`: passed.

The existing desktop operation match also lacks `export-ifc`; this separate pre-existing case was not changed or exercised here. Native Omarchy and an installed IFC verifier were not exercised. No dependency, authority service, target acceptance, or human decision was added. Independent review of this follow-up remains outstanding.
