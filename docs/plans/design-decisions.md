# Release 0.1 implementation decisions

The 29 October 2026 supplied specification supersedes earlier proposal documents. Implement the full Month 1 release baseline; Month 2 and later are a future backlog.

- Rust owns CLI, versioned project data, deterministic generation/evaluation, reports, and scoped adapter contracts. Python owns Blender and IFC workers.
- Structured manifests are authoritative; Blender and IFC files are derived artifacts.
- Use a Cargo workspace and separate crates following specification section 3. No cloud or model-provider dependency is required.
- Project input accepts YAML/JSON; normalize explicit units to SI and preserve original input per run. All dimensions must be finite and positive. Distinguish invalid schema from infeasible candidate constraints.
- Generation is deterministic, retains invalid candidates with explanations, and suppresses geometric duplicates. Version the algorithm and tolerance.
- Local IFC export before approval is allowed only as clearly marked DRAFT. Approval remains an explicit local human event; MCP proposes approval but cannot grant it.
- Use typed request/response files for trusted packaged workers, project-scoped outputs, immutable run directories, hashes and parsing checks, bounded execution with cancellation.
- Coordinator chooses MIT for original Rust/core code under the authorized implementation. Blender worker code and dependencies retain separate applicable licenses; distribution obligations remain a release check.
- Windows is the development host. Omarchy/Linux runtime, headless Blender, independent IFC reopen, and clean-target visual verification remain release gates and cannot be inferred from Windows unit tests.

## Approach

Follow the specified Rust/Python split. A Python-only prototype would not deliver the prescribed Rust core; direct arbitrary Blender MCP execution would not satisfy typed authority boundaries. Work in the newly cloned repository on feat/release-0.1, preserving upstream history. Sol medium agents implement bounded slices and independently review them; Astra coordinates and researches.

## Initial slice

D1-D2 deliver the Cargo workspace, init/validate CLI, strict input types and normalized SI output, schema, sample and negative fixtures, CI and initial ADRs. Later slices add generation/evaluation, persistence/reports, Blender, IFC, MCP, and final hardening in dependency order. Track all D1-D20 requirements and acceptance evidence in docs/plans/backlog.md.

