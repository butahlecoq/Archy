# D1–D2 independent review

Reviewed 2026-10-29 against `docs/plans/task-01.md`, A01–A02 in `backlog.md`, `design-decisions.md`, the supplied release specification, and `C:/work/archy/.tools/initial-review.diff`. Scope: interrupted core/CLI/schema/test implementation. No implementer completion report exists. This is a review of the current partial state, not a claim that unfinished work was submitted as complete.

## Spec compliance verdict: changes required; slice incomplete

The Rust/core and CLI boundaries, strict input structs, canonical SI output, translated rectangle support, source/authority preservation, bounded YAML parsing, stable code vocabulary, and occupied-target refusal are present. The SI suffix policy conflicts with the task, negative fixture acceptance is not exercised, and required baseline documentation/lockfile/evidence remain absent. Omarchy/Arch Linux is the primary runtime; Windows is the development host. Actual Omarchy execution is deferred and cannot be claimed from this review.

## Code quality verdict: changes required; compilation unverified

The implementation is small and shares parsing between CLI and core. Input size limits, create-new writes, no arbitrary process execution, and default assumption authority are useful safeguards. Error paths lose necessary context, tests leave most advertised cases unexecuted, and there is no demonstrated Rust build/test/lint result. Coordinator owns current toolchain revalidation; this reviewer did not install tools or rerun Rust checks.

## Prioritized findings

### P1 — SI-suffixed fields incorrectly depend on global units

`crates/archy-core/src/validation.rs:45–55` rejects a supplied `_m`/`_m2` dimension unless effective units are `m`; lines 91–93 apply the same restriction to `polygon_xy_m`. Thus changing a valid project's display/global units to `ft` while retaining explicit SI dimensions makes it invalid. A mixed brief using generic feet dimensions and `floor_to_floor_m: 3.2` also fails. Constraint `value_m` inherits the same issue when `unit` is omitted and global units differ.

Task-01 explicitly requires explicit SI fields to retain SI meaning irrespective of global units. The contrary policy in `task-01-implementation.md` and the schema description does not supersede that requirement. Accept the SI value unchanged, apply conversion only to generic fields, and cover mixed/SI-only alternate-global-unit manifests and polygons. Keep contradictory duplicate representations deterministic.

### P1 — Most named negative fixtures do not run through the Rust parser

`crates/archy-core/tests/validation.rs` never loads `tests/fixtures/briefs/negative-cases.json` or its listed negative files. The function named `negative_fixtures_have_stable_errors_and_paths` tests five string replacements instead. Nonrectangular/self-intersecting/open polygons, JSON NaN/overflow, missing floor height, duplicate constraint IDs, hard GFA capacity and unsupported units have fixture files but no corresponding Rust execution. `scripts/check-schemas.py` validates only positive JSON fixtures and error-code envelopes.

Drive parser assertions from the manifest (including code and field), and add currently missing required-field cases, malformed/valid aliases, YAML numeric overflow, and input limits as appropriate. The current fixture inventory is not acceptance evidence. In particular, wiring the unsupported-units fixture will expose the next finding.

### P2 — Parser errors discard nested actionable field paths

`crates/archy-core/src/validation.rs:31–40` guesses a path from English error messages and only recognizes missing/unknown/duplicate field messages. An unsupported units enum returns field `$`, although `negative-cases.json` requires `units`. Type errors in `brief.storeys` also return `$`; unknown nested keys return a leaf such as `guess` rather than `site.guess`. This makes the machine-readable field contract incomplete and dependent on parser prose.

Capture structured deserialization paths for both formats, including enum/type failures and sequence indices; keep syntax-only failures at the document level with line/column details. Test exact paths for nested missing/type/unknown fields and unsupported units.

### P1 — Required lockfile and executable baseline evidence are absent

There is no `Cargo.lock`. `.github/workflows/ci.yml` generates a lockfile and then requires it to be tracked, so the present checkout cannot pass that gate: `git ls-files --error-unmatch Cargo.lock` must fail. Exact direct dependency pins do not lock transitives. There is also no actual Rust compilation/test/lint evidence for this partial implementation. Compilation compatibility of the parser option macros remains unverified; this is not an assertion of a particular compiler error.

Resolve and commit a real lockfile using an executable authorized toolchain, then run the prescribed format/clippy/test checks with `--locked` and report their exact results. If still blocked, retain the slice as unverified rather than completed. Linux CI is useful development evidence; actual Omarchy acceptance remains separately deferred.

### P2 — Schema/source agreement is not checked

`crates/archy-core/src/lib.rs` exposes three schemars-generated schema functions, but tests never compare them or representative Rust-serialized output against checked-in schemas. `scripts/check-schemas.py` checks handwritten positive fixture files, so it cannot detect Rust serializer/schema drift or an incorrect checked-in normalized fixture. The structural-versus-semantic distinction is documented and is reasonable for geometry rules; it does not replace serializer agreement.

Add a meaningful check that actual normalized/error serialization conforms to checked-in schemas and that structural field/default/nullability rules stay aligned with source types. Exercise the golden normalized output, rather than only its separately maintained fixture.

### P2 — D1 documentation and handoff are unfinished

`README.md` still states that no software exists, cites the superseded source-of-truth direction, says no license was selected, and has no init/validate usage or file-only `--project` behavior. Required `docs/adr/0001-source-of-truth.md`, `docs/adr/0002-typed-workers.md`, `docs/dependencies.md`, a license status record, and `docs/plans/task-01-handoff.md` are absent. No license file exists despite the coordinator's recorded MIT choice.

Complete these bounded deliverables, clearly label the implementation as partial release 0.1, record dependency/source/license decisions and real verification results, and preserve the distinction between Windows development checks and deferred Omarchy runtime acceptance.

## Review evidence and limits

Inspected all current core/CLI source and tests, three checked-in schemas, schema checker, workspace/CI manifests, example/fixture contracts and relevant planning documents. Confirmed absent deliverables with filesystem checks. `rg.exe` itself was still blocked by Windows Application Control during this review, so native PowerShell reads were used. No application source was changed, no toolchain installed, no Rust checks run, and no release/runtime success is asserted. Coordinator should append current executable-check evidence separately before dispositioning the findings.
