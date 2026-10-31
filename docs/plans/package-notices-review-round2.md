# A21 notice admission rereview, round 2

**Scoped verdict: PASS.** Both P2 findings in [round 1](package-notices-review-round1.md) are addressed in the frozen A21 worktree. I found no new actionable regression in the reviewed notice staging changes. Base HEAD was `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`.

## Finding dispositions

- **Missing notice pair: ADDRESSED.** [`package.py`](../../packaging/arch/package.py) requires both reserved destinations before staging and checks the archive's manifest for exactly one correctly mapped row for each before emitting a recipe. My disposable probes removed both rows, removed either one, changed a mode, duplicated a row, and changed a mapping. All six cases returned exit 1 from both stage and recipe generation. Stage created no `usr` tree, and recipe created no output. A correctly mapped pair generated a recipe with exit 0. Recipe's pair check does not replace the stage parser's full manifest and collision checks.
- **Notice pair changed after validation: ADDRESSED.** Stage reads the bounded notice and JSON bytes once, verifies their digest relationship, and copies those captured bytes. I changed both source files at the first `open_stage_parent` call, after validation and before file creation. Stage succeeded with the original bytes for both files. The SHA-256 in the staged JSON matched the staged bundle. This independently exercises the round-1 substitution point; no product source was changed by the probe.

## Independent checks

`python3 -B -m unittest discover -s tests/packaging -q` exited 0 with 23 tests. Separate disposable probes rejected malformed inventory JSON, a wrong bundle hash, a symlink notice input, a symlink destination parent, and a preexisting target. The outside sentinel and existing target survived. Both stage inputs at one byte over their limits were rejected before stage writes. The bounded reader accepted exactly 16 MiB for the bundle and exactly 2 MiB for the inventory, and rejected one byte more. A source archive with an over-2-MiB manifest was rejected without a recipe. The focused suite also exercises rollback and the original parent substitution cases.

With `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a21-review`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, and `RUSTUP_AUTO_INSTALL=0`, I ran the real `notices.py` admission against `/tmp/archy-core-arch-source/vendor` into a disposable directory. It exited 0. A separate traversal of locked, offline x86_64 Cargo metadata found the same 89 unique registry crate pairs as the inventory. Their Cargo.lock package checksums matched the vendor checksum manifests, and each inventoried text's bytes matched its SHA-256. The inventory has 173 crate texts, including seven primary texts, and three Rust 1.99.1 texts. Its SHA-256 is `e4d95486832a64364e38fe4b565369ad8d075b6529edbf4d84973e69110cffd6`; the bundle SHA-256 is `ea7019a4ff8e69a9b0d49c4e54ab5869c1c2a0726997c7c9e30cd425402da710`, matching the inventory.

I staged those actual outputs with the frozen `files.txt` and a read-only publication binary into a disposable root. Stage exited 0 with 19 owned paths. Every staged file matched its stage inventory hash, and both notice files matched the admitted bytes. Simulated removal of those owned paths preserved an unrelated project sentinel. All seven primary files matched the supplied provenance file and input manifest by bytes, SHA-256, Git blob, and provenance fields; all three Rust files matched their input-manifest hashes. The upstream Git trees were not fetched during this rereview.

This is a source and disposable WSL review. It does not establish a new Arch package, linked-code or legal distribution audit, pacman transaction, or Omarchy target acceptance. The previous private package remains a separate artifact. No product source, main checkout, commit, push, child agent, or host installation was changed.

## Frozen source hashes

These SHA-256 values identify the reviewed worktree files and inputs:

| Path | SHA-256 |
| --- | --- |
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `packaging/arch/package.py` | `130719a86be79c5f181ad21ce30c9272ad79b3ff0790cd9f64101c504db7d7d7` |
| `packaging/arch/notices.py` | `e194be3a5383022c34aa886b6c9c44d3cc68f486d8edbc52c08f506f283e9f43` |
| `packaging/arch/files.txt` | `3649ef26e5e373f58ed1a50d663a215763f8dea4022c58f4a4b0e199e5166452` |
| `packaging/arch/notice-inputs.json` | `218fbaaee3f49a6dd6fc240b09c7ab4b45fb02f9b4eca459d309913778acdc4e` |
| `tests/packaging/test_package.py` | `6f106e91f5ac491b18b353943028c80e14120539ce993c79b1976c2fd5c78ecf` |
| `tests/packaging/test_notices.py` | `283c1d8945d8446fdd2dff244e809cdbdbac56c78c370ae52e61e57d96978154` |
| `docs/plans/package-handoff.md` | `9bacfd84d602290060f826a9128cedc8bd2cd3281ae8c3be51eb642b1ba793b4` |
| `docs/omarchy/package.md` | `bc4419c2216b68e632cf26c89bbaefb8103364f581371d11f4c46cd0101f155e` |
| `docs/plans/package-notices-review-round1.md` | `12d3d5b4cbbb630906195a22b893be7bf39edd82b5a7e98f400b35877ce89fd6` |
| Supplied `primary-crate-notices/provenance.json` | `4628a969c8f17a359dd16a6ac410d1dddbc8ba80c0affc4187f75458fc23ab27` |

| `packaging/arch/notices-sources/` relative path | SHA-256 |
| --- | --- |
| `geo-0.33.1/LICENSE-APACHE` | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` |
| `geo-0.33.1/LICENSE-MIT` | `2f8267b247fd81e9555a2e196e119a5b341bcae21011cd93985c312f1312f727` |
| `i_key_sort-0.10.3/LICENSE` | `47e00f18b265d350a2860d3899f17dd1c7689b966545e929df8ffa7033f5acdc` |
| `i_overlay-4.5.2/LICENSE-APACHE` | `59899c6091b540582ed617e8eeaac4919dc985ccfc35459ee9752b699be5205b` |
| `i_overlay-4.5.2/LICENSE-MIT` | `260cc580f53a756821a0f35475e2cd6da790f8fed0efc058252fe1bed8f2689e` |
| `rstar-0.12.2/LICENSE-APACHE` | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` |
| `rstar-0.12.2/LICENSE-MIT` | `9b3a76c27bac8f3e0ff2d1f64f65941c50310ae3ff69318cea230bd4a36822c5` |
| `rust-1.99.1/Apache-2.0.txt` | `074e6e32c86a4c0ef8b3ed25b721ca23aca83df277cd88106ef7177c354615ff` |
| `rust-1.99.1/COPYRIGHT-library.html` | `5647be074c8edf7339fd863055923a8fc80bc5610a8d4661ec3b767b9d392c27` |
| `rust-1.99.1/MIT.txt` | `b85dcd3e453d05982552c52b5fc9e0bdd6d23c6f8e844b984a88af32570b0cc0` |

Boundary Discipline guided the checks at the manifest and captured-byte boundaries. Prove It Works guided the disposable admission and staging probes. The interrogate review used run-backed findings without child agents.
