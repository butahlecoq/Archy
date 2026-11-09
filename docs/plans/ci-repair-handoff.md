# CI repair handoff

Date: 2026-10-29. Worktree: `/tmp/archy-ci-repair`, detached at `7189f5be0f8cc8d09e9f4e30db143c63bfe96552`. No commit, push, branch, agent, or shared-checkout source edit was made for this repair.

## Root causes and plan

Both published Rust runs [40878220163](https://github.com/butahlecoq/Archy/actions/runs/40878220163) and [40878229206](https://github.com/butahlecoq/Archy/actions/runs/40878229206) checked out `7189f5b` and stopped at `cargo fmt --all -- --check`. Their lockfile and pinned-toolchain steps succeeded; clippy, tests, and schema validation were skipped. A fresh detached base checkout reproduced the format failure. Its `cargo test --locked --workspace` exited 101: both generation integration tests failed because `generate_variants` returned `InvalidSchema: Generation not implemented`. Formatting alone would therefore leave CI red.

Plan executed: keep the base workspace and dependency graph; import only the Task02 generation/evaluation/canonical fixes after the independent round1 rereview; format the base Rust files; extend the existing source-derived schema gate for Task02; run the exact required CI gates. The workflow itself is unchanged.

## Source allowlist and provenance

The imported, reviewed content came from the shared repository's uncommitted Task02 snapshot documented in `docs/plans/task-02-handoff.md` and independently rereviewed in `docs/plans/task-02-review-round1.md`. Copied reviewed files:

- `crates/archy-core/src/{check,metric,validation,variant}.rs`
- `crates/archy-eval/src/lib.rs`, `crates/archy-eval/tests/analytic.rs`
- `crates/archy-generate/src/lib.rs`, `crates/archy-generate/tests/generation.rs`, `crates/archy-generate/examples/task02_contracts.rs`
- `schemas/{variant,generation,evaluation}.schema.json`
- `tests/fixtures/analytic/courtyard-shapes.json`, `tests/fixtures/{generation,evaluation}/courtyard.seed42.json`

The only authored adaptations are formatting the base `crates/archy-core/src/lib.rs` while retaining its existing exports and adding Task02-only registration and golden output validation to `scripts/check-schemas.py`. `cargo fmt --all` formatted the selected Rust files. No storage/run exports, CLI/report/adapters/doctor/IFC work, or unrelated schemas/fixtures were imported. `Cargo.toml`, `Cargo.lock`, `crates/archy-core/Cargo.toml`, and `.github/workflows/ci.yml` have no diff from `7189f5b`. The base already pins `geo=0.33.1`, `sha2=0.11.1`, `rand_chacha=0.10.0`, and `rand_core=0.10.0`; no new dependency was needed. These support planar geometry, canonical hashes, and deterministic seeded sampling without a backend dependency.

## Local CI evidence

All commands below ran from `/tmp/archy-ci-repair` on Rust `1.99.1` and exited 0 unless stated otherwise.

| Gate | Exact command and observed result |
| --- | --- |
| Tracked locked graph | `cargo metadata --locked --format-version 1 --no-deps > /dev/null`; `git diff --exit-code -- Cargo.lock`; `git ls-files --error-unmatch Cargo.lock` — all 0 |
| Format | `cargo fmt --all -- --check` — 0, no diff |
| Clippy | `cargo clippy --locked --workspace --all-targets -- -D warnings` — 0 |
| Tests | `cargo test --locked --workspace` — 0; CLI 5, core 12, eval 12, generation 4 passed; doc tests had 0 failures |
| Schemas | `/tmp/archy-ci-schema-venv/bin/python scripts/check-schemas.py` with CPython 3.12.13 and `jsonschema==4.26.0` — 0; all six checked-in schemas source-match and validate, Task02 serializer/golden fixtures conform, and 26 CLI negative fixtures conform |
| Diff hygiene | `git diff --check` — 0 |

The local schema environment was created outside the worktree with `uv venv` and installed the workflow's exact `jsonschema==4.26.0` pin. The GitHub runner's `python -m pip install` command itself was not run locally. No new remote Actions run exists because this worktree has not been published.

## Remaining review and publication

Independent reviewer should confirm the source allowlist and rerun the gates on this detached tree. Parent coordination can then decide how to publish the reviewed repair; remote CI can only confirm the published snapshot. The shared checkout still contains active CLI, storage, report, adapters, doctor, and IFC changes owned by other authors. They were deliberately excluded, so their integration or later CI failures are outside this evidence. Backend/reader integration and Omarchy acceptance were not run and are not claimed by these core gates.
