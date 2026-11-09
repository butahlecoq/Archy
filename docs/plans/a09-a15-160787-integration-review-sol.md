# A09 + A15 on 160787: independent integration review

30 October 2026. **Verdict: scoped pass for this frozen source integration; production and Omarchy acceptance remain open.** I found no new reproducible integration defect in the accepted-run reader, A09 service boundary, artifact capture, packaging checks, or inherited live A15 mutation and comparison paths. This verdict does not authorize installed worker execution, artifact publication, or human approval.

## Identity and prior review

I read `AGENTS.md`, the handoff, patch and freeze, and the earlier A09 supervisor rounds 1–3 and A15 mutation/comparison reviews. `HEAD` is `160787d1f6c65a85d0e022a111d56ee7b0eb2b86`. `sha256sum -c docs/plans/a09-a15-160787-integration-freeze.sha256` passed **21/21**. The freeze file hashes to `bdb3146e6dde1ff02c3583d7a3e688dd18a757c3e022f0890cda28d919135f91`. The patch and `git diff --cached --binary` both hash to `da5644ad8c9b490c176c9c8ffc1582e79cda14b27e6d9bfb7f0d2f785fec34cb`; reverse apply check and staged diff check passed. The patch represents all 21 staged product paths; this report is the only review write.

The resolved `project_read.rs` diff adds only `with_verified_locked_run`. It takes the store lock, checks the pinned root and source, calls semantic `verify_locked`, checks the accepted run's project ID, builds the owned request while locked, and rechecks binding before returning. `verify_locked` recomputes accepted ancestry, source, normalization, generation/evaluation evidence, and artifact claims. The existing A15 `snapshot`, `check_binding`, and `transact` code remains in place. The wrapper derives run, variant, and source digest from accepted evidence; it does not authenticate a human decision. The public adapter request constructors and `ServiceSession::start` remain callable without that provenance check, so clients must use the CLI wrapper when accepted-run provenance is required.

The earlier A09 false-success cases for denied cgroup access and malformed `cgroup.events` are closed in this snapshot's source and focused tests. `cgroup_stopped` requires a readable, bounded, well-formed `populated 0` record or two `NotFound` observations for group disappearance. `stop` also requires `ActiveState=inactive` and reaps the systemd wrapper. If the unit is collected before any cgroup path was observed, `stop` returns unverified; that remains a fail-closed cleanup gap. The gate accepts only fixed backend and output policy, checks the request framing and runtime inventory, and maps worker exit `0` to `ok` and `20` to `error`; other exits fail. The controller checks that exit against typed captured response after teardown.

Successful capture checks exact allowed names, regular-file metadata, per-file and aggregate bounds, byte digests and claims, and stable file identity. IFC uses a pinned `ifc` directory. Error capture retains response bytes and reports a separate `scratch_issue` when partial output is unsafe; an IFC error may be request-bound even when that separate issue exists, so callers must inspect both fields. Blender errors are explicitly unbound diagnostics. Captured bytes are not immutable run artifacts or independent IFC verification.

Packaging adds the gate binary at `usr/lib/archy/archy-service-gate`, checks its ELF program headers for static execution, requires its row in staging/recipe, and extends notice coverage to the CLI and gate locked normal dependency graph. The local dynamic debug gate is suitable for the negative prelaunch probe only; it is not a package-admissible static gate.

## Checks on this worktree

Rust checks used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-a15-160787-review-sol-20261030`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`, `--locked --offline`. Python checks used `PYTHONDONTWRITEBYTECODE=1` and `-B`.

| Check | Observed result |
| --- | --- |
| Focused `cargo test` for adapter lib/artifacts, gate bin, CLI accepted worker and desktop readers | Exit 0: 26 + 9 + 8 + 4 + 7 passed. The worker artifact tests call the Python worker entry points with native backend generation stubbed. |
| `python3 -B -m unittest -q tests.packaging.test_package tests.packaging.test_notices` | Exit 0: 32 tests, including gate ELF admission, notice coverage, and staging. |
| `cargo build -p archy-cli -p archy-mcp -p archy-service-gate` | Exit 0, fresh unique target. |
| Live `mutations.py`, `compare_stdio.py`, and `stdio_contract.py` against those binaries | Exit 0 each: generate/evaluate/compare, replay/conflict and replacement; CLI comparison parity/paging/tamper/no-write; modern/legacy schemas, bounds, historical reads and pinned-source refusal. |
| `tests/security/service_gate_unavailable.py` against the debug gate | Exit 0: five negative prelaunch cases; bounded FD, unavailable response, collected unit, unchanged host sentinel. |
| `cargo fmt --all -- --check`; `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0 each. |
| Full `scripts/check-schemas.py` | Exit 0: nine MCP input/output pairs, 50 decision/domain cases, 16 schema files, 26 CLI negative fixtures, parse/framing, live compare and mutation, and six demo tests. The intermittent parse-error backpressure test passed once here. |

## Published A16 head and later integration

The user supplied published head `15446eeaf316846936a093377041b8c1d2874092`. I compared its Git objects with this exact base, without changing another worktree. Its product overlap with this 21-path patch is **only `Cargo.toml` and `Cargo.lock`**. A16 adds `authority` as a workspace member and the local `archy-authority` lock package; this patch adds `crates/archy-service-gate` as a workspace member, the local gate lock package, and `archy-adapters` to the CLI lock entry. Preserve both member additions and all three lock changes. The A16 source under `authority/` and its packaging files have no path overlap with this patch. A16 proposal ingress/journal is not final human approval; its own integration note says native distinct identities, session checks, installation, decision writing, and an independent rollback anchor remain open. Re-run merged-head tests and notice/package admission after combining them.

## Limits and next gate

The host lacks `/usr/lib/archy/archy-service-gate`, the runtime inventory, Blender, and IFC Python. No positive installed controller → static gate → bwrap → native worker run, forced timeout/OOM/descendant teardown, root-owned runtime install, complete transitive/font inventory, clean Arch package build, immutable artifact publication, A13 independent IFC verification, or Omarchy acceptance ran here. The direct adapter path has no accepted-run or human-approval provenance check. A15's `render_variant`, `export_ifc`, and `propose_approval` MCP tools remain typed prerequisites; live mutation/compare checks cover generate and evaluate only. The known MCP parse-error pipe backpressure test is intermittent; one full-schema result cannot establish its stability. The pinned-path checks also have the documented uncooperative rename race between observations. Do not promote this scoped pass into release acceptance.
