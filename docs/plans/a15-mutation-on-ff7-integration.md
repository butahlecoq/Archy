# A15 mutation integration on ff7f6bc

The 8e36e11 reviewed-integration patch `e30ab43a9046eda1e74e4256c8b0de7d223f0b257702c64d616a3e866d68993d` was applied without conflict on ff7f6bc. All 15 source, patch and handoff hashes in `a15-mutation-on-8e-freeze.sha256` match. The later process fixture repair is preserved.

On this integration worktree, formatting, strict Clippy for core, CLI and MCP, locked offline core and MCP tests, and real MCP generate/evaluate/compare plus comparison stdio suites passed. Cargo warned that core and MCP examples share the output name `contracts` during the combined package test; schema execution invokes the packages separately. The full schema gate and independent integration review remain to be checked. An existing intermittent backpressure assertion in `parse_errors.py` is being independently repaired and is not included in this patch. Same-user uncooperative pathname replacement remains subject to the race described in the source handoff; the design does not claim a continuous path lease or human approval.

A separate `cargo test --locked --offline -p archy-cli` on the same integration tree also exited 0, including the long history-replay suite and golden artifacts; log `/home/butahlecoq/work/archy/.tools/a15-on-ff7-cli-tests.log`.

The integration tree's `python3 scripts/check-schemas.py` also exited 0, including all MCP protocol and mutation probes and six demo tests; log `/home/butahlecoq/work/archy/.tools/a15-on-ff7-schema.log`. The earlier recorded failure on the original pipe test remains evidence of a flake despite this successful run.
