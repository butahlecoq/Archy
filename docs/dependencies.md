# Dependency decisions for D1–D2

Rust 1.99.1 is pinned. Cargo.lock records the real resolved graph; checks use --locked. No Python/backend package is required for core tests. Python jsonschema 4.26.0 is a development-only independent checker.

| Direct crate | Pin | Purpose | Metadata license |
|---|---|---|---|
| serde | 1.0.229 | Typed serialization | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | JSON parsing/output | MIT OR Apache-2.0 |
| serde-saphyr | 2.0.0 | YAML duplicate/alias/input budgets | MIT OR Apache-2.0 |
| serde_path_to_error | 0.1.20 | Structured nested paths for both parsers | MIT OR Apache-2.0 |
| schemars | 1.2.3 | Source-derived structural schemas | MIT |
| clap | 4.6.7 | CLI arguments | MIT OR Apache-2.0 |

Exact pins resolve and compile with the pinned toolchain in Ubuntu WSL. Deprecated serde_yaml is not used. UUID/hash/RNG/geometry/MCP dependencies are deferred to their owning slices. Official versioned API documentation is available at docs.rs for each crate; lockfile checksums identify actual packages. Distribution must inventory all transitives and bundle their notices/license texts. This direct-dependency decision record is not a completed distribution audit. See the Omarchy reuse research for future backend boundaries.
