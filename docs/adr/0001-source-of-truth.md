# ADR 0001: Structured manifests are authoritative

Accepted 2026-10-29 for release 0.1 D1–D2.

The supplied release specification supersedes older proposals that included IFC in the editable source of truth. Rust owns versioned input and canonical SI types. Original briefs and future immutable normalized run snapshots are authoritative; Blender and IFC are derived artifacts. This slice only parses/normalizes and creates no run state.

CLI and future tools share parse_project/normalize. Generic dimensions obey units; _m/_m2 always mean SI; duplicate representations fail. Preserve source, declared authority and textual preferences without inventing regulatory or quantitative evidence. Absent authority becomes assumption. Only schema 0.1.0 is supported; future migrations must be explicit.

Generated JSON schemas track serde structure. Semantic dimension, geometry, finite-arithmetic and capacity checks remain core validation, without certifying legal compliance.
