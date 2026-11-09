# A21 notice admission independent review, round 1

The package maintainer needs a reproducible notice bundle for the locked core CLI dependencies and a stage step that cannot publish missing or inconsistent notice files. **Verdict: ISSUES.** The frozen source admits the 89-crate locked normal graph and its seven supplied primary texts, but two stage paths can still produce a package tree without a valid notice pair.

## Findings

### P2. Both notice manifest entries can be omitted

[`package.py`](../../packaging/arch/package.py) checks that `@notices` and `@notice-inventory` appear together (lines 172-183). It does not require either entry. The recipe runs `notices.py` before Cargo, yet `stage` accepts a changed `files.txt` that omits both generated files. An archive with that manifest would generate notices and then omit them from the package payload.

I ran `python3 packaging/arch/package.py stage --source . --binary /home/butahlecoq/work/archy/.tools/targets/reviewed-publication/debug/archy --manifest <disposable files.txt with the two notice lines removed> --dest <disposable dest>` in this worktree. The command exited 0, staged 17 owned files, and left both `usr/share/licenses/archy/THIRD-PARTY-NOTICES.txt` and `usr/share/archy/third-party-notices.json` absent. It printed no error. The expected result for a notice-enabled package source is refusal before any stage write. The disposable manifest was made by filtering only lines beginning `@notices\t` and `@notice-inventory\t` from the frozen `packaging/arch/files.txt`; no repository source changed. Require both reserved notice destinations for this package version, and make the recipe reject an archive manifest that omits them if the recipe is meant to establish that gate before build.

### P2. A changed notice input can pass preflight and stage an inconsistent pair

[`package.py`](../../packaging/arch/package.py) hashes `--notices-file` against `--notice-inventory-file` during preflight (lines 176-183), then reads each path again in the stage loop (lines 189-202). Those later reads are safe against symlink parents, but they are separate snapshots. I used a disposable in-process hook on `read_regular_nofollow` that changed only the notice text after its first read. `stage` exited successfully and staged `THIRD-PARTY-NOTICES.txt` with SHA-256 `7937c8086b8b692efb5ab2e2092f377bc3cdadd4aa44ba40c1a830d928382230`, while its staged inventory still declared bundle SHA-256 `40fbc1f8caf2e67bdb87a3b583282d1516952e1607bd41ab7d834844e21f622b`. The output reported `pair_matches: false`. Expected behavior is refusal and rollback, or staging the exact pair that passed preflight. This requires a concurrent change or another actor controlling a build input; the normal frozen admission and stage probe below produced a matching pair. Carry the validated bytes into the copy, or verify the staged pair before success with rollback on mismatch.

## Verified admission and scope

I ran `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a21-review CARGO_BUILD_JOBS=2 RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true python3 packaging/arch/notices.py --source . --vendor /tmp/archy-core-arch-source/vendor --output-dir <disposable directory under the target>` and got exit 0 with empty stderr. An independent `cargo metadata --offline --locked --format-version 1 --filter-platform x86_64-unknown-linux-gnu` traversal from `archy-cli` found 89 registry name/version pairs. The inventory had the same 89 unique pairs. Every row's `source_sha256` matched Cargo.lock and its vendored `.cargo-checksum.json`; each selected vendor or primary text matched its recorded SHA-256. The bundle contained 166 vendor and seven primary texts, plus three Rust 1.99.1 standard-library texts. The emitted JSON SHA-256 was `e4d95486832a64364e38fe4b565369ad8d075b6529edbf4d84973e69110cffd6`. The bundle SHA-256 was `ea7019a4ff8e69a9b0d49c4e54ab5869c1c2a0726997c7c9e30cd425402da710`, equal to `bundle_sha256` in that JSON.

I compared all seven copied primary files byte for byte with `/home/butahlecoq/work/archy/.tools/primary-crate-notices/provenance.json`. Each SHA-256, Git blob ID, repository, commit, source URL, and path matched the pinned manifest. All seven commits matched their crate's vendored `.cargo_vcs_info.json`. These are local provenance checks against the supplied evidence. I did not re-fetch or independently authenticate the upstream Git trees.

