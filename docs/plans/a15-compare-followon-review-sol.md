# A15 compare follow-on: independent scoped review

29 October 2026. **Verdict: scoped PASS for the staged live read-only `compare_variants` follow-on.** The exact patch is ready for integration on base `f1e2477047dbafbacfba3229daddab0ca58ebe02`. This verdict covers comparison reads and their preservation of the published checks. A15's five mutation tools, approval workflow, and full ticket acceptance remain open.

## Identity and reconciliation

I read `AGENTS.md`, `docs/plans/a15-compare-followon-handoff.md`, the 11-entry follow-on freeze, and the prior independent `.worktrees/a15-compare-integration-20261029/docs/plans/a15-compare-review-sol.md`. The prior review SHA-256 is `3321918a9748db1b76092e43bbcb188de867bdb4c8023d90d2983c32cae5ad50`; the original 37-entry freeze SHA-256 is `f694d40b6d125a75b25081fc9e47ddb8dd6209d26ec9884e1e23fa7f57ef8ee`.

`git rev-parse HEAD` returned the exact base above. `git diff --cached --binary | sha256sum` returned **`1eaeb6591e80624992dbd67049baebd5053a75ff0d3bddbf607c0074eddaa9b2`**. `sha256sum -c docs/plans/a15-compare-followon-freeze.sha256` returned **11/11 OK**. The freeze manifest SHA-256 is `91047791a4881c429cc5eab9a5ea13e9a9187583a221600588c29d47a015688`; the handoff SHA-256 is `730a0b527a9b1cb41042a4fdf00542e6f932607e4df52106f2e29f624438821c`.

I compared bytes of every frozen path with the reviewed original worktree and `git show HEAD:<path>`. Ten paths are byte-identical to the reviewed original: `Cargo.lock`, CLI `compare.rs`/`lib.rs`, MCP `Cargo.toml`/`server.rs`/`tools.rs`, MCP `stdio_contract.py`/`compare_stdio.py`, `docs/mcp/README.md`, and the comparison schema. None is identical to the base, as expected. The eleventh, `scripts/check-schemas.py`, differs from the base by exactly one tuple entry, `compare_stdio.py`, retaining the base A14 decision probe, both existing MCP stdio suites, and the A18 demo suite. `git diff --cached --check` passed.

The published-base A14 `tests/decision_contract_schema.py` (`ab5d4ae87545ca61edc0afc7c34d91eee84c1c4fb43b5fd7fe9ac53712b2772d`), A18 `tests/demo/test_demo.py` (`b7aa540e205df1b2b268c1bd890ac7b688c93c7e561ca62cd1096ba5eb6b4`), MCP `parse_errors.py` (`ba5dbf7e1b68f305d2e60fc12ab89d0c09d03193df5c86ad5b5b285fd792343c`), A21 `notices.py` (`5c56fae3c62882c2090c29fa31fd8b1e778324f372ff9aaaf51c0db770a8dc5b`), `package.py` (`8971fd67acc613c4a0fca715bc4ed7965bed96770788ee3fc9124642ea4eb856`), and both packaging test files (`9b9f14cf377f301f97584108d665df564514c1fcc870c42acf880d3a19d368a1`, `d66dbe19d80a9b4acec94d0203ad3269b17d20d20e938741be8d5b97906659b6`) are byte-identical to `HEAD`. The unchanged read/verification boundaries are `project_read.rs` `d6dbf17b5906b7c3f3e1ee5064c37927b8432b9dde54c2a39971e8c6867ca1f5` and `run_integrity.rs` `d6dfa2072632bc218ef1b01201aa616530c990a73ced14a4db8eae94c6669b43`.

## Live behavior and boundaries

The handler takes a pinned `BoundProjectRead` snapshot, verifies the accepted head and each selected run, and calls the CLI's shared `render_verified_comparison`. In my separate real stdio probe, three newly generated accepted runs were requested in reversed caller order over one-run pages. All three returned documents equaled actual `archy compare --json --run <id>` results. Candidate objective values were ascending inside each document, and each listed `daylight` and `cost` as uncomputed. The response contains separate documents and `source_run_id` values; it supplies **per-run dense ranks, not a cross-run ranking**. The README says the same. An unsupported `cost` objective returned `INVALID_SCHEMA`.

