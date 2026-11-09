# A14 passive comparison handoff

## Review round 1 correction

The independent review reproduced a raw U+202E in a declared authority. The same CLI probe reproduced it before this correction. Comparison Markdown now renders Unicode bidirectional controls (U+061C, U+200E–U+200F, U+202A–U+202E, U+2066–U+206F), Unicode line and paragraph separators, and control characters as visible Markdown-escaped markers such as `\[U+202E\]`. Legitimate Unicode, including Arabic text and emoji joined with U+200D, stays intact. Markdown delimiters remain escaped. JSON retains the original text. The policy applies to every string rendered by the comparison, including provenance, check explanations, preferences, and source hashes. Existing v1 rendering is unchanged.

The review's actual CLI probe returned `raw_bidi True` before the fix and `raw_bidi False` afterward. The corrected authority line contains `user\[U+202E\]:devorppa`. Direct rendering and real CLI regression tests failed before the change and passed afterward. The CLI test also verifies original JSON values and a byte-identical project inventory. Independent rereview of this correction is pending.

## Scope and result

This bounded slice adds `archy compare --project PATH [--run ID] [--json]`. Markdown is the default output. JSON uses `archy-comparison-v1`. It reads an accepted generation or edit batch and every variant in that batch. The default source is the latest accepted generation or edit run. `--run` selects an older one. A later evaluation run does not change the batch selector. The comparison uses the selected run's verified input and normalized snapshot, so edits to the current project file do not relabel old evidence.

The ranking policy `absolute-target-gfa-delta-v1` sorts by the absolute value of the evaluator's `target_gfa_delta` in square meters. Equal numeric values share a dense rank. Variant ID breaks display ties. Missing objectives have null values and ranks and sort last. The objective is approximate area distance, not a quality, feasibility, or compliance score. Failed and unknown checks remain visible alongside rank; evaluation tolerance remains visible. Declared constraint source and authority are unverified declarations. Preferences are subjective text. Daylight and cost are uncomputed. The output makes no regulatory or professional compliance claim.

The CLI uses `ProjectStore::read_history` and the existing semantic `verify_history` path. It does not lock, recover, create `.archy`, write a run, or change accepted history. Hashes identify evidence and detect inconsistency; they do not authenticate a human or detect a complete owner-controlled rewrite or rollback. Existing `archy-report-v1` rendering and fixtures remain unchanged. No dependency was added.

## Files and integration

- `crates/archy-report/src/compare.rs` owns the comparison document, ranking, and Markdown escaping. `crates/archy-report/src/lib.rs` exports it.
- `crates/archy-report/tests/compare.rs` and `tests/fixtures/comparison.schema.json` cover ties, failures, missing objectives, escaping, provenance, and schema drift.
- `crates/archy-cli/src/compare.rs` reads accepted evidence. `crates/archy-cli/tests/compare.rs` exercises the real binary, passive inventory, history selection, changed current input, and a consistently rehashed forged metric.
- `crates/archy-cli/src/main.rs` and `crates/archy-cli/src/lib.rs` register the command and public API. These two shared CLI files collide with the simultaneous MCP workstream. The parent integrator must merge command registration and module exports deliberately; this branch has no MCP changes.

The standalone comparison schema sits with its test fixture because `scripts/check-schemas.py` snapshots the complete legacy report contract example. Registering it in that example changes the frozen v1 fixture. The new test compares the schema to `comparison_schema()` directly. An integration owner can later add it to a revised central schema registry without changing v1 bytes.

## Validation

Run these commands from this worktree with `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a14-compare`, `CARGO_BUILD_JOBS=2`, and `RUSTUP_AUTO_INSTALL=0`:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --locked -p archy-report --test compare --test report -p archy-cli --test compare --test golden_artifacts
cargo test --workspace --locked
python3 scripts/check-schemas.py
```

The comparison tests, workspace suite, clippy, and schema script passed in this worktree. One pinned IFC worker integration test in the workspace suite is ignored by default because it requires its external worker environment. No actual Omarchy, Blender, IFC, or human authorization acceptance was run or claimed here. Independent review remains for the parent integrator, as this workstream was instructed not to use subagents.

## Remaining A14 work

This slice creates no approval, proposal, decision, selection state, or export. Final approval requires an independently enforced human authorization boundary. A same-UID CLI prompt or claimed actor cannot provide that boundary. A14 remains open. The parent integrator should review this diff and resolve the shared CLI collision before integrating it with MCP work.

## Frozen source hashes

These SHA-256 values cover the exact source, tests, schema, review, and baseline documents used for this handoff. The handoff itself is frozen separately at `/home/butahlecoq/work/archy/.tools/targets/a14-compare/a14-comparison-handoff.sha256` to avoid a recursive hash. Run `sha256sum -c /home/butahlecoq/work/archy/.tools/targets/a14-compare/a14-comparison-handoff.sha256` from this worktree.

```text
bd2f59776b87a80ae70935d1f3880d739edcc59299a830927692587d031a2c6c  AGENTS.md
8e76b2b39c2fbc37eea9e5da808baaaf657884deee3b4fd961bce053b8c24235  docs/specs/release-0.1.txt
1097d881b8ef92e9a5e80c50ee213207c9f2092f1d42478883006d03d84ed690  docs/plans/backlog.md
d27d3036a9135ee9933e2c83049b77b6b14a05bdcb592be4a00380accdcc47ea  /home/butahlecoq/work/archy/.tools/research/a14-comparison-approval.md
13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40  Cargo.lock
4bfe49fbe94f2e0dda3ba74e5cca8d65ca961bd751e876bb95d5425dca96ce05  crates/archy-report/src/compare.rs
53c5cad1a147b851bcfab84b532d77c0f67d1a358fdc4f5e1087b1897470088b  crates/archy-report/src/lib.rs
7bd9313d4637ee500c5532a6b6fb235fe714f060f6fbc4e810abe8e92396e916  crates/archy-report/tests/compare.rs
796705db31a220c7ecaac810987e164423b0d32cb535182377a7b47c72acce97  crates/archy-report/tests/fixtures/comparison.schema.json
70b913468260bc258c8c9a3d2ec8ceb80671cc3408a352284662830f7955c351  crates/archy-cli/src/compare.rs
e5abb8588ccdccad4c47e3e1de223824d877bb48ac9acf40fcbc67a0bda8fe76  crates/archy-cli/src/lib.rs
80f6323b0f5245e1319aa0964fe3f9ad0e9666d1247e099e000d9dcfd0b9d247  crates/archy-cli/src/main.rs
ccd8151bff74593a42030ac91d6b7be4b6b6593a020163a06a76f4fe292d6cc8  crates/archy-cli/tests/compare.rs
c2cf36b61fec408ab41a33936f47519a2aad9168e73dc42ce44cc7a2764b79b7  tests/fixtures/briefs/courtyard.json
0df362c3b2de4455ad0a41ff463d9bfb3fd4e52eb3ee1c36f62c3d15d5b1f09c  docs/plans/a14-comparison-review-round1.md
7a7023dcca1d92cee89b88fcd96160d57bf95fb41f2c9d7f01233f5c1022343e  crates/archy-report/src/templates.rs
2454a985022f8bc8d54078dceea17b9cbc07f5862960e1a270106993f37cf18d  crates/archy-report/tests/fixtures/report-contracts.json
```
