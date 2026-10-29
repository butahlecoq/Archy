# A21 slice 4 core package handoff

## Implementation and test plan (before edits)

1. Define a small explicit core inventory under the three allowed `/usr` prefixes. Stage it into an existing disposable root with preflight validation, collision rejection, SHA-256 inventory, and rollback of newly created files on failure. Test the real staging interface first, including hostile links and preservation sentinels.
2. Generate an Arch PKGBUILD only from a caller supplied source archive, source identity, and verified SHA-256. Build offline with Cargo.lock and a disabled rustup auto installer. Keep package hooks absent. Test invalid identities and archive mismatches.
3. Inspect the available ELF and cached locked Rust sources for dependency and notice evidence. Run focused tests, syntax checks, and available Arch tool probes. Record exact limits and leave source freeze, Arch build, and target acceptance open.

## State and ownership

Bounded core packaging foundation, with Gauss rereview of the P1/P2 fixes passed in [package-review-round1.md](package-review-round1.md). A content-addressed **development** source archive and Arch container package now exist; neither is a released revision or Omarchy acceptance. The parent checkout was at HEAD `7189f5be0f8cc8d09e9f4e30db143c63bfe96552` with dirty worktree inputs. The recipe generator writes a pinned PKGBUILD only after checking the supplied archive against its exact SHA-256 and requiring Cargo.lock and package tooling inside a single safe archive root. The `v0.1.0-test` identifier in unit tests is a synthetic fixture only. No hook file exists. No host install, user configuration edit, service enablement, backend import, commit or push was performed.

Changed authored paths: `packaging/arch/package.py`, `packaging/arch/files.txt`, `tests/packaging/test_package.py`, `docs/omarchy/package.md`, `docs/plans/package-handoff.md`. Existing `packaging/arch/compatibility.json` was read only. Rust, Cargo, QML, workers, mise, doctor, and existing install.md remain under their other owners.

## Package contents and WSL staging evidence

The allowlist stages 17 regular files into an existing disposable root: one executable mode 0755, 15 data files mode 0644, and Archy's MIT license mode 0644. All SHA-256 values below are from a **temporary WSL stage of the current mutable tree**, using the existing `target/debug/archy`, and are not a source or release identity. The binary digest and any mutable source digest must be remeasured for a frozen Arch build.

| Mode | SHA-256 | Owned path |
| --- | --- | --- |
| 0755 | `0b2431e5d343ee8ebd2d7e54cad8172b8b8641c055d3943063fbb891e8d4f941` | `usr/bin/archy` |
| 0644 | `4bdac00243fa6fe444216f687cbcaacb74977e40daa5c8e4103deeaea4a2d4c7` | `usr/share/archy/compatibility.json` |
| 0644 | `95efc31a76d9566da04d5402fbc90c10c7c713731e7db40fd138e8483301fc75` | `usr/share/archy/schemas/backend.schema.json` |
| 0644 | `79e1cf512797ecdb8223d3e8654fda8842a3212b826addcafa611ce084c98e08` | `usr/share/archy/schemas/desktop-capabilities.schema.json` |
| 0644 | `aef69ae283cc13cc8054ae568df21a7ee93d224d6088814e60aabb8f0b72e634` | `usr/share/archy/schemas/desktop-status.schema.json` |
| 0644 | `2254e6c2942d180bf5444db978370f2ce42c3744f40f05cdcb0c22900246a5c1` | `usr/share/archy/schemas/doctor.schema.json` |
| 0644 | `fd29c499a715774969732271a21645c7a1724de82c7d9ce02a7eab07f239ee55` | `usr/share/archy/schemas/error.schema.json` |
| 0644 | `432812d0263665c32c20c2f8e3825103dc5aa696039d9916dc634c89ca3f0532` | `usr/share/archy/schemas/evaluation.schema.json` |
| 0644 | `4917c85d3e8950f86cf1ba62640ed038ce4972bda489aae05298a946e593b62a` | `usr/share/archy/schemas/generation.schema.json` |
| 0644 | `e3f7ec99c99379e316e85c10e4dccb799d86a0d861b323b527d83168a7d5869` | `usr/share/archy/schemas/normalized-project.schema.json` |
| 0644 | `ccabbac470805b32c16b465e52d54da0c1c3552a443dd1da101d45c381aa9476` | `usr/share/archy/schemas/project.schema.json` |
| 0644 | `1ac061bf8c854800c07bfc64a4f928893b3507f0a2b90f82d37d7a77b135cc59` | `usr/share/archy/schemas/report.schema.json` |
| 0644 | `bfdfafb17ee9664518a7f9b2a50f0a8b19bddb4e2c47d43b447cccf864f931d4` | `usr/share/archy/schemas/revision.schema.json` |
| 0644 | `dba036ec616cdd646abf1f090cc6f93363b5d2ba6f23d53853a42a5c254fcb87` | `usr/share/archy/schemas/run-manifest.schema.json` |
| 0644 | `053c758f9feefed45b13e098e8a42b736e2d693a7f46e5b6eefc124694c8e3be` | `usr/share/archy/schemas/run-request.schema.json` |
| 0644 | `34b063814572be904e26a9da83b7c51495fa9bfe1b972e114bdea2b38b47922f` | `usr/share/archy/schemas/variant.schema.json` |
| 0644 | `fce40550b4b13a878fea7b237236a998f7a0a099ef47c1c438c94263c5271988` | `usr/share/licenses/archy/LICENSE` |

