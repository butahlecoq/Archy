# A10 render publication integration candidate

Date: 2026-10-30. Own worktree: `.worktrees/a10-render-publication-integration-20261030`, branch `codex/a10-render-publication-integration-20261030`, based exactly on published `e61c977d3e8b504dfafd2e81689e97f62c934e76`. No A10 commit, push, installation, or child agent. This published head includes the reviewed A18 and A20 documentation, A22 verified artifact opener/installer, and scoped MCP active-swap timeout fix.

## Source and reconciliation

The original candidate at `.worktrees/a10-render-publication-20261029` supplied a 40-path SHA freeze and an eight-file real Blender fixture freeze. Both manifests checked cleanly before integration (40/40 and 8/8). The original manifest records that source state, not the integrated tree; the integrated freeze is separate.

The candidate was based on `ff7f6bc`. The later published head overlaps 22 of its 40 frozen paths. In the three-way carry, the only textual conflicts were `Cargo.toml` (workspace members and line endings), `crates/archy-cli/src/accepted_worker.rs` (new render helper beside published worker service logic), and `crates/archy-cli/src/project_read.rs` (the same locked-run reader method present on both sides and nearby snapshot ordering). `Cargo.toml` kept both `authority` and `archy-service-gate`; `accepted_worker.rs` gained `run_blender_request`; `project_read.rs` kept the published implementation without duplicate methods. `Cargo.lock` was regenerated offline from the combined workspace. A09 service gate, adapter capture, A15/A16 authority, MCP, and published security/packaging files stayed at published content where identical; A10 render, reader, desktop status, CLI and replay semantics were added around them.

The combined notice graph covers normal dependencies of the CLI and service gate: 106 registry crates, including exact `png 0.18.1`, `gltf 1.4.1`, `gltf-json 1.4.1`, `gltf-derive 1.4.1`, `fdeflate 0.3.7`, and `flate2 1.1.10`. The published notice generator and packaging file inventory already include the service gate. The pinned PNG/GLB libraries enable independent verification of captured bytes; the installed Blender worker remains a separate service dependency.

## Validation on this worktree

The target `/home/butahlecoq/work/archy/.tools/targets/a10-integ-1e2313` and `TMPDIR=/home/butahlecoq/work/archy/.tools/tmp-a10i` are workspace backed, not `/tmp` tmpfs. Cargo used `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`, and `--locked --offline`.

- `cargo test -p archy-cli -p archy-adapters -p archy-service-gate -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable`: exit 0. This included 4 render verifier unit tests, 4 CLI render publication tests, 7 desktop reader tests, 4 accepted-worker tests, 26 adapter unit tests, 9 capture tests, 8 gate tests, and the remaining CLI suites. The 64-run history stress test was excluded; pinned native IFC and live systemd tests were ignored.
- `python3 -m unittest discover -s tests/packaging -p 'test_*.py' -q`: exit 0, 32 tests.
- `python3 scripts/check-schemas.py`: exit 0. Generated schemas/serializers, nine MCP pairs and live probes including backpressure and mutations, 26 negative CLI fixtures, and six demo tests passed.
- `cargo clippy -p archy-cli -p archy-adapters -p archy-service-gate --all-targets -- -D warnings`: exit 0.
- `cargo fmt --all -- --check` and `git diff --check`: exit 0 before the later source repair is imported.

## Earlier one-pixel repair and release limits

The independent rereview at `/home/butahlecoq/work/archy/.tools/github-updates/a10-render-rereview-sol.md` found that the frozen PNG verifier treats a white image with a single black first pixel as 262,143 foreground pixels. This integration has copied the source fix at SHA-256 `ba102b34c3e6437f589ceecb6e0f5ffdf90fed45a436c89df57911dffc6b1b08`: a median of composited border pixels supplies a stable reference, and the new test rejects a single black outlier at seven border/interior positions. The copied source is still provisional because the source candidate has not yet refreshed its 40-path manifest. Its previous freeze still records the pre-repair reader. The broad Rust and schema checks above ran before this import; the following focused checks ran afterward:

- `cargo test --locked --offline -p archy-cli --lib`: exit 0, 5 passed, including the outlier, real fixture, transparent PNG, JSON bound, and captured publication/replay tests.
- `cargo test --locked --offline -p archy-cli --test render_publication`: exit 0, 4 passed, including CLI render status, failed receipts and selector replay.
- `cargo clippy --locked --offline -p archy-cli -p archy-adapters -p archy-service-gate --all-targets -- -D warnings`: exit 0 after the copied repair.

At that provisional stage, the source SHA freeze and another independent review were still required.

This WSL2 host lacks `/usr/lib/archy/archy-service-gate` and `/usr/bin/blender`. The fail-closed missing-gate render test passed, but installed supervised success, positive systemd/cgroup teardown, full root-owned runtime inventory and vendored notice bundle, Omarchy execution, visual acceptance, measured parser peak memory, and full geometry/preview equivalence remain unproven. The real fixture demonstrates independent byte decoding and semantic checks, not an installed successful render. A failed immutable render receipt returns CLI exit 0 and reports `render_outcome: failed` in desktop status.

## Earlier one-pixel source repair reconciliation

The authoritative source freeze is now SHA-256 `c614f4e671c294131f8e871c8fca69dc9296bd24ce4b7e77af249954939b2295` and verifies all 40 source paths. The prior patch `f771a373cdb6700e74d7bec82c114d496bdb608a6a8b816bf0ad95e4c63596cd` preceded its final captured-worker regression. This integration now carries the source `render.rs` byte-for-byte at `5ddacedc0770ba68a8c2cb45a2aa8b9b0582154b71d49b5e45c27b5cfc0d081c` and `render_verify.rs` byte-for-byte at `ba102b34c3e6437f589ceecb6e0f5ffdf90fed45a436c89df57911dffc6b1b08`. The expanded captured-worker test rejects both invisible and one-pixel PNGs before publication and rejects a consistently rehashed one-pixel substitution during `verify_run` and idempotent replay. The source freeze and handoff are copied exactly. Of the 40 frozen source paths, 35 match byte-for-byte. Five differ intentionally: `Cargo.toml`, `Cargo.lock`, CLI `lib.rs` and `project_read.rs`, and core `storage.rs`, retaining published authority, service gate, and storage work.

The integration branch was advanced from `1e2313f` through `c8c8641` to `e61c977` by replaying a saved 28-path staged patch. The A18/A20 advance changes 14 documentation paths with no A10 overlap. The A22 advance changes 39 paths, overlapping A10 only in CLI `lib.rs` and `main.rs`; three-way application kept both the published desktop opener and A10 render command and exports. The earlier validation section records the original integration and provisional reader-import checks; the later broad checks ran before this A22 advance and before the gradient repair.

## Variable-background source repair before narrow-shape fix

The then-current source freeze was `44bcb42e4eeb0cd4df1245d9e7c3e58bd4300d4e907d3752364aa402c634ceb`; `sha256sum -c` passed all 40 paths in the source worktree. This integration copied exact source bytes for `render.rs` (`0286e5b606fe0ec23b4a73d65e430304bfa18456dc0642a4414c5a6f921a35d5`) and `render_verify.rs` (`23e33e7495f107ec170d55d34bc69d0d913014606711662cb7e8ded3ea36717f`), its freeze, and its handoff. The reader rejected the reviewed subject-free gradients and bands when their camera border varied, while the captured publication and rehashed replay test exercised those claims. A uniform-border fabricated interior remained possible; the check was bounded preview-profile evidence, not pixel-to-model parity.

Of the 40 source-frozen paths, 34 match byte-for-byte in this integration. The six intentional differences are `Cargo.toml`, `Cargo.lock`, CLI `lib.rs`, `main.rs`, and `project_read.rs`, plus core `storage.rs`. They retain published A09/A15/A16/A22 work and reconcile A10's module, command, and dependency edges. The A22 desktop opener and A10 render command both appear in CLI dispatch; their reader/export integration is tested separately below. The candidate remains uncommitted and unpublished while independent source rereview proceeds.

The integration freeze is `docs/plans/a10-render-publication-integration-freeze.sha256`: it hashes the other 27 A10 staged paths and excludes itself. The exact binary patch against `e61c977` is `/home/butahlecoq/work/archy/.tools/patches/a10-render-publication-integrated-e61c977.patch`. Its SHA-256 is reported with the patch outside this self-referential handoff.

