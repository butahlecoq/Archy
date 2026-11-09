# D1–D2 downstream handoff

Implementation evidence: [task-01-report.md](task-01-report.md). Source contracts are in archy-core; CLI delegates parsing/normalization to that shared library. This is a partial release baseline, pending independent review and final CI; actual Omarchy acceptance is deferred.

## Public interfaces

- `parse_project(text: &str, format: InputFormat) -> Result<NormalizedSpec, ArchyError>` accepts Json/Yaml, enforces input/parser budgets, deserializes strict types, then normalizes. No file mutation or backend dependency.
- `normalize(project: Project) -> Result<NormalizedSpec, ArchyError>` validates an already-deserialized project. All fields remain subject to semantic checks.
- `Project`, `Site`, `Brief`, `Constraint`, `Generation`, `Shape`, `ConstraintKind` are exported from core. Project requires version/id/units/site/brief/constraints; preferences default to an empty string list and generation defaults to count3/seed42/rectangle+l_shape.
- `units::Units::{M,Mm,Cm,Ft}` serializes as m/mm/cm/ft; `Units::meters(self) -> f64` returns the length scale. Generic area conversion squares that scale. Explicit SI fields bypass conversion. Supplying both dimensional representations is an error.
- `NormalizedSpec` includes version/id/units=M, site, brief, constraints, preserved preferences, generation, normalization_version=si-rectangle-v1 and tolerance_m=1e-6. NormalizedSite has a 5-point closed consistently wound axis-aligned polygon_xy_m retaining translation and north_angle_deg modulo360. NormalizedBrief has storeys, floor_to_floor_m, target_gfa_m2 and target_gfa_hard.
- `NormalizedConstraint` preserves id/kind/source, canonical value_m and declared authority; omitted authority becomes assumption. Declared authority is never verified law. ConstraintKind currently supports MaxHeight and MinEdgeSetback.
- `ArchyError {code: ErrorCode, message: String, field: String, retriable: bool}` serializes directly (no wrapper). Nine stable codes are INVALID_SCHEMA, INVALID_GEOMETRY, CONSTRAINT_FAIL, BACKEND_UNAVAILABLE, BACKEND_TIMEOUT, ARTIFACT_INVALID, REVISION_CONFLICT, PERMISSION_DENIED, INTERNAL_ERROR. Fields are dotted paths with sequence indices; `$` denotes a document error. Current errors are non-retriable; most codes reserve future use.
- `project_schema()`, `normalized_schema()`, `error_schema()` return source-derived `schemars::Schema`. The `contracts` core example exports these and real serialized instances for independent checking.
- `validation::MAX_INPUT_BYTES=1_048_576`, `TOLERANCE_M=1e-6`, `TOLERANCE_M2=1e-6` are public constants. Area tolerance is dimensionally distinct. A04 owns future metric tolerance metadata/evaluation.

## Command behavior

`archy init --name ID --dir NEW_DIR` requires an absent target and existing parent, validates the embedded example before creating a directory, and writes project.yaml plus project.schema.json with create-new semantics. It refuses occupied directories/files/symlinks. Failure can leave a partial new directory; it never removes potentially user/concurrent files. It does not create ancestor directories.

`archy validate --project FILE [--json]` accepts .yaml/.yml/.json manifest files only. Success emits normalized JSON; --json failure emits one structured error to stdout and nonzero exit. Progress/errors without --json use stderr. Source bytes remain unchanged. Clap argument usage failures use clap's normal stderr contract, distinct from a project validation envelope.

## Follow-on boundaries

Only archy-core/archy-cli are actual packages. Future generate/eval/adapters/report/mcp packages are reserved, without placeholders. Generation must reuse canonical coordinates/preferences and retain infeasible candidates; it must not convert declared constraints into verified regulation. Over-height candidate feasibility belongs to evaluation, not project schema rejection. Normalization records declarations but does not generate variants or run state.

Add migrations explicitly rather than changing schema0.1.0 meaning silently. Before editing types, regenerate the structural schemas from contracts and run scripts/check-schemas.py, which catches schema/serializer drift and exercises all manifest negative cases through the actual CLI. Run pinned fmt/clippy/test --locked. Worker/backend/Omarchy testing remains separate; consult ADR0002 and reuse research before those slices.
