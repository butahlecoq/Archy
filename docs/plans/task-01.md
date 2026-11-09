# D1–D2: Workspace and Project Validation Implementation Plan

> **For agentic workers:** Use the user-requested Sol medium implementation agent and a separate Sol medium reviewer. Implement only this slice. Read [design-decisions.md](design-decisions.md) and [backlog.md](backlog.md), A01–A02, before editing. Record a short risk note and actual test results; do not claim unavailable checks passed.

**Goal:** Create an auditable Rust baseline whose `archy init` and `archy validate` commands normalize valid project briefs to SI and reject malformed inputs with stable JSON errors.

**Architecture:** Cargo workspace with Rust CLI/core ownership and reserved component boundaries for generation, evaluation, adapters, reporting and MCP. YAML and JSON deserialize through the same strict input model into canonical SI types. The supplied October 2026 specification takes precedence over older repository proposals.

**Spec:** [release-0.1.txt](../specs/release-0.1.txt), sections 3–5, D1–D2 and section 14; [design-decisions.md](design-decisions.md).

**Dependencies:** None within the new implementation. Coordinator reports official-documentation research pins: Rust 1.99.1; clap 4.6.7; serde 1.0.229; serde_json 1.0.154; schemars 1.2.3; serde-saphyr 2.0.0 (MSRV 1.89). Verify exact compatibility during resolution; explain dependency changes and preserve source references. Avoid deprecated serde_yaml. Commit a real resolved Cargo.lock; never hand-invent checksums. UUID/hash/random dependencies belong to later slices unless essential here.

## Global constraints

- Only `init` and `validate` become functional commands in this slice; no generator, geometry worker, IFC, MCP server or external model calls.
- Preserve existing upstream history and documentation; identify superseded proposals with clear current-spec links instead of deleting them.
- Validation cannot certify legal/regulatory compliance. Required unknown regulatory values remain declared assumptions rather than inferred rules.
- Reject non-finite, zero/negative or contradictory dimensions and unsupported schemas/units with actionable field paths.
- Canonical lengths are meters; units on display are presentation. Model rectangle site in right-handed local XY/Z-up coordinates.
- No silent overwriting of existing user files; init is project-scoped and non-destructive.
- Original Rust/core code uses MIT as the coordinator-selected repository license under the authorized implementation. Record rationale and separately license Blender worker code and third-party dependencies according to their obligations.
- Current Windows Application Control prevents Cargo execution. Attempt available verification and report exact blockers; add Linux CI to run Rust checks. Neither compilation nor tests can be claimed passing without actual output.

## Files and boundaries

- Create root `Cargo.toml`, resolved `Cargo.lock` if tooling permits, `rust-toolchain.toml`, `.gitignore` and `.github/workflows/ci.yml`.
- Create workspace manifests and crate roots for `crates/archy-{cli,core}/`. Reserve generate/eval/adapters/mcp/report names in documentation; add their crates with the owning slice rather than empty placeholders.
- Implement core in `crates/archy-core/src/{lib,project,units,error,validation}.rs`; init/validate wiring in `crates/archy-cli/src/{main,init,validate}.rs`.
- Create `schemas/{project,normalized-project,error}.schema.json`, `tests/fixtures/briefs/`, core/CLI integration tests in each crate's `tests/`, `examples/courtyard-housing/project.yaml`.
- Add `docs/adr/0001-source-of-truth.md`, `docs/adr/0002-typed-workers.md`, `docs/dependencies.md`, a license-decision status record and concise current README instructions. Coordinate shared files if another agent edits concurrently.

## Interface decisions to document before implementation

The implementer chooses concrete Rust names/signatures, exports them from `archy-core`, and records them in `docs/plans/task-01-handoff.md` for downstream consumers. Define: strict input project types; canonical `NormalizedSpec`; SI unit conversion; `validate`/parse functions shared by CLI and future tools; error envelope with stable code, field path/details and retriable flag. Include full required entity vocabulary (Project/Site/Constraint plus schema version and source authority), leaving Variant/Run/Artifact behavior to later issues.

Accept the supplied illustrative brief using `project_id`, `schema_version: 0.1.0`, `units: m`, site `width_m`, `depth_m`, `north_deg`, brief storeys/target GFA and constraints `value_m`/source. Add explicit floor-to-floor height to the golden fixture: 3.2 m. Generic unsuffixed dimension fields obey the declared global units; explicit SI `_m`/`_m2` fields always retain SI interpretation and must never be silently rescaled. Reject contradictory duplicate generic/SI representations instead of choosing one silently. Support a translated closed rectangular `polygon_xy_m` site as well as width/depth shorthand, preserving its position in canonical coordinates; test equivalence up to the intentional translation. Formalize required/defaulted fields and supported units. Permit missing legal authority but never classify its source as verified law. Target GFA is soft unless explicitly marked hard. Contradictory hard dimension bounds should be validation errors; infeasible sampled candidates remain a later generator/evaluator concern.