The independent probe exercised malformed cursor offsets and a changed last-run token, a changed ordered selection, a 101-ID selection, a 129-byte ID, and limit 101. The malformed offsets/token returned `INVALID_SCHEMA`; changed selection returned `REVISION_CONFLICT`; schema-bound inputs returned JSON-RPC `-32602`. A page cursor remained usable with a different page limit, consistent with its binding to selection and revision rather than page size. After a new accepted run changed the head, the old cursor returned `REVISION_CONFLICT`. Corrupting an accepted manifest returned `ARTIFACT_INVALID`. Full project-tree mode, size, nanosecond mtime and SHA-256 inventories were unchanged around successful and failed MCP reads. The checked-in live suite additionally exercised duplicate/unaccepted IDs, wrong operation, consistently rehashed metric forgery, and pinned-source replacement.

## Commands and results

All Rust/build-driven commands used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-compare-followon-20261029`; Python probes used `PYTHONDONTWRITEBYTECODE=1` and `-B` where shown.

```text
git rev-parse HEAD
git diff --cached --binary | sha256sum
sha256sum -c docs/plans/a15-compare-followon-freeze.sha256
git diff --cached --check
  exact base and patch hash above; 11/11 OK; diff check exit 0
PYTHONDONTWRITEBYTECODE=1 python3 -B scripts/check-schemas.py
  exit 0: nine MCP schema pairs; A14 50 cases; modern/legacy,
  parse/framing and compare live stdio; A18 six demo tests
PYTHONDONTWRITEBYTECODE=1 python3 -B /tmp/archy-a15-followon-sol-probe.py
  exit 0: 3/3 CLI parity, caller order, ranking, cursor bounds,
  stale head, tamper refusal, unchanged inventories
cargo test --locked --offline -p archy-mcp --test bounds --test framing -p archy-cli --test compare --test desktop_readers
  exit 0: 6 bounds, 3 framing, 4 CLI compare, 7 desktop reader tests
PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest discover -s tests/packaging -q
  exit 0: 24 tests
node crates/archy-mcp/tests/unicode_regex.js
  exit 0: eight Unicode fields
cargo fmt --all -- --check
  exit 0
python3 packaging/arch/notices.py --source . --vendor /home/butahlecoq/work/archy/.tools/targets/a15-a21-integration-20261029-vendor --output-dir /home/butahlecoq/work/archy/.tools/targets/a15-compare-followon-sol-notices
  exit 0: 91/91 registry rows; inventory SHA-256
  215d1f061bfd35362cdd0ffaf7a596c434d1e3bd30355f62bac7a249efaa07bd;
  bundle SHA-256 bb7333062a0d0885d0cc4bde6370374d72760fe515f0a2513cce79fc2dd09184
```

The independent probe script SHA-256 is `a8980d41ec3c0d0fd2c3b0f391c3cdb192429e7f5120d0fc59ea5e9db30b29e3`, retained under `/tmp`. Its first two runs incorrectly expected a handler error for 101 IDs; the schema correctly returned `-32602`, and the corrected run passed. The first notice attempt used an older vendor tree missing `chrono-0.4.45`; the next used the correct vendor tree but lacked the required precreated output directory. After creating that directory, admission passed with hashes identical to the handoff. These were probe setup errors.

## Limits and A02 overlap

The published base already routes historical verification through `parse_project_v1`; current `project_read.rs` still uses `parse_project` and exposes the v1 `NormalizedSpec`. The separate A02 handoff scopes its work to historical v1 verification and says current v2 authoring/admission, normalized DTO, report versioning and exhaustive historical profile dispatch remain future work. This follow-on preserves v1 accepted bytes and does not validate that future migration. When A02 changes current admission, update the MCP normalized output schema and test live current/v1 historical comparison parity; keep v1 recomputation and report bytes versioned. No current source collision or failing gate was found here.

The probe is a finite read/race sample, not exhaustive concurrency proof. No real Omarchy installation, backend render, package build, distribution, full A15 mutation path, or future A02 v2 path was exercised. No source, other worktree, commit, push or install was changed; this review file is the sole new review output in the target worktree.
