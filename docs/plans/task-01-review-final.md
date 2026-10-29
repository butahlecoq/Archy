# D1–D2 final independent review

Reviewed 2026-10-29 at `59d3c90336452f1cba9b4e1fe47b0e941b387b82`, using the `b9e436f..59d3c90` diff package, current core/CLI source and tests, generated schema contracts/checker, CI, and baseline documentation. Requirements: task-01, original task-01-review, implementation report/handoff, current-handoff SI ruling, design decisions and release specification. This report reviews the bounded D1–D2 slice, not full release acceptance.

## Spec compliance verdict: changes required

The six original findings are addressed. One newly exposed valid-input failure in `init` remains (P2 below). Correct it before advancing this slice. Omarchy/Arch Linux remains the primary target; actual target execution is explicitly deferred until a machine exists.

## Code quality verdict: changes required

The implementation keeps shared typed validation in core, bounds parser input, uses non-overwriting filesystem operations, and compares generated schemas with actual serializers and CLI output. The report contains executable WSL format/lint/test/schema evidence. The remaining YAML construction bug contradicts the public identifier contract and needs a focused regression test.

## Actionable finding

### P2 — `init` cannot create projects with valid numeric/boolean/null-like names

Location: `crates/archy-cli/src/init.rs:21` (template substitution at line 24; validation at line 26).

The identifier check accepts 1–128 ASCII letters, digits, underscores and hyphens, consistent with the documented project-ID contract. However, `name` is substituted into an unquoted YAML scalar. With strict YAML string parsing, `archy init --name 123 --dir <absent-target>` fails with `project_id: ... The string value [123] must be quoted`. Names `true` and `null` fail similarly. These are valid IDs; the command should generate a valid project for them. The normal `courtyard-demo` case succeeds. No target directory is created on these failures.

Serialize the project ID as a YAML string (or build and serialize the typed project) instead of inserting a raw scalar. Keep the existing identifier contract. Add focused CLI coverage for numeric, boolean-like and null-like names, requiring successful init, subsequent validate, exact preserved project_id, and collision refusal.

## Original findings disposition

| Original finding | Review result |
|---|---|
| P1 SI-suffixed fields depend on global units | Addressed: SI values bypass conversion; generic dimensions use declared units; mixed/alternate-unit and translated polygon tests exist. Duplicate representations fail. |
| P1 negative fixtures are not executed | Addressed: the core test consumes the 26-case manifest and checks codes/declared paths; the schema checker also exercises those files through the real CLI. Required-field, malformed alias, overflow and input-budget cases were added. |
| P2 nested parser paths are discarded | Addressed for the reviewed contract: both deserializers use serde_path_to_error; missing/unknown/duplicate leaf paths are completed; nested type/enum/index cases have assertions. |
| P1 absent lockfile/build evidence | Addressed: real Cargo.lock is tracked, CI consumes --locked, and task-01-report records pinned WSL Rust 1.99.1 format/clippy/test/build results. |
| P2 schema/source agreement unchecked | Addressed: contracts exports source-derived schemas and real project/normalized/error values; checker compares committed schemas and validates serializer/CLI output plus golden equality. Structural-versus-semantic boundaries are explicit. |
| P2 unfinished baseline documentation | Addressed: README, ADRs, dependency/license records, MIT license, report and downstream handoff exist and describe this as partial release work with deferred target acceptance. |

## Independent verification and limits

- Read the load-bearing normalization, geometry, SI conversion, error, init/validate and schema-checking paths and associated test changes. No additional blocker was identified in those reviewed paths.
- Ran `CARGO_TARGET_DIR=/mnt/c/work/archy/.tools/cargo-target cargo build --locked -p archy-cli` from the repository in Ubuntu WSL: passed on the reviewed source.
- Ran a focused Python subprocess probe against that target directory's `debug/archy`, using a fresh temporary parent and absent target per name: `123`, `true`, `null` each exited 1 and left the target absent; `courtyard-demo` exited 0 and created its project. This directly confirms the finding above.
- Did not duplicate the implementer's full fmt/clippy/workspace-test/schema runs on unchanged code. Their reported results are implementation evidence, not checks independently rerun by this reviewer. No final remote CI result was inspected.
- No product source was edited. No Omarchy installation/desktop/runtime or visual acceptance, backend/Blender/IFC/MCP behavior, distribution license audit, merge, or issue closure was performed or implied.