All stable error codes must serialize consistently: `INVALID_SCHEMA`, `INVALID_GEOMETRY`, `CONSTRAINT_FAIL`, `BACKEND_UNAVAILABLE`, `BACKEND_TIMEOUT`, `ARTIFACT_INVALID`, `REVISION_CONFLICT`, `PERMISSION_DENIED`, `INTERNAL_ERROR`. Most are reserved here. CLI JSON stdout must be machine-parseable; diagnostic progress belongs on stderr. Validation failures exit nonzero.

## Steps

- [ ] **1. Write risk/interface note.** Inspect upstream docs, record chosen public types/units policy and dependency rationale. Main risks: ambiguous SI suffixes, YAML non-finite values/duplicate keys, schema drift and unverified toolchain execution.
- [ ] **2. Establish workspace and CI.** Create manifests, crate roots and Rust 1.99.1 toolchain pin. Configure Linux `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; use `--locked` once a real lockfile exists. Python CI starts when Python source exists and must not fail on empty globs.
- [ ] **3. Add input fixtures and failing validation tests.** Golden YAML and equivalent JSON describe a 20×35 m rectangle, north 0°, three storeys at 3.2 m, 900 m² target, max height 12 m and edge setback 3 m. Assert canonical SI equality and preserved constraint source; do not assert computed metrics yet.
- [ ] **4. Add negative test fixtures.** Missing ID/schema/site/storeys/floor height as required by documented schema; unsupported schema/unit; zero/negative dimensions/storeys; JSON/YAML non-finite and overflowing values; duplicate/unknown fields; malformed YAML aliases; contradictory hard bounds; nonrectangular/self-intersecting supplied polygon if polygon input is supported. No panic, every error has stable code and actionable field path. If aliases are supported, bound expansion; otherwise reject explicitly.
- [ ] **5. Implement strict shared parsing/normalization.** Use the common validation entry point for YAML/JSON. Generate matching schemas from source types where feasible; normalization must not alter source file bytes. Record assumption/source/authority behavior and floor-height conventions.
- [ ] **6. Add CLI contract tests and implement init.** `archy init --name courtyard-demo --dir <new-dir>` creates a schema-referencing example/skeleton. Validate the created project. An existing user file/directory collision must fail without changing bytes; paths outside the intended explicit init target must not be touched.
- [ ] **7. Implement validate CLI.** `archy validate --project <file> --json` returns normalized spec on success. Invalid input produces the structured error envelope and nonzero exit; stdout remains parseable and stderr does not contaminate it. Document whether `--project` accepts both a manifest and project directory, and test the chosen behavior consistently.
- [ ] **8. Verify schema and docs.** Assert sample/fixture conformance and Rust serializer/schema field agreement. Add source-of-truth/typed-worker ADRs and dependency/license status. README accurately labels this slice as partial release implementation.
- [ ] **9. Run checks, hand off and review.** Run meaningful core/CLI tests, format and lint where executable. If Windows policy blocks Cargo, capture exact failure and request coordinator/CI verification rather than fabricating evidence. Record files, contracts, commands/results, blockers and next action in `docs/plans/task-01-handoff.md`; independent reviewer checks diff against A01–A02.

## Review focus and acceptance evidence

| Risk | Owning check | Evidence required |
|---|---|---|
| SI suffix/global-unit ambiguity | valid YAML/JSON + alternate-unit normalization tests | exact canonical equality and documented policy |
| Non-finite/overflow/contradictory values | named negative fixtures | stable codes/field paths, no panic |
| Duplicate/unknown YAML/JSON or alias surprises | parser tests | deterministic rejection or bounded documented alias behavior |
| Init destroys existing files | occupied-target CLI test | byte-identical original files and nonzero error |
| Claimed compilation without executable toolchain | command/CI transcript | actual pass output, or explicit unresolved blocker |

Slice completion requires passing available checks plus real Rust build/test evidence from a runnable environment; until that exists, implementation may be reviewable but the slice is not verified complete. This slice does not satisfy full release acceptance by itself. GitHub authentication is currently blocked: preserve A01/A02 local IDs and report the tracker blocker; do not claim issues/PRs were created.