Staging validates every declared source and destination before any copy, rejects path traversal, links, special sources, unexpected modes, duplicate destinations, unsafe parents and existing target files. It writes each file with exclusive creation, rolls back only its own new files if a later step fails, and never deletes a preexisting path. The temporary-root test simulates removal of the inventoried package paths and verifies project, run, approval, config and backup sentinels survive. This is **removal simulation**, not pacman behavior.

## Build and dependency evidence

Generated recipe: local source archive with exact SHA-256; `cargo build --offline --locked --release -p archy-cli`; `RUSTUP_AUTO_INSTALL=0`; `CARGO_NET_OFFLINE=true`; separate Cargo target directory; stage only via the allowlist into `$pkgdir`; `options=('!debug')` prevents an additional debug package outside the bounded path set. Build dependencies are `cargo`, `python`; runtime dependencies are `glibc`, `libgcc`, as confirmed by the exact Arch artifact and package ownership lookup below. There is no mise, Cargo, venv or Python runtime dependency for the installed CLI. `python` is used only to stage at package build time. The development snapshot contains an offline vendor tree with a relative `.cargo/config.toml`; a final release must review its exact inputs and licenses.

`readelf -d target/debug/archy` on WSL initially returned NEEDED `libc.so.6`, `ld-linux-x86-64.so.2`, `libm.so.6`, `libgcc_s.so.1`; no RPATH/RUNPATH line was returned. `readelf --version-info` showed GLIBC and GCC versioned symbols (including GLIBC_2.39 weak and GCC_4.2.0). That WSL observation did not establish Arch ABI compatibility; the later Arch development artifact and package ownership lookup below supplied the actual dependency evidence. No direct Blender, IfcOpenShell, Qt or Quickshell runtime dependency is declared because none is shipped in this core package.

## Rust dependency notice inventory from current cache

`cargo metadata --offline --locked --format-version 1 --filter-platform x86_64-unknown-linux-gnu` exited 0. Traversing normal dependency edges from `archy-cli` yielded 89 registry package IDs. This is a conservative candidate notice set: metadata can include host proc macro/build dependencies and cannot prove which code was linked into the binary. License strings are the cached crates' package metadata, not a completed license/text/source audit. The current cache lacks a top-level LICENSE/LICENCE/COPYING/UNLICENSE file for `geo@0.33.1`, `i_key_sort@0.10.3`, `i_overlay@4.5.2`, and `rstar@0.12.2`. A later read-only scan through depth 3 found no file with license/copying/notice in its name for any of the four. Their cached Cargo.toml metadata declares respectively `MIT OR Apache-2.0`, `MIT`, `MIT OR Apache-2.0`, and `MIT OR Apache-2.0`; the README files for geo, i_overlay, and rstar describe the license choice and refer to LICENSE files absent from these cached crate trees. Review exact source distributions/upstream license texts before release. Third-party notice texts are not yet packaged, so this foundation is not a distribution approval.

