# A18 render demo continuation handoff

Date: 2026-10-30. Base: `8f0dfc59bd85e2085c1e312148899455a363562d`. Scope: `scripts/demo.py`, its demo tests, and source entry documentation. This is a source continuation of issue #18, not release acceptance. No push, host service, child agent, IFC export or approval action occurred. An independent review remains to be done before this slice is accepted.

Published head advanced separately to `75df70f6bd75b5fa6afd4a20b6fd8dfe38d7fe07` during this review. Its only changes from this base are the five Python probes in `crates/archy-mcp/tests/{compare_stdio,mutations,parse_errors,render_variant,stdio_contract}.py`; none overlaps this slice. Apply this commit on top of that head during integration and preserve those five caller-cwd/Cargo-target corrections. The new GitHub CI run was pending at this handoff; this worktree remains on the requested base.

The demo now invokes the published render CLI with the evaluated run, all three variant IDs, expected revision and an explicit printed idempotency key. It checks the retained render record, source manifest digest, result statuses, verifier metadata and listed artifact bytes. Both comparison formats are preserved. A failed result is reported with its code and retained receipt path. Verified output reaches an explicit human review boundary; it does not select, export or approve anything. The optional candidate selector is checked against the evaluated IDs. The injected protocol fixture exercises only the Python driver's success and rejection branches; its bytes are labeled fixture data and are not Blender or native acceptance evidence.

## Environment and real CLI evidence

- Worktree: `/home/butahlecoq/work/archy/.worktrees/a18-demo-continuation-20261030`, ext4 `/dev/sdd`. WSL2 kernel `6.6.87.2-microsoft-standard-WSL2`; Rust/Cargo 1.99.1; Python 3.14.4. `/usr/lib/archy/archy-service-gate` and `/usr/bin/blender` are absent.
- Dedicated `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a18-demo-continuation-20261030`, `TMPDIR=$CARGO_TARGET_DIR/tmp`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0` for Cargo. `cargo build --offline --locked -p archy-cli`: exit 0.
- `archy doctor --json`: exit 1, `target_acceptance: deferred`; Omarchy unavailable, OS unsupported for target acceptance, Blender and IFC backends missing. This is a WSL source check.
- Real command: `python3 -B scripts/demo.py --archy $CARGO_TARGET_DIR/debug/archy --project-dir $CARGO_TARGET_DIR/evidence-project-final` with the dedicated `TMPDIR`: exit **2**. The generated, evaluated, rendered and comparison commands all exited 0; the demo stopped on three retained `BACKEND_UNAVAILABLE` render results. The project has three run directories, both comparison copies and zero GLB, PNG or IFC files. It did not claim review readiness.
- Local evidence outside the repository: `$CARGO_TARGET_DIR/demo-final.log` SHA-256 `3d9a7427790fdc4ae0874442ea8f5a4f3d4a9531ce4f9b1ee6cc041b66789c22`; render receipt `$CARGO_TARGET_DIR/evidence-project-final/runs/02688eb2-7ab3-4447-be70-5df77e2dcc82/render.json` SHA-256 `1a956691e7df15d698e44d34ffac44ca7875b6bf7db4a508117c1dff46fbead7`; comparison JSON SHA-256 `69e61497f4e5541eda6eba30f508e4e55415d44e740d11a179a9ea3bf491b991`. The paths are local review evidence, not published assets.

## Verification and remaining gates

Demo tests cover real absent-gate behavior and exact-command idempotent replay, retained history, collisions, malformed evaluation/render receipts, structured errors, artifact tampering and candidate selection. The synthetic success branch asserts three reported GLB and nine PNG paths and the stop before human approval. Schema CI calls this same demo test directory from `scripts/check-schemas.py`; it also builds and probes MCP contracts. Final command results and commit hash are recorded in the final handoff response.

`ARCHY_BIN=$CARGO_TARGET_DIR/debug/archy PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest discover -s tests/demo -v` with the dedicated `TMPDIR`: final exit 0, eight tests in 90.163 seconds on this shared WSL host. `git diff --check` and Python AST parsing of the three changed Python files: exit 0. These tests do not constitute a throughput benchmark.

`PYTHONDONTWRITEBYTECODE=1 python3 -B scripts/check-schemas.py` with the same dedicated Cargo and temporary directories: exit 0. It passed 50 serde/domain cases, nine MCP schema pairs, live MCP read/mutation/render/replay probes, 26 negative CLI fixtures, doctor and desktop schema checks, and all eight demo tests (92.749 seconds inside this CI path). This is the current schema CI step; no workflow edit was needed.

`cargo test --offline --locked -p archy-cli --test render_publication` with the same Cargo controls: exit 0, four tests passed, including absent-gate retained failures, exact replay, and incomplete-selector rejection. No pinned Blender/gate success was available on this host.

An installed pinned Blender gate, visual inspection, target Omarchy containment, IFC export and independent IFC parsing remain unobserved. No regulatory or professional compliance claim follows from these geometric outputs. Issue #18 remains open pending those gates and human selection/approval.
