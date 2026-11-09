# Independent review: MCP notice coverage and staging

**Scope verdict: the 89-to-91 dependency-coverage regression is fixed for this exact worktree.** The old self-consistent 89-crate pair is rejected before destination writes; a fresh, independently generated 91-crate pair stages and verifies all 19 owned paths. One pre-existing trust limit remains: stage accepts a substituted bundle and inventory when their mutual digest and crate identity triples match. This review supports transplanting the bounded coverage fix with that limit stated; it does not certify arbitrary notice inputs, legal sufficiency, a final package, or Omarchy acceptance.

## Identity and scope

I read `AGENTS.md`, `docs/plans/mcp-notice-regression-handoff.md`, the previous A21 notice reviews (`package-notices-review-round2.md`, `package-notice-build-review.md`, and `package-notice-build-rereview.md`), the actual packaging code, and the focused tests. Worktree base HEAD was `d356133b7311bb05104837be6a356ee3574fe581`. The integration ref `codex/integration` was `ae6e6a6923fad5a4aae343cea67fcf40f62fd43e` during this review. The local `main` ref was `155df35690c7093c954529b62301caddd763bc55`. `ae6e6a6` itself changed service-controller files, not the notice files. The gate author's separate packaging changes are not in this worktree and were not reviewed here.

The intended transplant is only the four packaging code/test diffs below. `Cargo.toml`, `Cargo.lock`, and `crates/archy-mcp/` are provisional graph context copied for the test; they must be reconciled with the integration owner's final source. I made no source edits, commits, pushes, host install, or changes to another worktree. This review file is the only worktree addition.

| Input | SHA-256 |
| --- | --- |
| `packaging/arch/notices.py` | `5c56fae3c62882c2090c29fa31fd8b1e778324f372ff9aaaf51c0db770a8dc5b` |
| `packaging/arch/package.py` | `8971fd67acc613c4a0fca715bc4ed7965bed96770788ee3fc9124642ea4eb856` |
| `tests/packaging/test_notices.py` | `9b9f14cf377f301f97584108d665df564514c1fcc870c42acf880d3a19d368a1` |
| `tests/packaging/test_package.py` | `d66dbe19d80a9b4acec94d0203ad3269b17d20d20e938741be8d5b97906659b6` |
| Ordered `git diff --binary --` of those four paths | `82983a8dba3288302dd96001b7b04e719ead16d2109e9156d601d14123231727` |
| Provisional `Cargo.toml` | `51f3cb607ef314327f30551d33c0d244c6c0e9fd8c07e76f4e7897bfe2db2c6` |
| Provisional `Cargo.lock` | `6866671914d12c1e0a39e3b72505be5056d24d1464823618c866096f2fc67fde` |
| `packaging/arch/files.txt` | `3649ef26e5e373f58ed1a50d663a215763f8dea4022c58f4a4b0e199e5166452` |
| `packaging/arch/notice-inputs.json` | `218fbaaee3f49a6dd6fc240b09c7ab4b45fb02f9b4eca459d309913778acdc4e` |
| Binary used for stage probes | `3075cfb474bb6ca690e40af8c39a08dc088182b3a537ddade8b9197b2d2f7071` |

Hashes were checked with `sha256sum`; the diff hash used `git diff --binary -- packaging/arch/notices.py packaging/arch/package.py tests/packaging/test_notices.py tests/packaging/test_package.py | sha256sum`. `git diff --check --` on those four paths exited 0. A whole-worktree `git diff --check` reported trailing whitespace on three provisional `Cargo.toml` lines with CRLF endings (lines 2, 25, and 26); those manifest lines are outside the four-file notice transplant.

## Real dependency and artifact evidence

I ran, from this worktree, with `RUSTUP_TOOLCHAIN=1.99.1 CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0`:

```sh
cargo metadata --offline --locked --format-version 1 --filter-platform x86_64-unknown-linux-gnu > /tmp/a21-review-metadata.json
cargo vendor --offline --versioned-dirs /tmp/a21-sol-review/vendor > /tmp/a21-sol-review/vendor-config.toml
python3 -B packaging/arch/notices.py --source . --vendor /tmp/a21-sol-review/vendor --output-dir /tmp/a21-sol-review/fresh
sha256sum /tmp/a21-sol-review/fresh/*
```

Cargo metadata succeeded offline and locked. Fresh admission produced 91 unique normal `archy-cli` registry crates. Its inventory SHA-256 was `215d1f061bfd35362cdd0ffaf7a596c434d1e3bd30355f62bac7a249efaa07bd`; bundle SHA-256 was `bb7333062a0d0885d0cc4bde6370374d72760fe515f0a2513cce79fc2dd09184`. The inventory contained 169 vendor texts, seven primary texts, and three Rust standard-library texts. I separately compared every inventoried source checksum to `Cargo.lock` and the fresh vendor `.cargo-checksum.json`, and every vendor text hash to its vendored bytes and checksum manifest: zero mismatches. `chrono 0.4.45` has `LICENSE.txt` SHA-256 `946c9835d8034d24404f8cfec5f4654cee5dad17e944afc3d06d742cf2882831`; `iana-time-zone 0.1.65` has `LICENSE-APACHE` `696759d65dfe558ff7d9f031c76db19ec5c0767470fb67c4e8d990820d1e99c9` and `LICENSE-MIT` `da28ccc6b158fc2d8cccc74e99794b1cff1d29bd7bbeb019442fcf0c04c6cad9`.