| Metadata license expression | Current candidate packages |
| --- | --- |
| MIT OR Apache-2.0 (63) | annotate-snippets@0.12.17, anstream@1.0.0, anstyle-parse@1.0.0, anstyle-query@1.1.5, anstyle@1.0.14, base64@0.23.1, bitflags@2.13.2, block-buffer@0.12.1, cfg-if@1.0.5, clap@4.6.7, clap_builder@4.6.7, clap_derive@4.6.7, clap_lex@1.1.1, colorchoice@1.0.5, cpufeatures@0.3.1, crypto-common@0.2.2, digest@0.11.3, dyn-clone@1.0.20, encoding_rs_io@0.1.8, errno@0.3.14, geo-types@0.7.20, geo@0.33.1, getrandom@0.4.3, granit-parser@2.0.0, hash32@0.3.1, heapless@0.8.0, heck@0.5.0, hybrid-array@0.4.15, i_overlay@4.5.2, is_terminal_polyfill@1.70.2, itoa@1.0.18, libc@0.2.191, log@0.4.34, num-traits@0.2.19, ppv-lite86@0.2.21, proc-macro2@1.0.107, quote@1.0.47, rand@0.10.3, rand_chacha@0.10.0, rand_core@0.10.0, rand_pcg@0.10.2, ref-cast-impl@1.0.27, ref-cast@1.0.27, robust@1.2.0, rstar@0.12.2, scopeguard@1.2.0, serde-saphyr@2.0.0, serde@1.0.229, serde_core@1.0.229, serde_derive@1.0.229, serde_derive_internals@0.30.0, serde_json@1.0.154, serde_path_to_error@0.1.20, sha2@0.11.1, sif-itree@0.4.1, simdutf8@0.1.5, smallvec@1.16.4, stable_deref_trait@1.2.1, syn@3.0.9, thiserror-impl@2.0.21, thiserror@2.0.21, typenum@1.20.1, unicode-width@0.2.3 |
| MIT (11) | float_next_after@2.0.0, geographiclib-rs@0.2.7, i_float@1.16.0, i_key_sort@0.10.3, i_shape@1.18.0, i_tree@0.18.0, libm@0.2.16, schemars@1.2.3, schemars_derive@1.2.3, strsim@0.11.1, zmij@1.0.23 |
| Apache-2.0 OR MIT (5) | const-oid@0.10.2, multiversion_no_op@1.0.0, nohash-hasher@0.2.0, utf8parse@0.2.2, uuid@1.29.0 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT (2) | linux-raw-sys@0.12.1, rustix@1.1.4 |
| MIT/Apache-2.0 (2) | arraydeque@0.5.1, core_detect@1.0.0 |
| Unlicense OR MIT (2) | byteorder@1.5.0, memchr@2.8.3 |
| (Apache-2.0 OR MIT) AND BSD-3-Clause (1) | encoding_rs@0.8.42 |
| (MIT OR Apache-2.0) AND Unicode-3.0 (1) | unicode-ident@1.0.27 |
| Apache-2.0 (1) | approx@0.5.1 |
| BSD-2-Clause OR Apache-2.0 OR MIT (1) | zerocopy@0.8.63 |

## Validation and remaining gates

Focused tests initially failed because `packaging/arch/package.py` did not exist; after implementation the five tests passed. They cover inventory/mode/digest, simulated owned-path removal with five classes of sentinel, collision, traversal, source and destination links, rollback with preexisting inventory and unrelated data, and pinned recipe generation/hash mismatch. The synthetic recipe also passed `bash -n`. `python3 -m py_compile packaging/arch/package.py` and `git diff --check -- packaging/arch tests/packaging docs/plans/package-handoff.md` exited 0 in the local run. Exact final recheck appears below after any final edits.

Initial host probes: `command -v makepkg`, `command -v namcap`, and `command -v pacman` returned no path on WSL; `/usr/bin/docker` exists and `docker --version` printed `Docker version 29.4.2, build 055a478`. The coordinator verified official Arch base image `docker.io/library/archlinux@sha256:624bdc7fc6615155d67bc34cf7146c0c9fd0e3c92943977da63a630e4ca49c30` (tag `base-devel-20261025.0.616225`). The later Arch build uses the coordinator's derived nonroot image recorded below. An Arch container check is build-environment evidence only, never actual Omarchy or host pacman acceptance.

