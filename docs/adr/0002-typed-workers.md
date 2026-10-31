# ADR 0002: Bounded typed workers

Accepted architecture boundary 2026-10-29; implementation deferred.

Rust owns CLI, canonical data, generation/evaluation and orchestration. Packaged Python workers own Blender/IfcOpenShell through versioned typed requests and responses. Requests must identify bounded operations, canonical parameters and project-scoped staging destinations. No project-supplied Python, arbitrary API names, shell strings or unrestricted paths are accepted.

Workers need executable/version checks, resource limits, cancellation, scoped output validation and artifact verification before immutable publication. Missing containment must fail explicitly. Backend dependencies remain optional for core tests. Reuse existing Omarchy applications and Linux isolation; no new viewport or scheduler.

No worker protocol or backend is implemented here. Future package boundaries archy-generate, archy-eval, archy-adapters, archy-report and archy-mcp are reserved in documentation; owning slices add real crates rather than placeholder packages. The workspace currently contains archy-core and archy-cli only.
