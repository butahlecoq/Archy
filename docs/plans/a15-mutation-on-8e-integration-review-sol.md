# A15 mutation on published comparison: independent integration review

29 October 2026. **Verdict: scoped PASS for this exact mutation integration on `8e36e11d1399dee37dc2b85a8e2225b473571602`.** I found no new reproducible defect in the integrated generate/evaluate path or regression in published comparison. This is not full A15 acceptance: render, IFC export, and approval remain prerequisite errors. The merge must retain `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1` and rerun combined-head gates, including the separately reviewed MCP pipe repair when integrated.

## Identity and source findings

I read `AGENTS.md`, the integration handoff, patch, freeze, the earlier A15 comparison reviews and published integration note, and the relevant CLI, core storage, MCP, schema runner, wire tests, and public MCP README. The prior mutation round-two review and its 17-path source freeze are described in the handoff but are not present in this worktree, so I did not independently recheck their source worktree. My verdict covers the integrated files here.

`git rev-parse HEAD` returned the exact base above. `sha256sum -c docs/plans/a15-mutation-on-8e-freeze.sha256` passed **15/15**; the freeze manifest SHA-256 is `44d3cfcf321182a9da261572db3e7f7d723b482de5b6187d516aafd8b2716be8`. The frozen patch SHA-256 is `e30ab43a9046eda1e74e4256c8b0de7d223f0b257702c64d616a3e866d68993d`. `git apply --check --reverse` for that patch and `git diff --check` both exited 0. The patch includes the untracked `mutations.py`; `git diff` alone omits it. The 15 hashes establish the current exact files, not the unexamined original 17-path freeze.

Published CLI comparison (`compare.rs`, SHA-256 `108b527be2d28a04be08f3ba323ab77b619d87d29ce1456a95ac1c0607060cc7`), MCP comparison wire test (`compare_stdio.py`, `4f9c261d5ce9484e2c5b594b5018fdbfa0b9ac0546e9c0c3205ee62a7efa8631`), and comparison schema (`f542b0bfbf0c8ff1cc313aba4a84a365c37c7051986422aad747c8c14121c1d7`) have no diff from `8e36e11`. The comparison handler and `CompareVariantsData` remain in the shared MCP files; the changes add mutation dispatch, a mutation receipt type, and serialization. The schema runner's only diff is appending `mutations.py` after its existing stdio, parse, and comparison probes. `parse_errors.py` is byte-identical to the base at `ba5dbf7e1b68f305d2e60fc12ab89d0c09d03193df5c86ad5b5b285fd792343c`; the independent pipe fix was not copied.

`8e36e11` is an ancestor of `ff7f6bc`. That later commit changes `crates/archy-adapters/src/process.rs` and four fixture review documents, with no path overlap with this patch. It is absent from this exact-base worktree and must survive the subsequent merge.

## Transaction and lifecycle findings

The MCP server accepts only its launch-bound project, validates tool inputs, routes generate/evaluate through the shared `BoundProjectRead` and locked CLI transactions, and serializes mutation calls with a mutex plus the store lock. It returns the typed `MutationData` receipt; three other planned mutation tools still return typed prerequisites. The advertised mutation schemas match live SDK descriptors and wire output. Evaluation verifies its accepted source run and chosen variant before `locked.begin` creates staging for a new key. The storage transaction validates destination components, writes by pinned directory descriptors, publishes with no-replace rename, and records a revision only after a run is staged and synced. Replay verifies the accepted run before returning its receipt.

The real `mutations.py` stdio run passed generate → evaluate → compare, historical revision read, concurrent identical requests, restart replay, changed-key and stale-revision conflicts, destination traversal and symlink refusals, invalid selector with unchanged staging entries, tamper refusal, orphan recovery, and source/root replacement during active staging. The separate real `compare_stdio.py` run passed CLI parity, paging, tamper refusal, fixed-source checks, and no-write checks. The core test `binding_swap_at_publication_boundaries_never_returns_success` passed all four synchronized combinations: source/root replacement at the pre-run rename check and after run publication. The former leaves revision 0 and retries as a new run; the latter reports retriable uncertainty, then same-key replay after restoration produces exactly one accepted receipt.

These checks constrain **cooperating** operations that use the store lock and preserve a stable configured binding, and they detect sampled external swaps at the checked phases. They do not give the pathname an atomic lease. An **uncooperative** rename between an identity check and publication or response can escape detection even though writes remain pinned to the original root descriptor. The README states that a success receipt is about the pinned project and observed checks, not continuous identity of the configured pathname. This is a residual TOCTOU limit, not evidence that cooperating calls can redirect writes into a replacement root.

## Commands and outcomes

All Rust and live MCP commands used `RUSTUP_AUTO_INSTALL=0`, `CARGO_NET_OFFLINE=true` where Cargo was invoked, and the unique `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-mutation-review-sol-20261029`. Python runs used `PYTHONDONTWRITEBYTECODE=1` and `-B`.

| Check | Observed result |
| --- | --- |
| `cargo test --locked --offline -p archy-core -p archy-mcp -p archy-cli` | Exit 0. Included 20 CLI workflow tests, four comparison tests, 8 core interruption tests, 21 storage tests, 6 MCP bounds tests, and 3 MCP framing tests. The 64-run CLI history test passed in 151.05 s. |
| `python3 -B crates/archy-mcp/tests/mutations.py` | Exit 0; real generate/evaluate/compare, historical read, replay/conflict, scope, tamper, staged swaps, and recovery passed. |
| `python3 -B crates/archy-mcp/tests/compare_stdio.py` | Exit 0; published comparison parity, paging, tamper refusal, fixed source, and no writes passed. |
| `python3 -B scripts/check-schemas.py` | Exit 0 in this review: 9 MCP schema pairs, 50 decision cases, 16 schema files, stdio parse/compare/mutation probes, and six A18 demo tests passed. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `cargo fmt --all -- --check`; `git diff --check` | Both exit 0. |

The handoff records an earlier **failed** schema run at the unchanged `parse_errors.py:208` stdout backpressure assertion: observed `available=60482`, `capacity=65536`, required `61440`. That run stopped before compare, mutation, and A18 subprocesses; the handoff says those were run directly. It also records a later passing full schema retry. This review's passing full run confirms the gate can pass but does not remove the intermittent failure. Do not describe the original run as green, waive it, or silently copy the separate unreviewed pipe fix.

The target path is unique, but `df` places it on `/dev/sdd` ext4 and `lsblk` reports `ROTA=1`; this host does not verify it as SSD. No source files, other worktrees, commits, pushes, installs, or child agents were changed for this review. This review document is the sole worktree edit.