Pinned base image probe: `docker run --rm --network none --read-only --tmpfs /tmp:rw,noexec,nosuid,size=16m docker.io/library/archlinux@sha256:624bdc7fc6615155d67bc34cf7146c0c9fd0e3c92943977da63a630e4ca49c30 /bin/sh -c 'command -v makepkg; command -v namcap || true; command -v pacman; makepkg --version | head -1; pacman --version | head -2'` exited 0. It reported `/usr/sbin/makepkg`, `/usr/sbin/pacman`, `makepkg (pacman) 7.1.0`, and `Pacman v7.1.0 - libalpm v16.0.1`; `namcap` returned no path in the base image. This was a tool availability probe only, with no source mount, package build, install, or host mutation.

Final local recheck: `python3 -B -m unittest discover -s tests/packaging -v` exit 0 (5 tests); AST parsing of `packaging/arch/package.py` and `tests/packaging/test_package.py` exit 0; `git diff --check -- packaging/arch docs/omarchy/package.md docs/plans/package-handoff.md tests/packaging` exit 0 (these paths are untracked, so this command cannot inspect their whitespace). Synthetic generated PKGBUILD `bash -n` ran inside the recipe test and exited 0. `docker image inspect archy-arch-dev:20261029 --format '{{.Id}}'` exited 1, `No such image`, while the coordinator's shared image was building; no duplicate environment setup was attempted. No full workspace Rust test was run during concurrent Rust edits.

Incremental shared image evidence after that recheck: `docker image inspect archy-arch-dev:20261029 --format '{{.Id}}'` exited 0 and returned immutable ID `sha256:c4050a4bcd4c1f640d8c77b9635bfa70f448da232f1e4b878ce2258c3998de05`. A read-only, network-disabled disposable container with a 16 MiB `/tmp` tmpfs ran `pacman -Q rust python namcap` and `command -v makepkg namcap pacman`, exit 0: `rust 1:1.99.1-1`, `python 3.14.8-1`, `namcap 3.6.0-3`, and paths `/usr/sbin/makepkg`, `/usr/sbin/namcap`, `/usr/sbin/pacman`. No source mount or build was used at that checkpoint. This image is an Arch build environment, not an Omarchy target. Gauss later passed P1/P2 rereview; the development build below follows that review.

Open gates after the Arch development build: independent review of the final recipe and artifact; release source/revision identity and publication decision; complete third-party notice/license audit and any required source/replacement obligations; actual disposable Omarchy install/upgrade/remove and preservation checks; backend runtime packaging; A22 desktop integration; A23 target acceptance. The source archive below is a pinned development snapshot of a dirty parent, not a release revision. Full A21 remains open.

## Review round 1 fixes, frozen for Gauss rereview

Gauss's [review](package-review-final.md) requested two blocking changes. P1 was reproduced by replacing `dest/usr/bin` with a symlink between preflight and `os.open`: the original pathname write reached an outside directory, and pathname rollback could remove a preexisting outside sentinel. P2 was reproduced with SHA-256-correct tarballs containing duplicate `Cargo.lock` or a directory in place of that mandatory file: the original recipe generator emitted a PKGBUILD. Both root causes matched the original code paths.

P1 implementation: `open_directory` walks each absolute path component from `/` using `O_DIRECTORY|O_NOFOLLOW`; `read_regular_nofollow` pins source parent and file descriptors; `open_stage_parent` creates/opens destination directories relative to a pinned root descriptor; final files use `O_CREAT|O_EXCL|O_NOFOLLOW` relative to their pinned parent descriptor and `fchmod` on the file descriptor. After writes, `verify_created` compares the rooted path's device/inode with the created file and rejects a substituted root or parent. Rollback uses the held parent descriptor plus the created file's device/inode, so it can unlink only that new file even when its pathname has been swapped. Inventory creation also uses an exclusive descriptor-relative file open. No new dependency was added.

P2 implementation: tar entries are normalized and must be unique; a regular file cannot be an ancestor of another entry; each of `Cargo.lock`, `packaging/arch/files.txt`, and `packaging/arch/package.py` must occur exactly once as a regular file before recipe output is created. Links and special members remain rejected. Archive bytes are now captured through a no-follow file descriptor and the same captured bytes supply both the SHA-256 check and tar inspection.

