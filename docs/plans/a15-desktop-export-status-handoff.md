# Accepted IFC export in desktop status

Base: `1a0bf93f613daa67d3b75d4d779ad055144ab2fb`. This change is isolated from the proposal follow-up review.

Plan: reproduce status on a genuinely accepted draft export, add the typed operation at the desktop boundary, keep the published schema and plugin validator in sync, and verify the read-only display still says draft.

Root cause: run integrity accepted `export-ifc`, but `desktop_status::summary` rejected the same operation after verification. The plugin and published schema also excluded it. Before the fix, the genuine-export Rust test returned `ARTIFACT_INVALID: unsupported run operation`; the Node desktop probe returned no status; the schema test rejected the summary.

Change: add `export-ifc` to the Rust desktop operation enum and summary mapping, the JSON Schema enum, and the plugin validator. The plugin displays `draft schematic IFC` alongside required-check status. No export, approval, or artifact-open behavior changed. No dependency was added.

Verification (disk-backed `TMPDIR`, isolated Cargo target):

- Genuine native IFC export to desktop status: 1 passed after failing before the fix. The test verifies `DRAFT`, the typed operation, selected-run equality, required-check value, and verified design-manifest artifact.
- `archy-cli` library suite without native environment variables: 23 passed. The native export tests in this suite return early when `ARCHY_IFC_TEST_PYTHON` is absent; the focused native test above ran with the pinned Python and Node executables.
- `desktop_readers`: 8 passed.
- DesktopModel Node tests: 13 passed, including the failing-before probe regression.
- Desktop status schema Python tests: 6 passed, including the failing-before enum regression.
- Generated Rust desktop status schema equals the published JSON Schema.
- `cargo clippy --locked --offline -p archy-cli --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check`: passed.

The standalone CLI command could not reach desktop summary in this development environment because the installed `/usr/lib/archy/ifc-verifier` directory is absent; it returned `BACKEND_UNAVAILABLE` at run verification. The Rust test uses the pinned local IFC worker and reader without host services and reaches the exact desktop summary path. Independent review of this branch remains pending.
