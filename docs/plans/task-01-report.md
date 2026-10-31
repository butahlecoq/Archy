# D1–D2 implementation report

Date: 2026-10-29. Branch: feat/release-0.1. Implementation follows task-01 and the coordinator SI ruling in current-handoff. Independent review is the next integration gate; this report records implementation evidence rather than whole-release acceptance.

## Delivered changes

Completed the interrupted Rust core/CLI baseline and review findings: explicit SI values remain SI under any declared units; generic values convert; simultaneous representations fail. Mixed units and translated SI polygons are covered. Text preferences survive normalization. Target GFA remains soft by default; infeasible height is deferred to candidate evaluation. Area comparisons now use a distinct 1e-6 m² constant rather than a length tolerance.

Both actual deserializers use serde_path_to_error to capture nested enum/type/container paths and sequence indices. Missing/unknown/duplicate field messages contribute the leaf key that serde reports at the containing object; syntax-only failures retain document location in the message. YAML numeric/boolean-to-string coercion is disabled. Anchors/aliases/merge keys, duplicate keys and unsupported tags remain rejected under input budgets.

The translated-site regression uses explicit input rather than newline-sensitive replacement. The manifest test executes all 26 negative fixtures (17 original, 9 added) with expected codes and declared field paths. Required-field, alias, overflow, duplicate, geometry, bound, generic/SI and parser-path cases are covered. CLI tests check init/validate, source preservation, occupied file/directory refusal, invalid-name no-mutation, file-only project arguments and machine-readable failure output.

Schemas now come directly from source types. The contracts example exports actual Project/NormalizedSpec/ArchyError serialization plus generated schemas. The Python checker compares generated schemas to committed files, validates actual serialization and golden normalized equality, builds the real CLI and exercises its golden output and all 26 negative fixtures. Schemas are structural: core semantic validation still owns positive/finite dimensions, representation exclusivity, versions, geometry and hard capacity.

Added accurate README usage/contracts, source-of-truth/typed-worker ADRs, dependency rationale, original-core MIT license and package metadata, separate backend/license status and this handoff. Future crates are reserved in documentation only. CI now verifies the tracked lockfile with --locked rather than re-resolving transitives on every run. Existing Rust source was formatted across both crates.

## Actual execution evidence

Environment: Ubuntu under WSL on the Windows development host, pinned Rust 1.99.1. No Windows security policy was changed. Commands below ran from /mnt/c/work/archy/repository via `wsl -d Ubuntu -- bash -lc`; Cargo used `CARGO_TARGET_DIR=/mnt/c/work/archy/.tools/cargo-target`.

| Command | Actual result |
|---|---|
| cargo test --locked -p archy-core (initial regression run) | Failed as expected: nested paths, SI/preferences, translated polygon, negative-manifest unsupported units; 6 passed/4 failed |
| cargo test -p archy-core (first fixes) | 9 passed/1 failed: sequence-path marker truncated at closing bracket; corrected |
| cargo test --locked --workspace (next regression run) | Exposed YAML number-to-string coercion in nested constraint source; corrected with no_schema=true |
| cargo fmt --all | Completed |
| cargo fmt --all -- --check | Passed |
| cargo clippy --locked --workspace --all-targets -- -D warnings | Passed after correcting one needless-borrow warning in the explicit-input test |
| cargo test --locked --workspace (final) | Passed: 12 core integration tests + 3 CLI integration tests, 0 failures; unit/doc targets had 0 tests |
| cargo build --locked --workspace | Passed |
| python3 scripts/check-schemas.py (final) | Passed: all 3 schemas equal generated contracts; curated/actual serializers conform; actual CLI golden output and all 26 negative fixtures conform |
| cargo metadata --locked --format-version 1 | Passed; direct-dependency licenses inspected from resolved metadata |
| git diff --check | Passed (Git emitted only Windows line-ending conversion notices) |

The actual CLI was additionally invoked with `validate --project examples/courtyard-housing/project.yaml --json`, producing the expected normalized fixture. Real init invocation and non-destructive collision behavior run in CLI integration tests.

## Limits and next actions

Actual Omarchy installation/runtime/desktop and visual acceptance remain explicitly deferred until a target machine exists. No generation, evaluator, worker, backend, IFC, MCP or external AI call was implemented. No release/compliance certification is implied. Complete transitive dependency notices and backend obligations remain distribution gates. Independent review and CI for the final pushed commit must be assessed before integration; no merge or issue closure is authorized by this report.

## Round 2: valid scalar-looking init IDs

The independent review at 59d3c90 exposed one P2: inserting a valid ID as an unquoted YAML scalar rejected numeric/boolean/null-like names. Fixed init to quote the project_id scalar after the unchanged identifier allowlist (which excludes quotes, escapes and newlines), and restrict substitution to the project_id entry. No input types, schemas or identifier rules changed.

The focused regression first failed on ID123 with the expected strict-YAML quoting error. After the fix, 12 scalar-looking identifiers (123, true, false, null, NULL, yes, 001, 0xFF, 1e3, -123, - and _) each initialized, validated with the exact string ID, and refused a second init without changing manifest bytes. Five unsafe identifiers (slash, space, newline injection, empty and overlength) failed without creating a target.

Fresh Ubuntu WSL Rust1.99.1 checks on this fix: `cargo fmt --all -- --check` passed; `cargo clippy --locked --workspace --all-targets -- -D warnings` passed; `cargo test --locked -p archy-cli` passed all5 CLI integration tests; `python3 scripts/check-schemas.py` passed schema/source equality, actual serializers, real CLI golden output and all26 negative fixtures. Cargo used the same external CARGO_TARGET_DIR recorded above. Core code/schema remained unchanged, so the previous12 core-test result was not rerun for this scoped correction. `git diff --check` passed. Scoped independent rereview and final CI remain required; Omarchy execution remains deferred.