Test-first evidence: the first expanded run of `python3 -B -m unittest discover -s tests/packaging -v` exited 1 with 5 failures and 1 error; it exposed a test fixture that accidentally occupied the outside target and reused a generated recipe path. After correcting only those fixtures, the red run exited 1 with 6 failures and 1 error: four malformed tar cases were accepted, the destination and source substitutions were accepted, and mid-copy pathname rollback deleted the outside sentinel. Following P2 alone, `python3 -B -m unittest discover -s tests/packaging -p test_package.py -k RecipeTests -v` exited 0 (2 tests). Following P1, the focused run exited 0 (9 tests), covering both substitutions, mid-copy rollback, archive duplicate/non-file/prefix cases, previous preservation tests, and synthetic PKGBUILD `bash -n`. A further archive-parent substitution test failed against the pathname based archive read (`exit 1, 1 failure`), then passed after descriptor capture (`exit 0, 1 test`).

Checks at the P1/P2 rereview checkpoint: `python3 -B -m unittest discover -s tests/packaging -v` exit 0, 10 tests; disposable WSL staging with `target/debug/archy` exit 0, 17 owned files all within the three allowed prefixes; Python AST parse of `package.py` and `test_package.py` exit 0; authored-path trailing whitespace check exit 0. No Arch package build had been performed at that checkpoint. The Arch development build below does not claim a release source or actual Omarchy acceptance.

## Arch development source and package build after P1/P2 rereview

The coordinator supplied an offline vendor tree from `cargo vendor --locked --offline` (coordinator-reported exit 0) and a nonroot Arch package image. `docker image inspect archy-arch-package-build:20261029 --format '{{.Id}}'` returned immutable image ID `sha256:8471fd2be9c91daf7278e3067744448f7c437878aefeb67705b6f16f53de6eeb`; `id` in the image reported UID/GID 1000 (`archy-build`), and `pacman -Q rust python namcap` returned `rust 1:1.99.1-1`, `python 3.14.8-1`, `namcap 3.6.0-3`. The image derives from the coordinator's verified Arch base, but it is not Omarchy.

Snapshot selection: from the frozen scoped inputs in the dirty main checkout, copy root `Cargo.toml`, `Cargo.lock`, `LICENSE`, the embedded example project YAML, all 14 schema JSON files, `packaging/arch/{package.py,files.txt,compatibility.json}`, the six workspace crate manifests and their `src` trees, and the coordinator-supplied vendor tree. Add `.cargo/config.toml` with `replace-with = "vendored-sources"` and relative `directory = "vendor"`; never copy the coordinator's absolute vendor path. The final inventory has **6,817 regular files**: 6,756 vendor and 61 nonvendor. It contains no `.git`, `.tools`, `target`, cache, or private plan-state path. Source files were hashed before and after copying; an attempted first script exited 1 with `SameFileError` because it treated absolute crate paths as relative, before any archive was emitted. The corrected copy script checked all copied hashes and rechecked source inputs after copying. The parent HEAD was `7189f5be0f8cc8d09e9f4e30db143c63bfe96552`, **dirty=true**. This is a content-addressed development snapshot, not a tagged or published release.

Final source identity and artifact locations:

| Item | Exact evidence |
| --- | --- |
| Development source ID / inventory SHA-256 | `0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b` / `5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b` |
| Source inventory | `/var/tmp/archy-a21-core-build-5/source-inventory.tsv` (every source path, mode and SHA-256) |
| Deterministic gzip tar archive | `/var/tmp/archy-a21-core-build-5/archy-source.tar.gz`; SHA-256 `927121bc8e7289d9edc17be2f9e9d41cfae81472fd51835928c89f8dbc714946` |
| Generated PKGBUILD | `/var/tmp/archy-a21-core-build-5/pkgwork/PKGBUILD`; SHA-256 `94c6af5ea05c0dd4d85e937a03e70808cc76c3dd7392a43d6225ac61366d0333` |
| Main package | `/var/tmp/archy-a21-core-build-5/pkgwork/artifacts/archy-0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b-1-x86_64.pkg.tar.zst`; 1,484,441 bytes; SHA-256 `2008fd85344c76e07f6eb1fd4ae7303863d11dd17473a583b5809a8345a324fe` |
| Extracted artifact inventory | `/var/tmp/archy-a21-core-build-5/pkgwork/artifact-files.tsv` (post-strip paths, modes and SHA-256) |
| Build logs | `/var/tmp/archy-a21-core-build-5/pkgwork/archy-0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b-1-x86_64-build.log` and `/var/tmp/archy-a21-core-build-5/pkgwork/archy-0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b-1-x86_64-package.log` from `makepkg --log` |

