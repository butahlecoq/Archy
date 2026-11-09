# A14 authority, witness and notices on published A12/A18

## Review target

The tested source merge is `22c92d1393edbdd846f6bc1f44cb3bd26523b873`, with parents `a1e5b3807082848a527ef113c3707369a4ad02fd` and `edcf7f51f4b8963be845df344869184c05531bdc`. Its tree is `527203d95055f207c3abf1cd0604e5a2a7ce8cba`, exactly the reported clean merge tree. Worktree: `/home/butahlecoq/work/archy/.t/a14-a1e5-integration-20261030`. This handoff is a subsequent documentation-only commit; review the merge and this handoff together.

Plan: preserve both parents' source and test behavior; inspect the overlapping history and recovery paths; run focused recovery, authority, witness, packaging, demo and strict lint checks; hand the frozen result to independent integration review. No push or service installation was performed.

The staged merge changed only three existing CLI/core files: `lib.rs` exports `verify_history`, `run_integrity.rs` makes that existing verifier public, and `storage.rs` adds A14 bounded history reading. The published base retains 5d's semantic orphan validation before revision publication, its post-boundary artifact recheck, and the ff14-equivalent `54bc0b8` grouping. The published A12 IFC implementation, CLI tests, five MCP test scripts, IFC reader/worker, A18 demo and tests, and package file manifest have no delta from the first parent. Authority source/tests and notice admission source/tests have no delta from the second parent. `Confirm` unconditionally returns denial. The package file manifest does not stage the authority service, policy or binaries.

SHA-256 at the source merge:

| File | SHA-256 |
| --- | --- |
| `crates/archy-cli/src/lib.rs` | `250c8a08f41aa3db48c773d933581be385c5d66671bc288dddc6bd52a87544d0` |
| `crates/archy-cli/src/run_integrity.rs` | `28c886c910d2ec8b7354bde740c2125b96a5d3509bba9295750802d334e7181f` |
| `crates/archy-core/src/storage.rs` | `8a9e9bef69f239839024c55bfc7b13a5b16cc62dc33517b7ba793b71538922d0` |
| `crates/archy-cli/src/export_ifc.rs` | `edd7b1e5a4191f6fcda58b3729a04adb37930c50ff9fba01dac42a5c7c947803` |
| `authority/src/human_bus.rs` | `214d3702addb52ac48d04143731f462f499970f3f42e2c3229bb9e333245f1b0` |
| `authority/src/bin/journal_witness.rs` | `2ba6baa2cb49bec84eee4f04e83fa42dfa900ff042e43e04f671b43ad7bd4f86` |
| `packaging/arch/authority-notice-inputs.json` | `a7528813d1cfd92027fd84008e9c98c63f65094fc0e7dbab4fd819681a541a29` |

## Fresh checks on the source merge tree

Cargo used Rust 1.99.1, `--locked --offline`, two build jobs and an isolated target copied from an existing cache. Unless stated otherwise `TMPDIR=/tmp`.

- `cargo +1.99.1 test -p archy-core --lib orphan -- --test-threads=1`: 4 passed, including binding replacement and post-boundary orphan recheck.
- `cargo +1.99.1 test -p archy-cli --lib malformed_orphan_never_consumes_revision_or_retry_key -- --exact export_ifc::tests::malformed_orphan_never_consumes_revision_or_retry_key`: 1 passed; forged orphan did not consume revision or retry key.
- `cargo +1.99.1 test -p archy-cli --lib export_ifc::tests::native_ -- --test-threads=1`: 4 passed, 18 filtered, in 321.32 seconds. `TMPDIR=/var/tmp`, `ARCHY_IFC_TEST_PYTHON=/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python`, `ARCHY_IFC_TEST_NODE=/home/butahlecoq/.nvm/versions/node/v22.22.2/bin/node`, and `ARCHY_IFC_READER_NODE_PATH=tests/backend/ifc-reader/node_modules`. It exercised v2 and historical v1 JSON/YAML export/replay and orphan retry, selected DRAFT forged captures, and valid orphan exact-key replay. The temporary web-ifc module copy matched the three existing JS/WASM/package pins and was moved to trash afterward. The 30-second worker deadline was unchanged.
- `cargo +1.99.1 test -p archy-authority -- --test-threads=1`: 48 passed, including actual private-bus Review/Confirm denial, bus restart, journal, protected project and bounded zbus receive checks. Logind is mocked in the private-bus test.
- The exact freshly built `target/debug/archy-journal-witness` and `authority/tests/witness_separate_uid.py` ran in disposable `archlinux:base-devel-20261025.0.616225` Docker with `--network none --cap-add SYS_ADMIN --security-opt seccomp=unconfined` and read-only binary, script and fixture mounts: exit 0; separate UIDs, bind-mount check, rollback/replacement, simulated crash-state recovery, corrupt state and unauthorized peer/write checks passed. Simulated disk states are not power-loss tests.
- `python3 -B -m unittest discover -s tests/packaging -q`: 40 passed, including the 8 authority notice admission tests. The current `package.py` cannot directly stage the authority inventory; this test does not install authority.
- `cargo +1.99.1 build -p archy-cli`, then `ARCHY_BIN=<this-worktree>/target/debug/archy python3 -B -m unittest discover -s tests/demo -v`: build passed and 12 demo tests passed.
- `cargo +1.99.1 clippy --locked --offline --workspace --all-targets -- -D warnings`, `cargo +1.99.1 fmt --all -- --check`, and `git diff --check` passed. No complete workspace test suite or fresh CI run is claimed.

Dependency rationale: the authority uses `archy-cli` to verify accepted run semantics and a pinned local zbus patch for bounded bus handling. Notice admission checks the locked authority graph and patched source. Independent authority/notice reviews supplied for `edcf7f` do not replace review of this integration.

## Remaining limits

The broader PR security rereview was interrupted by an automated cybersecurity safeguard and remains open; the fresh tests here do not replace it. Independent integration review of the exact merge, including `storage.rs` recovery versus bounded review reads, is pending. Witness crash checks simulate reachable on-disk states; its evidence cannot cover an unobserved journal tail. Authority remains uninstalled, `Confirm` remains denied, and no human approval or release decision is implied. Installed runtime, actual Omarchy/Arch target acceptance, package authority staging and associated policy/service validation remain separate gates.