Running the same admission with `--vendor /tmp/archy-core-arch-source/vendor --output-dir /tmp/a21-sol-review/stale-output` exited 1 with `No such file or directory: 'chrono-0.4.45'`; that output directory held zero files. The retained old pair at `/home/butahlecoq/work/archy/.tools/targets/a21-packaging-notices-admitted/` has 89 crates, inventory SHA-256 `e4d95486832a64364e38fe4b565369ad8d075b6529edbf4d84973e69110cffd6`, and bundle SHA-256 `ea7019a4ff8e69a9b0d49c4e54ab5869c1c2a0726997c7c9e30cd425402da710`. This is a stale vendor snapshot relative to the provisional MCP lock, not evidence of a missing notice text in the fresh dependency set.

I ran `python3 -B /tmp/a21-sol-review/probe.py` with the real source, `packaging/arch/files.txt`, the retained old pair, the fresh pair, and the binary identified above. The script creates a new disposable destination per case, checks for any failed-case writes, and on success checks all stage-inventory hashes and modes plus both staged notice bytes. Results:

| Case | Exit | Destination files | Stage inventory | Result |
| --- | ---: | ---: | --- | --- |
| Old 89-crate pair | 1 | 0 | absent | `notice dependency coverage mismatch` |
| Fresh 91-crate pair | 0 | 19 | present | all 19 paths, modes, hashes and notice bytes verified |
| Duplicate row, removed row, extra row | 1 each | 0 each | absent | coverage mismatch |
| Changed source checksum or version | 1 each | 0 each | absent | coverage mismatch |
| Wrong `crate_count` or unhashable row field | 1 each | 0 each | absent | invalid inventory |

Each failed case also preserved an unrelated destination sentinel. An independently filtered Windows Cargo metadata graph had 92 registry crates, versus 91 for `x86_64-unknown-linux-gnu`: Linux-only members included `iana-time-zone`, `libc`, and `linux-raw-sys`; Windows-only members included `anstyle-wincon`, `once_cell_polyfill`, `windows-link`, and `windows-sys`. Thus the fixed Linux target filter matters. Both notice admission and stage use the same `normal_registry_graph` function. This confirms the current graph comparison, not a full dependency-linkage audit.

`python3 -B /tmp/a21-sol-review/boundaries.py` additionally gave: oversized bundle (16 MiB + 1), oversized inventory (2 MiB + 1), and `../` manifest destination each exited 1 with zero files or stage inventory. With an empty `CARGO_HOME` and offline mode, stage failed at `cargo metadata --offline --locked` with exit 101, again before destination writes. The focused packaging suite, `python3 -B -m unittest discover -s tests/packaging -q`, exited 0: **24 tests**. Its stage tests include collision, symlink/parent substitution, captured notice bytes, and rollback of created files after an injected mid-copy failure. Those failure-atomicity checks passed; I did not inject a new post-write failure into the real 19-file stage run.

## Verified remaining limit: pair substitution

Stage compares only `(name, version, source_sha256)` for each crate to the locked graph, plus the inventory's `bundle_sha256` to the supplied bundle. In `probe.py`, replacing one crate's `texts` with `[]` while retaining all 91 identity triples exited 0 and staged 19 files. In `boundaries.py`, replacing the bundle with `b'forged notices\n'` and updating only `bundle_sha256` in the otherwise unchanged inventory also exited 0 and staged 19 files. Both outputs were byte-consistent with their supplied pair, but neither was freshly admitted against vendor data. These are run-backed limits of the stage trust boundary, not a regression introduced by the four-file fix. The generated recipe invokes admission before build and stage, but stage alone does not authenticate that an arbitrary pair came from admission. Do not describe this change as full notice-content or provenance validation at stage time.

## Merge implications and limits

Transplant only the four scoped diffs after reconciling the MCP graph with the integration owner's final manifests and lock. Plato's gate work independently touches `notices.py`, `package.py`, and related tests; resolve those overlaps deliberately, preserving the shared graph definition, the prewrite coverage check, and gate behavior. Re-run fresh offline locked admission and real stage with the final merged lock and vendor set. If the final graph differs from this provisional 91-crate graph, these bundle hashes and this verdict do not transfer automatically. The accepted pair-substitution limit remains a separate policy/design decision for the release flow.

No final Arch `makepkg`, pacman transaction, host or Omarchy install, target runtime check, upstream provenance recheck, complete linked-code inspection, or legal audit was performed. Earlier A21 package and legal limits still apply.
