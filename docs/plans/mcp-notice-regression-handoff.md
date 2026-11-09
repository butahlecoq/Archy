# MCP notice coverage regression: isolated handoff for Sol review

## Scope and plan

This worktree starts at `d356133b7311bb05104837be6a356ee3574fe581`. I copied only the provisional root `Cargo.toml`, `Cargo.lock`, and `crates/archy-mcp/` from `a15-integration-20261029` into this worktree to reproduce the merged workspace graph. Those copied files are **test context**, not part of the notice fix to transplant. No other checkout or worktree was written. The bounded plan was to inspect A21's reviewed notice/package flow, reproduce the vendor and staging failures, patch only the coverage boundary, and validate the real offline inputs.

## Diagnosis

The prior A21 private source captured `/tmp/archy-core-arch-source/vendor`, with 89 locked normal CLI registry crates. Its notice inventory and bundle hashes are `e4d95486832a64364e38fe4b565369ad8d075b6529edbf4d84973e69110cffd6` and `ea7019a4ff8e69a9b0d49c4e54ab5869c1c2a0726997c7c9e30cd425402da710`. With the provisional MCP manifests, admission against that vendor directory fails on missing `chrono-0.4.45` and writes zero files. This is a stale vendor snapshot, not evidence that the two new crates lack notice texts.

`cargo vendor --offline --versioned-dirs` using the provisional lock succeeded from the locally cached registry into an isolated directory. Fresh admission finds 91 crates. `chrono 0.4.45` supplies `LICENSE.txt` (SHA-256 `946c9835d8034d24404f8cfec5f4654cee5dad17e944afc3d06d742cf2882831`); `iana-time-zone 0.1.65` supplies `LICENSE-APACHE` (`696759d65dfe558ff7d9f031c76db19ec5c0767470fb67c4e8d990820d1e99c9`) and `LICENSE-MIT` (`da28ccc6b158fc2d8cccc74e99794b1cff1d29bd7bbeb019442fcf0c04c6cad9`). Both Cargo metadata license fields say `MIT OR Apache-2.0`; this records metadata and supplied text, not a new legal determination. Fresh inventory SHA-256 is `215d1f061bfd35362cdd0ffaf7a596c434d1e3bd30355f62bac7a249efaa07bd`; bundle SHA-256 is `bb7333062a0d0885d0cc4bde6370374d72760fe515f0a2513cce79fc2dd09184`.

The separate regression was in `package.py stage`: its code checked only that the old 89-crate bundle hash matched its own JSON. A focused test showed it returned success for stale coverage. Stage now recomputes the offline, locked, filtered `archy-cli` normal graph and compares the set of `(name, version, Cargo.lock source checksum)` with inventory rows, rejecting missing, extra, or duplicate rows before the first destination write. The graph traversal was moved unchanged from `notices.py` to `package.py` so admission and stage use one definition. No dependency or notice terms were changed by the fix.

## Validation

- Before the fix, the new stale-coverage test failed because stage returned zero. After the fix, `python3 -B -m unittest discover -s tests/packaging -q` passed 24 tests.
- Actual stage with the previous 89-crate files exits with `notice dependency coverage mismatch` and leaves zero destination files. Actual stage with the fresh 91-crate pair stages 19 manifest paths. A second stage using the locally built `target/debug/archy` stages 19 paths, with a byte-identical binary and inventory count 91.
- Offline `cargo build --locked -p archy-cli` with the fresh vendored directory and an independent target completed in 1m 16s. `git diff --check` on the four fix files passed.
- No Arch `makepkg`, pacman transaction, Omarchy installation, release archive, full linked-code license audit, or external upstream provenance recheck was performed. The earlier A21 package review's target and legal limitations still apply.

## Exact review surface

Apply only the diffs for `packaging/arch/notices.py`, `packaging/arch/package.py`, `tests/packaging/test_notices.py`, and `tests/packaging/test_package.py`. Their current SHA-256 values, respectively, are `5c56fae3c62882c2090c29fa31fd8b1e778324f372ff9aaaf51c0db770a8dc5b`, `8971fd67acc613c4a0fca715bc4ed7965bed96770788ee3fc9124642ea4eb856`, `9b9f14cf377f301f97584108d665df564514c1fcc870c42acf880d3a19d368a1`, and `d66dbe19d80a9b4acec94d0203ad3269b17d20d20e938741be8d5b97906659b6`. The ordered `git diff --binary -- packaging/arch/notices.py packaging/arch/package.py tests/packaging/test_notices.py tests/packaging/test_package.py` has SHA-256 `82983a8dba3288302dd96001b7b04e719ead16d2109e9156d601d14123231727`.

The provisional `Cargo.toml` and `Cargo.lock` SHA-256 values are `51f3cb607ef314327f30551d33c0d244c6c0e9fd8c07e76f4e7897bfe2db2c6` and `6866671914d12c1e0a39e3b72505be5056d24d1464823618c866096f2fc67fde`. They and the copied MCP crate must be reconciled with the integration owner's final source separately. Plato's gate changes independently touch `notices.py`, `package.py`, and related tests. During reconciliation, preserve the shared graph definition, stage coverage check before writes, and the gate's executable behavior; rerun admission and staging on the final merged lock. This worktree does not include or review Plato's gate changes. Independent Sol review remains pending.
