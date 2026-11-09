# A09 + A15 + A16 on 15446ee: independent integration review

30 October 2026. **Verdict: scoped pass for the frozen source integration.** I found no new reproducible defect in the combined accepted-run, service, packaging, authority, or live MCP paths checked below. This is not installed runtime, artifact publication, human decision, or Omarchy acceptance.

## Identity and freeze

I read `AGENTS.md`, `a09-a16-combined-15446-integration.md`, its freeze, and the original independent `a09-a15-160787-integration-review-sol.md`. `HEAD` is `15446eeaf316846936a093377041b8c1d2874092`. `sha256sum -c docs/plans/a09-a16-combined-15446-freeze.sha256` passed **28/28 entries** (27 product paths and the integration note); the freeze file SHA-256 is `d1384980b4a7235324cf9d9bbbfd8ce1c6b59d9f6bf1e2e6aad425dbd5d5deec`. Checking the original A09/A15 freeze against this tree passed **19/21**. Only `Cargo.toml` and `Cargo.lock` differ.

`git diff -- Cargo.toml Cargo.lock` shows the manifest retains A16's `authority` workspace member and adds `crates/archy-service-gate`. The lock retains the local `archy-authority` package, adds `archy-service-gate` with `rustix`, `serde`, `serde_json`, and `sha2`, and adds `archy-adapters` to `archy-cli`; no other lock section changed from this head. Locked offline Cargo metadata listed both local packages and all nine `archy-*` workspace packages. The older A16 26-entry freeze matches its 19 unaffected paths. Its seven differences are the expected manifest/lock, later A15 MCP server/tools/schema checker, and A09 package/notices files; its authority source, authority packaging inputs, A14 contract, and adapter process source still match. The combined 28-entry freeze separately covers the merged bytes, including MCP and authority source.

## Source and behavior review

`accepted_service_request` opens a bound project reader, locks the store, rechecks pinned root/source, semantically verifies accepted ancestry and run evidence, checks project ID, selects a variant from that run, derives the source digest from verified input, and builds an owned typed request before releasing the lock. The focused tests passed current and historical binding, foreign/orphan/absent/stale evidence refusal, rehashed semantic forgery refusal, and concurrent writer snapshot behavior. Neither this wrapper nor A16 ingress authenticates a human decision. The public adapter request constructors and `ServiceSession::start` still accept direct callers without accepted-run provenance.

The fixed gate checks seqpacket GO/request/EOF framing, duplicate JSON keys, backend/operation/output policy, and pinned runtime inventory before bwrap. Worker exit `0` requires `ok`; exit `20` requires `error`; other exits fail. The controller checks the captured response against exit status after stopping the unit. The service tests passed malformed/denied `cgroup.events` refusal, populated/inactive evidence, fixed properties, cgroup and process identity, cancellation and descendant pipe teardown. `stop` requires `ActiveState=inactive`, an empty well-formed bounded cgroup record or confirmed group disappearance, and reaps the systemd wrapper. If a collected unit has no observed cgroup path, teardown remains unverified and fails closed.

Artifact capture enforces fixed names, regular-file identity, per-file and aggregate bounds, byte digests and claims, and a pinned IFC output directory. Handled errors retain response bytes and expose a separate `scratch_issue`; IFC error binding may coexist with such an issue, so callers must inspect both. Blender handled errors remain unbound diagnostics. These bytes are neither published immutable run artifacts nor independent IFC verification.

The package diff stages the gate at `usr/lib/archy/archy-service-gate`, requires a static executable ELF in preflight, includes the CLI plus gate locked normal dependency graph in notices, and requires the gate row in the recipe. Packaging tests exercised static PIE admission, dynamic dependency rejection, notice coverage, destination rollback, and staged inventory. The A16 authority binaries and units are not installed by this recipe; the authority journal remains private proposal ingress, with no decision writer, session confirmation, or rollback anchor. A14 decision schemas represent claims and do not turn a matching digest into verified evaluation or human approval. MCP generate/evaluate and compare remain live; render, IFC export, and approval proposal return typed prerequisite errors.

## Independent checks

Rust checks used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-a16-15446-review-sol-20261030`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`, `--locked --offline`. Python used `PYTHONDONTWRITEBYTECODE=1` and `-B`. This target is separate from the coordinator's running combined check.

| Command or check | Observed result |
| --- | --- |
| `cargo test -p archy-adapters -p archy-service-gate -p archy-cli -p archy-authority -p archy-mcp --locked --offline` | Exit 0. Adapter lib 26, artifacts 9, isolation 4, protocol 8, authority lib 2, socket boundary 13, journal boundary 8, CLI accepted worker 4, desktop readers 7, comparison 4, core workflow 20, MCP bounds 6/framing 3, gate 8, and the other selected test groups passed. The native IFC protocol and live user-systemd receiver tests were ignored by their explicit environment gates. |
| `python3 -B -m unittest discover -s tests/packaging -q` | Exit 0, 32 tests. |
| `cargo fmt --all -- --check`; `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0 each. |
| `cargo build --locked --offline -p archy-cli -p archy-mcp -p archy-service-gate` | Exit 0 in the unique target. |
| `python3 -B tests/security/service_gate_unavailable.py <review debug gate>` | Exit 0, five negative prelaunch cases: bounded FD, unavailable result, collected inactive unit, unchanged host sentinel. This dynamically linked debug gate is not package admissible. |
| `python3 -B scripts/check-schemas.py` | Exit 0 once. Nine MCP input/output pairs; 16 source schema files; A14's 50 serde/domain cases; 26 CLI negative fixtures; live MCP modern/legacy stdio, parse errors, generate/evaluate/compare, historical reads, replay/conflict, paging, tamper and pinned-source refusal; six demo tests all passed. |

The MCP parse-error pipe backpressure check is an existing intermittent flake documented in the prior independent review. It **passed in this single schema run**; that result does not establish stability. A separate combined check session was running during this review; its results were not used as independent evidence.

## Limits

No positive installed controller → static gate → bwrap → native Blender/IFC worker run, forced native timeout/OOM/descendant teardown, clean Arch package build, complete runtime inventory, root-owned installation, immutable artifact publication, A13 independent IFC verification, or Omarchy acceptance was established. The direct adapter trust boundary, A16 human decision and external rollback anchor, and the documented uncooperative path-replacement race remain open. No source, integration note, other worktree, install, commit, push, or child agent was changed by this review.
