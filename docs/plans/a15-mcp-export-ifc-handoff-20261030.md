# A15 MCP export_ifc review handoff

Base: `3ad404af5ce4753cea02be86d4c54f1aa2d9a7b4`. This commit changes only the A15 MCP facade, the shared CLI IFC export entry, its protocol checks, schema fixture, and MCP docs. It does not alter the original checkout, A13 worker `verify.py`, A23 runtime files, host services, or remote refs.

## Behavior

- `export_ifc` requires the configured project ID, one accepted source run UUID and variant, an idempotency key, canonical expected revision, and a project-relative destination. The existing transaction validates key and destination and owns publication, replay, stale revision, and conflict checks.
- The MCP handler calls the selected CLI IFC export body through its already pinned project reader. A replacement project file is refused. The CLI still opens its own reader and calls the same body.
- A successful response carries the verified run receipt and selected source/variant, labeled `SCHEMATIC_MASSING_ONLY` and `DRAFT`. These labels do not grant or imply human approval. A missing fixed service gate returns the actual `BACKEND_UNAVAILABLE` error, with no success data or revision.
- The proposal tool remains unavailable; no generic path, shell, process, Python, or approval tool was added.

## Verification

- `cargo fmt --all --check` and `git diff --check`: exit 0.
- `cargo clippy --locked -p archy-cli -p archy-mcp --all-targets -- -D warnings`: exit 0.
- `cargo test --locked -p archy-cli -p archy-mcp`: exit 0, including IFC export/replay, pinned reader, and MCP transport tests.
- `python3 crates/archy-mcp/tests/stdio_contract.py`: exit 0.
- `python3 crates/archy-mcp/tests/parse_errors.py`: exit 0.
- `python3 crates/archy-mcp/tests/compare_stdio.py`: exit 0.
- `python3 crates/archy-mcp/tests/mutations.py`: exit 0.
- `python3 crates/archy-mcp/tests/render_variant.py`: exit 0.
- `python3 crates/archy-mcp/tests/export_ifc.py`: exit 0.
- `python3 scripts/check-schemas.py`: exit 0 after the new `export_ifc.py` entry was added; nine MCP schema pairs matched SDK descriptors and all 12 demo checks passed.
- `export_ifc.py` observed the absent `/usr/lib/archy/archy-service-gate`: MCP and CLI returned identical `BACKEND_UNAVAILABLE` error objects, repeated calls created no receipt or revision, and malformed/stale/key conflict/unsafe destination/project replacement calls failed without publication. The successful IFC branch is conditional and was not exercised against a real installed backend here.

All commands used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a15-mcp-ifc-20261030` where relevant.

## Separate review and integration

Request independent security review of binding, destination, revision, idempotency, and draft status; request compatibility review of the advertised output schema and CLI/wire parity. Do not claim installed Omarchy or native IFC acceptance from this environment. Final integration must include the disjoint A13 worker placement fix `18faf75bc9c6b5ffb9b2a51b31d7aefff917025b`. A23 runtime changes and broader A12 security rereview remain separate.