| Crate and upstream file | Verified SHA-256 |
| --- | --- |
| `geo 0.33.1 LICENSE-APACHE` | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` |
| `geo 0.33.1 LICENSE-MIT` | `2f8267b247fd81e9555a2e196e119a5b341bcae21011cd93985c312f1312f727` |
| `i_key_sort 0.10.3 LICENSE` | `47e00f18b265d350a2860d3899f17dd1c7689b966545e929df8ffa7033f5acdc` |
| `i_overlay 4.5.2 LICENSE-APACHE` | `59899c6091b540582ed617e8eeaac4919dc985ccfc35459ee9752b699be5205b` |
| `i_overlay 4.5.2 LICENSE-MIT` | `260cc580f53a756821a0f35475e2cd6da790f8fed0efc058252fe1bed8f2689e` |
| `rstar 0.12.2 LICENSE-APACHE` | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` |
| `rstar 0.12.2 LICENSE-MIT` | `9b3a76c27bac8f3e0ff2d1f64f65941c50310ae3ff69318cea230bd4a36822c5` |

The three pinned Rust text hashes also matched. `COPYRIGHT-library.html` was `5647be074c8edf7339fd863055923a8fc80bc5610a8d4661ec3b767b9d392c27`, `MIT.txt` was `b85dcd3e453d05982552c52b5fc9e0bdd6d23c6f8e844b984a88af32570b0cc0`, and `Apache-2.0.txt` was `074e6e32c86a4c0ef8b3ed25b721ca23aca83df277cd88106ef7177c354615ff`.

The focused suite `python3 -B -m unittest discover -s tests/packaging -q` exited 0 with 19 tests. A disposable copy of the actual locked inputs with `i_key_sort`'s primary file removed failed admission with `FileNotFoundError` and zero output files. A changed copy failed with `external notice checksum mismatch` and zero output files. A full disposable stage using the admitted bundle and the frozen `files.txt` exited 0, produced 19 owned paths under the three allowed prefixes, and staged a matching notice pair. Removing those 19 paths left an unrelated project sentinel unchanged. These checks do not include an Arch rebuild or a pacman transaction.

This source is a notice-admission worktree based on `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`. It is not the previously built private Arch artifact. The 89-crate normal graph and local Rust texts are a bounded inventory, not a complete linked-code or legal distribution audit. Actual Omarchy installation, target ABI, and package release acceptance remain open.

## Frozen source identity

I read these SHA-256 values from the A21 worktree before writing this review:

| Path | SHA-256 |
| --- | --- |
| `packaging/arch/notices.py` | `e194be3a5383022c34aa886b6c9c44d3cc68f486d8edbc52c08f506f283e9f43` |
| `packaging/arch/notice-inputs.json` | `218fbaaee3f49a6dd6fc240b09c7ab4b45fb02f9b4eca459d309913778acdc4e` |
| `packaging/arch/package.py` | `86713f1198970fd419e0ce9c023d95c89ce7aa4e3f0425ee1e5fd237d1612892` |
| `packaging/arch/files.txt` | `3649ef26e5e373f58ed1a50d663a215763f8dea4022c58f4a4b0e199e5166452` |
| `tests/packaging/test_notices.py` | `283c1d8945d8446fdd2dff244e809cdbdbac56c78c370ae52e61e57d96978154` |
| `tests/packaging/test_package.py` | `b006e7098d060d2d9f17380abc41c8187381c5b10c995e9a7d5f1d41930aa9b1` |
| `docs/plans/package-handoff.md` | `59c3cf7f224c433fbec8a53840b1c9c732bb94fa18658f19cbbd3765372d6d4b` |
| `docs/omarchy/package.md` | `b051d83b6b0e8584cc4abf2544f10560573691a0b38b8601c17ff5e1a5813881` |

Boundary Discipline shaped the two findings at the manifest and byte-pair admission boundaries. Prove It Works and interrogate's run-backed rule led to the disposable stage probes. No product source, main checkout, commit, push, agent, host install, or package distribution changed in this review.
