# A14/A16 human-bus integration candidate, 30 October 2026

## Plan and base

Start at published `a9c91d4da8b37c37f82232f5cc93cb62c8ab884f`, port the frozen candidate from `.worktrees/a14-a16-bus-confirm-20261030-unique` on `1e2313fe0bf20f69f246836334adb761276d2390`, reconcile current v2 and historical verification, run focused and integration checks, then carry the commit onto published `c72f5309a681f36f56cb05e3932486bb7410a3f0`. The latter adds only twelve A23 harness/playbook files. This is an isolated branch and worktree; nothing is pushed or installed.

## Port and conflicts

- Kept the published CLI verifier, canonical v2/profile handling, desktop and MCP files. Added its public `verify_history` export. Ported only the bounded accepted-history reader into current core storage. The candidate's old CLI export patch needed a manual location fix; no source merge conflict remains.
- Added the protected writer, bus identity, review endpoint, snapshot worker, fixed binaries and their focused tests. Regenerated `Cargo.lock` against the published workspace. Vendored exact zbus 5.19.0 with the candidate's receive limit, FD bound and atomic close-on-exec corrections. The bus binary and ingest binary are not package staged.
- Changed protected initialization to the current parser, which accepts v1 and v2 sources. Snapshot evidence now uses the verified accepted `normalized.json` artifact bytes. On the current runner, deriving those bytes from the numeric v1 view produced a different digest for canonical v2 runs. Added a full v2 protected snapshot test and adapted the private-bus proposal fixture to accepted artifact bytes.
- Copied the historical candidate handoffs and slice manifests as provenance. Their checksums are superseded where later candidate stages or this integration changed files. The new integration manifest freezes the final changed paths. The historical zbus patch's required space-prefixed context lines use a path-specific whitespace rule.

## Security and deployment gates

`Confirm` and both seqpacket `OP_DECIDE` routes remain unconditionally denied. There is no approval lease, trusted human display and affirmative action, fresh non-retained Polkit check, journal decision append or approved projection. No target/security or human approval success is asserted.

The independent combined review reports a session freshness gap after snapshot work and finite daemon recovery under repeated oversized traffic. The six-file follow-on patch from `5a7f2e774d66854669bf6c2d78ed0cfc65896a90` (SHA-256 `483039964b2c2be1a7152a52383bba57483d39d44f49046d7a6d7a1cf0f66319`) is under separate review and is not grafted here. Native distinct-UID bus policy, logind, mount-race and Omarchy tests, installed units, notices, deployment policy and sustained abuse remain open. The separate operating witness candidate `99aaed2d56d6f497d89c39c83de53523419fd951` received a scoped independent security PASS under its documented trust model and is being ported separately. It is not grafted here. Later reconciliation requires a root-owned journal parent; the witness cannot prove removal of records never extended into it and does not approve a human decision.

## Validation

All Cargo commands use Rust 1.99.1, `--locked --offline`, `TMPDIR=/home/butahlecoq/work/archy`, and unique ext4 `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/a14-a16-bus-a9-target-20261030-codex`.

- `cargo +1.99.1 test --locked --offline -p archy-authority -p archy-core -p archy-cli -p archy-mcp`: exit 0. The 64-run historical CLI test passed in 253.89 seconds. The private-bus, protected writer including v2, storage, accepted worker, desktop, golden artifacts, validation and MCP tests passed.
- `cargo +1.99.1 clippy --locked --offline -p archy-authority --all-targets -- -D warnings`: exit 0.
- `cargo +1.99.1 fmt --all -- --check`, `rustfmt +1.99.1 --edition 2024 --check authority/vendor/zbus-5.19.0/src/connection/socket/unix.rs`, and `python3 authority/tests/disposable_bus.py`: exit 0; the latter reported ProcessFD. `git diff --cached --check` passed with the patch-specific whitespace attribute.
- `CARGO_NET_OFFLINE=true python3 scripts/check-schemas.py` with the same Cargo target and temp directory: exit 0. The v2 source and normalized fixtures, nine MCP schema pairs, actual Rust serializers, 26 CLI negative fixtures, desktop contracts and six demo tests passed. Doctor reported exit 1 with target acceptance deferred.
