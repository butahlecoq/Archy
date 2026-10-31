# A12 v2/YAML plus strict IFC and HEADER preliminary integration

30 October 2026. Base: published `c72f5309a681f36f56cb05e3932486bb7410a3f0`. This is a local integration candidate only. The frozen HEADER correction received an independent ISSUES verdict: claim-bearing legal STEP comments in HEADER are accepted. A systemic comment correction is in progress outside this commit. This commit does not approve or publish A12, prove installed Omarchy behavior, or establish a target claim.

## Frozen inputs

- v2/YAML candidate: staged tree `0e2e273aaabe1ed93d740387316e688fab6991d0`; patch `/var/tmp/a12-v2-yaml-c72-review-only-20261030.patch`, SHA-256 `7d6209daed2a03b9780c67d94fa8fd7edba81ef694ea3913422849ecf691f745`; scoped independent PASS in `.tools/github-updates/a12-v2-yaml-c72-review-sol.md`.
- Strict semantic candidate: staged tree `5a6a65f95ab21367fa68484de265025be21e3ef4`; 37-path patch SHA-256 `7c4f4c995fe14837c1be88f2cdb2bd4d6a6d1fac1a1eac3fe67df30105eb915c`; rereview 7 found the HEADER bypass. Its service gate, adapter, packaging, and security files already match published c72 bytes.
- HEADER correction: clean commit `3c9f3981cd3ba91be60c9ba126d9af0b5452e51c`; one-commit patch SHA-256 `43529c5fce431d8ea0f568009ed370594a7215a802a9432b70554b4559ad41a5`; handoff `.tools/github-updates/a12-ifc-header-authority-fix-handoff-20261030.md`. Its independent review returned ISSUES for claim-bearing STEP comments; field-level controls passed.

## Reconciliation

The v2 patch applied cleanly to c72. The strict reader's complete DATA metadata, entity arity, derived field, and source-bound GUID checks were combined with the v2 reader's bounded original JSON/YAML source modes and byte digest checks. The strict STEP scope was retained. The HEADER patch then added fixed HEADER values and a timestamp bound to `ExportedAtUTC`; its worker sets the same timestamp and fixed `model.ifc` filename. The v2 framed bridge and 1 MiB source bound remain. Rust verifier pins were recalculated from the combined files.

The HEADER patch did not apply wholesale: `ifc_verifier.rs` and `reader.test.cjs` overlapped the v2 changes, and its historical semantic freeze document was absent. Those paths were reconciled manually; the document was added as provenance. The header fixtures and worker change applied cleanly. Newer c72 Cargo/workspace, authority/profile migration, A22 desktop, A23 playbooks and validation, and service gate files were retained. The three A17 staged-edit interruption paths from published `15108bad8a187ad838eca5b0c524836a2ebd4175` were applied exactly after the A12 reconciliation. There are no unmerged paths.

The exact combined script SHA-256 values are:

- `reader.cjs`: `ac49cd744f9d85e59d3f83c3907ef10fb7c215958e024568da47c002ec649555`
- `step-scope.cjs`: `e2f797d8ac4063265aa5977b60800675e43a5b3ceb94e202058161f1f8ca1718`
- `verify-stdin.cjs`: `71abb0c0050f2980d6c94c858ceb75796b11ab5e011c2867d7050c0ae362ef4c`
- Worker `properties.py`: `88a8ffc62207c0efdeaa4943afe6e01bc2626a9227e8bed8ca71181c7d7c907f`

`ifc_verifier.rs` pins the first three. The package inventory contains all three verifier scripts. The HEADER patch changed only Node test subprocess deadlines to 60 seconds; the native worker wall deadline and worker timeout settings were not changed.

## Local evidence and remaining gates

- Node 22.22.2 combined reader suite: **36 passed, 0 failed** in 242.61 seconds. This includes original v2 JSON/YAML bytes, all six HEADER claim frames, HEADER timestamp binding, strict DATA metadata, GUID, arity, and geometry checks.
- Repaired WSL interpreter: CPython with SQLite 3.46.1 and IfcOpenShell 0.9.0.post1. Native matrix: **3 passed, 0 failed; 4 filtered** in 304.96 seconds. The real worker and independent parser exercised canonical v2 JSON/YAML rectangle and L, historical v1 JSON/YAML rectangle and L, and selected DRAFT rejection, retry, publication, and replay. The first compile was interrupted after the new HEADER patch arrived; only the complete rerun is counted.
- `cargo test --locked --offline -p archy-cli --lib -- --skip native_`: **4 passed, 0 failed**. A17 interruption test: **2 passed, 0 failed**. Packaging unittest discovery: **32 passed, 0 failed**.
- `cargo fmt --all -- --check`, script pin/byte comparison, package inventory check, and staged whitespace check passed on the combined tree after the A17 graft.

The HEADER correction's independent review found a byte-level comment claim bypass (`.tools/github-updates/a12-ifc-header-authority-review-sol.md`). The same framed mutation was run on this combined tree with fixed accepted source bytes: both rectangle and L returned exit 0 and `status: ok` for `/* Certified architect; approved for construction */` after `HEADER;`. The next systemic comment patch must be grafted into the combined STEP parser, repinned, and independently reviewed before A12 approval. The frozen HEADER field correction and these passing tests do not close that issue. Installed Omarchy package/service validation, target runtime ownership and human approval remain separate. No push, other worktree edit, service change, or child agent occurred.