Final build invocation used `docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges --read-only --memory 4g --cpus 4 --pids-limit 512 --tmpfs /tmp:rw,nosuid,size=512m`, image `archy-arch-package-build:20261029`, archive bind mounted read-only at `/source.tar.gz`, and only `/var/tmp/archy-a21-core-build-5/pkgwork` bind mounted writable at `/work`. `HOME=/work/home`, `CARGO_HOME=/work/cargo-home`, `PKGDEST=/work/artifacts`, and `CARGO_BUILD_JOBS=4`; no host home, Docker socket, privileged mount, or network was supplied. The archive and generated recipe are in disposable output paths. The installed package was not applied to the host or container package database.

| Final check on exact source/artifact | Exit and observed result |
| --- | --- |
| `python3 /package.py recipe --archive /source.tar.gz --source-id 0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b --sha256 927121bc8e7289d9edc17be2f9e9d41cfae81472fd51835928c89f8dbc714946 --output /work/PKGBUILD` | 0; exact pinned recipe emitted. |
| `makepkg --verifysource` | 0; `archy-source.tar.gz ... Passed`. Local `file:///source.tar.gz` input under `--network none`; makepkg's “Downloading” label was its local file retrieval. |
| `makepkg --printsrcinfo` | 0; source hash above, URL `https://github.com/butahlecoq/Archy` from observed `git remote origin`, MIT, `cargo`/`python` build dependencies, `glibc`/`libgcc` runtime dependencies, `options = !debug`. |
| `makepkg --cleanbuild --log --noconfirm` | 0; offline locked Rust release build and core package creation. Exactly one `.pkg.tar.zst` artifact exists in the final artifacts directory. |
| `namcap /PKGBUILD` | 0; warning `Missing Maintainer tag`. No maintainer identity was invented. |
| `namcap /artifact/archy-0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b-1-x86_64.pkg.tar.zst` | 0; no output or findings. |
| `pacman -Qip /artifact/archy-0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b-1-x86_64.pkg.tar.zst` | 0; version matches source ID, URL above, `Depends On: glibc libgcc`, MIT, `Install Script: No`, unsigned, `Packager: Unknown Packager`. |
| `pacman -Qlp /artifact/archy-0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b-1-x86_64.pkg.tar.zst` | 0; 17 regular owned files and their parent directories only under `/usr/bin`, `/usr/share/archy`, `/usr/share/licenses/archy`. |
| `bsdtar -xf /artifact/archy-0.1.0_dev5794fd798587f1acfc4df95e5ef7fb3fec005e6f0b056c42d2b3e81b8d6b2a5b-1-x86_64.pkg.tar.zst -C /inspect` | 0; inspection used the exact hashed artifact. |
| `readelf -d /inspect/usr/bin/archy` | 0; NEEDED `libc.so.6`, `ld-linux-x86-64.so.2`, `libm.so.6`, `libgcc_s.so.1`; no RPATH/RUNPATH entry. |
| `/inspect/usr/bin/archy --help` | 0 in the Arch image, using the extracted package binary without mise activation, Cargo invocation or venv. |
| `/inspect/usr/bin/archy --version` | 0; stdout `archy 0.1.0`, stderr empty. Captured in `/var/tmp/archy-a21-core-build-5/pkgwork/version.stdout` and `.stderr`. |
| `/inspect/usr/bin/archy doctor --json` | 1; valid 6,778-byte JSON stdout, empty stderr, `ready: false`, `target_acceptance: deferred`; 12 checks: 5 missing, 3 available, 3 deferred, 1 unsupported. Captured in `/var/tmp/archy-a21-core-build-5/pkgwork/doctor-json.stdout` and `.stderr`. |
| `/inspect/usr/bin/archy doctor` | 1; 1,539-byte text stdout begins `Archy doctor — diagnostic ready: no`, empty stderr. Captured in `/var/tmp/archy-a21-core-build-5/pkgwork/doctor-text.stdout` and `.stderr`. |