## Narrow-shape source repair

The new authoritative source freeze is SHA-256 `0b99e1dc14139176ccdab76652476104c7e84e91df0cdc642292d907525872a2`; all 40 paths verify in the source worktree. This integration copies the exact `render_verify.rs` at `bb896e5902970be220a2bac3fb387ddf8398e95be65992bec089ae7695e56e6f`, the unchanged source `render.rs` at `0286e5b606fe0ec23b4a73d65e430304bfa18456dc0642a4414c5a6f921a35d5`, and the refreshed source freeze and handoff. The same 34 of 40 paths match byte-for-byte, with the six integration differences listed above.

The fixed 1% whole-frame foreground cutoff falsely rejected a valid 200 m × 2 m Blender top view. The verifier now requires at least 128 contrasting pixels, a bounding box of at least 2 × 16 pixels, and at least 2% contrast occupancy inside that box while retaining its 1% border-variation rejection. The source handoff records fresh pinned Blender runs for wide rectangle, deep L, ordinary rectangle and ordinary L, all three views passing the PNG profile; it also records negative synthetic and replay checks. Those PNG-only probes did not establish each fresh GLB's geometry parity. Independent source rereview is pending, and a fabricated interior with a uniform border can still pass without pixel-to-model proof.

## Checks after narrow-shape source import

Using the same offline Cargo environment and ext4 target described above, `cargo test --locked --offline -p archy-cli -p archy-adapters -p archy-service-gate -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable` exited 0. It includes 11 CLI library tests, four render publication tests, eight A22 desktop reader tests, four accepted-worker tests, 26 adapter unit tests, nine adapter capture tests, and eight service-gate tests. The 64-run stress case was filtered; native IFC and live systemd cases were ignored. `cargo clippy --locked --offline -p archy-cli -p archy-adapters -p archy-service-gate --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and the eight-file fixture SHA check exited 0. The A22 installer, plugin, and JavaScript checks above ran on the same published-base integration tree before this verifier-only import; their source files did not change.

`PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-schemas.py` exited 0 after this import. It includes 50 serde/domain cases, nine MCP schema pairs, live read, stdio and mutation probes including the published active-swap timeout case, 26 negative CLI fixtures, desktop open/status schemas, and six demo tests. The 32 packaging tests above ran before this verifier-only import; packaging source and locked dependencies did not change.

## Checks before the narrow-shape source fix

All Cargo commands used `--locked --offline`, `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a10-integ-1e2313`, workspace-backed `TMPDIR=/home/butahlecoq/work/archy/.tools/tmp-a10i`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, and `RUSTUP_AUTO_INSTALL=0`.

- `cargo test -p archy-cli -p archy-adapters -p archy-service-gate -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable`: exit 0. This includes eight CLI library tests (gradient/background, isolated pixel, exact fixture, captured publication and rehashed replay, A22 desktop opener), four render publication tests, eight desktop reader tests, four accepted-worker tests, adapter capture and gate suites. The unrelated 64-run history stress test was filtered; pinned IFC and live systemd tests were ignored.
- `cargo clippy -p archy-cli -p archy-adapters -p archy-service-gate --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and the eight-file fixture SHA check: exit 0.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/packaging -p 'test_*.py' -q`: exit 0, 32 tests.
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-schemas.py`: exit 0 on this final combined tree. It passed 50 serde/domain cases, nine MCP input/output schema pairs and live stdio/read/mutation probes, 26 negative CLI fixtures, the desktop open/status schemas, and six demo tests. The scoped MCP active-swap test runs through this checker.
- A22 controls on this combined tree: `cargo test -p archy-cli --test desktop_readers` exit 0, eight tests; Python installer/plugin tests exit 0, 19 tests; `node --test tests/omarchy/integration/*.test.js` exit 0, 17 tests. The desktop reader tests are also included in the broad Rust command above.

The host remains WSL2 with no installed service gate or `/usr/bin/blender`. These checks establish source, fixture, and fail-closed behavior, not installed supervised success, positive cgroup teardown, Omarchy execution, complete root-owned runtime inventory, visual acceptance, or pixel-to-model parity. An independent source review of this latest freeze is pending. No A10 publication or release approval follows from this integration candidate.