`pacman -Qo /usr/lib/libc.so.6 /usr/lib/ld-linux-x86-64.so.2 /usr/lib/libm.so.6 /usr/lib/libgcc_s.so.1` in the same Arch image exited 0: the first three are owned by `glibc 2.44+r50+g1848099f063e-1`, and `libgcc_s.so.1` by `libgcc 16.2.1+r23+gd564253eb6c8-1`. This directly corrected the initial WSL-derived `gcc-libs` guess. The exact Arch binary's `readelf --version-info` check showed GLIBC version requirements through `GLIBC_2.35` plus weak `GLIBC_2.39`, and GCC through `GCC_4.2.0`. Container execution and these symbols do not prove compatibility with an unknown Omarchy installation.

### Exact final package file inventory

These are **post-strip hashes from the extracted main package**, not the staging JSON's pre-strip binary hash (`666097dff9f400f9a08cbae8c18ab82ea87a4e5132298c4626c635057cdeb272`). Every entry is a regular file.

| Mode | SHA-256 | Owned path |
| --- | --- | --- |
| 0755 | `643be1d625666a9830d815d9deb7960688bcb46d417837bc64eb32bc2b931941` | `usr/bin/archy` |
| 0644 | `4bdac00243fa6fe444216f687cbcaacb74977e40daa5c8e4103deeaea4a2d4c7` | `usr/share/archy/compatibility.json` |
| 0644 | `95efc31a76d9566da04d5402fbc90c10c7c713731e7db40fd138e8483301fc75` | `usr/share/archy/schemas/backend.schema.json` |
| 0644 | `79e1cf512797ecdb8223d3e8654fda8842a3212b826addcafa611ce084c98e08` | `usr/share/archy/schemas/desktop-capabilities.schema.json` |
| 0644 | `aef69ae283cc13cc8054ae568df21a7ee93d224d6088814e60aabb8f0b72e634` | `usr/share/archy/schemas/desktop-status.schema.json` |
| 0644 | `2254e6c2942d180bf5444db978370f2ce42c3744f40f05cdcb0c22900246a5c1` | `usr/share/archy/schemas/doctor.schema.json` |
| 0644 | `fd29c499a715774969732271a21645c7a1724de82c7d9ce02a7eab07f239ee55` | `usr/share/archy/schemas/error.schema.json` |
| 0644 | `432812d0263665c32c20c2f8e3825103dc5aa696039d9916dc634c89ca3f0532` | `usr/share/archy/schemas/evaluation.schema.json` |
| 0644 | `4917c85d3e8950f86cf1ba62640ed038ce4972bda489aae05298a946e593b62a` | `usr/share/archy/schemas/generation.schema.json` |
| 0644 | `e3f7ec99c99379e316e85c10e4dccb799d86a0d861b323b527d83168a7d5869d` | `usr/share/archy/schemas/normalized-project.schema.json` |
| 0644 | `ccabbac470805b32c16b465e52d54da0c1c3552a443dd1da101d45c381aa9476` | `usr/share/archy/schemas/project.schema.json` |
| 0644 | `1ac061bf8c854800c07bfc64a4f928893b3507f0a2b90f82d37d7a77b135cc59` | `usr/share/archy/schemas/report.schema.json` |
| 0644 | `bfdfafb17ee9664518a7f9b2a50f0a8b19bddb4e2c47d43b447cccf864f931d4` | `usr/share/archy/schemas/revision.schema.json` |
| 0644 | `dba036ec616cdd646abf1f090cc6f93363b5d2ba6f23d53853a42a5c254fcb87` | `usr/share/archy/schemas/run-manifest.schema.json` |
| 0644 | `053c758f9feefed45b13e098e8a42b736e2d693a7f46e5b6eefc124694c8e3be` | `usr/share/archy/schemas/run-request.schema.json` |
| 0644 | `34b063814572be904e26a9da83b7c51495fa9bfe1b972e114bdea2b38b47922f` | `usr/share/archy/schemas/variant.schema.json` |
| 0644 | `fce40550b4b13a878fea7b237236a998f7a0a099ef47c1c438c94263c5271988` | `usr/share/licenses/archy/LICENSE` |

### Build findings, superseded attempts, and limits

The first Arch build of development snapshot `0.1.0_dev97a067f1bd48be5c0e6b2478b9d8d6e36cc2fbcea4af0878f8c40f853b1b09ab` (archive SHA-256 `953acb60f46c0519f89914cd65e4d7e5729d038fee2e6c91483e109d2b708e33`) exited 0 and produced a main artifact with SHA-256 `85325397a7f209a8948179c634b9bf29615523f6bd4423ff0db2b2500ff5ba92`. `namcap` then reported `gcc-libs` may be unnecessary, and `pacman -Qo` identified `libgcc` as the actual owner. Snapshot `0.1.0_dev4ed2d019e5b4eaf68d41f065bc2ffc45dd4c387ace40eb9d176cc5e337320ce0` (archive SHA-256 `6b312b02d01d0d50461cbb83d375b39e40f94931a0d06179eaa4036dd595dc84`) corrected that dependency and description; its first build exited **4** with `No space left on device` because `/tmp` was a full 3.9 GiB tmpfs. Only our own extracted build trees were removed; source archives, inventories, logs and artifacts were retained. The same pinned snapshot rebuilt under `/var/tmp`, exit 0, main artifact SHA-256 `34708e91e886aaf74e5f485ab84867cca4c621c8e9fc6c16068f2080d6cd2e9a`. A later `--printsrcinfo` probe on snapshot `0.1.0_devcaac29fd82ede1e5370790a6bb835d8415ef1f9532fd1378b368e4051e69320a` (archive SHA-256 `3d5bababb64fcb3df81bd865029f8badb9ff55ddd502d12ab7c3b9b556555cb4`) exited **11** only because that probe mounted its build directory read-only; rerunning with the same disposable directory writable exited 0. That snapshot added the observed Git origin URL and built exit 0, main artifact SHA-256 `43e0bf341dd0c92104c9deab3e9a3908aa9de9692844bcc64d5dcfd5c2907dbc`, but makepkg also produced an `archy-debug` package under `/usr/lib/debug` and `/usr/src/debug`. The final snapshot above adds `options=('!debug')`; its artifacts directory contains exactly one core package. Superseded artifacts are evidence only and should not be distributed.

The final makepkg run exited 0 but warned `Package contains reference to $srcdir` for `usr/bin/archy`; byte inspection of the extracted binary found 74 `/work/src/...` occurrences, including vendored Rust source paths. This generic build path is reproducibility/privacy review material, not evidence of a runtime write or external path dependency. `namcap` on the final artifact returned no findings; `namcap` on the PKGBUILD still warns `Missing Maintainer tag`, because no maintainer identity was authorized. The package is unsigned, has `Packager: Unknown Packager`, and was not installed. Third-party notice texts are not staged, and the existing 89-package metadata inventory is not a complete linked-code/license audit. Backend runtimes and workers are intentionally absent. Actual Omarchy ABI, pacman install/upgrade/remove preservation, signed distribution, and independent review of this final artifact remain open. The artifact remains **private until the license and notice audit**.

The read-only doctor check used `--network none`, no privileged mount, a 64 MiB private `/tmp` tmpfs, `HOME=/tmp`, and only the extracted package tree mounted read-only at `/inspect`. Its JSON `host.environment` is `wsl`, `host.os_id` is `arch`, `host.kernel` is `6.6.87.2-microsoft-standard-WSL2`, and `platform.raw_package_identity` is null. All target gates `OM-A21-01` through `OM-A21-05` remain deferred. This is a container smoke check, not installed Omarchy doctor evidence. The captured archive predates the coordinator's later reviewed artifact P2 **test-file** update; the selected CLI/core/generate/eval/report/adapters/schema/package build inputs did not change, so the binary artifact is still tied to the manifest above. The later file is outside this explicit build-input archive and has no claimed effect on its bytes.

Final scoped source check: `sha256sum packaging/arch/package.py /var/tmp/archy-a21-core-build-5/snapshot/archy-source/packaging/arch/package.py` returned the same `09d1a310d5d97a24bb3de717e977418893b0670b780080cd52597ab1c88c7774` for both files. `python3 -B -m unittest discover -s tests/packaging -v` exited 0 with 10 tests; Python AST parsing and authored-path whitespace checks exited 0. No full Rust workspace test was run while other owners edited source.
