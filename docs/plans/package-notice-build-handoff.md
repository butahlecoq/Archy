# A21 private notice-enabled Arch package build handoff

This is a private development build for separate independent review. It was made from worktree `/home/butahlecoq/work/archy/.worktrees/a21-notice-package`, branch `codex/a21-notice-package`, clean at commit `ea6c37f5698f50b091e635945a55b8f5e41b8975` before this handoff was written (Git tree `ffd3884137eea78a12de248676bf95a0f8533b81`). The exact source archive includes the notice generator, input manifest, and ten pinned notice source files; the bundle and inventory are generated during the build. It does not represent an approved release or a complete legal audit.

## Source capture and private evidence

All evidence below is under the private mode-0700 directory `/var/tmp/archy-a21-notice-build-IJHK1bgL` on `/dev/sdd`. No files from the workspace's main checkout, home configuration, or environment were copied. `capture.py` selects 73 tracked build-input files from `git archive` of the exact commit: root Cargo manifest, lock and license; six crate manifests and source trees; the embedded example; schemas; and `packaging/arch`. It copies the supplied `/tmp/archy-core-arch-source/vendor` into the snapshot and compares every copied vendor file against the supplied tree by SHA-256. It adds `.cargo/config.toml` with `replace-with = "vendored-sources"` and relative `directory = "vendor"`. The source contains 6,830 regular files: 6,756 vendor, 73 tracked, and the generated Cargo config. No symlink or special input was accepted. The deterministic tar uses one `archy-source` root, normalized ownership and timestamp, and gzip timestamp zero. The script reread the completed archive and matched every archived file hash against the inventory before the build.

The selection excludes `.git`, `.tools`, build targets, project data, and user configuration. Sensitive-name checks and scans for common private-key, AWS, OpenAI, GitHub, and Slack credential patterns found no matches. One upstream vendor file, `vendor/float_next_after-2.0.0/.envrc`, contains only `use flake` (SHA-256 `1d26c74fd25a4c3dbb09e029fc609588da499fd4af2a41c88f6316c7f8c54cf1`); it is part of the supplied crate, was not sourced by this build, and has no assignment or secret. These checks and the constrained input selection do not prove the absence of every possible secret or proprietary content.

| Item | SHA-256 / identity |
| --- | --- |
| Exact source inventory, all 6,830 modes, paths and file hashes: `source-inventory.tsv` | `ec43ff8adbc28e058a0feaebb26be915044e078bb7c746be4c00283ea1f00060` |
| Development source ID | `0.1.0_devec43ff8adbc28e058a0feaebb26be915044e078bb7c746be4c00283ea1f00060` |
| Exact source archive `archy-source.tar.gz` (14,983,735 bytes) | `c7a8fcf375d3a86345e1deb3b456181dc2560f9ab27fdb511f1415218744bcd8` |
| Generated `work/PKGBUILD` | `617431485c0344f413ce17fa8e00b7d3cff47bbaa8c016572997d17aab9ec84f` |
| Build binary before makepkg stripping | `d0e6ac7f3aef561988a839adbf79eada0a503da3b1e058f59fb1cc7e3aacba7e` |
| Binary extracted from the finished package | `ba034490cb5511857ebf1da78d117f72924cf75d9c48a61e9e6da52825763ec7` |
| Package `work/artifacts/archy-0.1.0_devec43ff8adbc28e058a0feaebb26be915044e078bb7c746be4c00283ea1f00060-1-x86_64.pkg.tar.zst` (1,523,748 bytes) | `ff207478344593ccaf118c9c7019532178c527f0e61b3c962c3e2e4125424824` |
| Extracted package inventory `work/artifact-inventory.tsv` | `5f55e3464597a879c488a0951ab08b53945407b97bead47bac69e8db29f4d8ab` |
| Capture and verification scripts `capture.py` / `verify.py` | `967bb1608f8ef80e88fbb10129c3e31bd3960a5e26e4d1b1dfcb193363a9b748` / `4020b65ddcc9cf2e48ce36cbc861c4dd6a21e3b84917541c04ed20ff581e9218` |
| Build and inspection scripts `work/build.sh` / `work/inspect.sh` | `846bb63cf1f393bf38553eab7cddb87ae53baeb71f068c8651a9140fa958c170` / `eb0c81b18c7a887681612b8b1fde2de11f9caa52cec76e50e7e9bc2f3b094c47` |

`python3 /var/tmp/archy-a21-notice-build-IJHK1bgL/capture.py` exited 0 before recipe generation. The script and full source inventory remain in the private directory for audit. The source archive was rehashed after the build, and its digest still matched the recorded value. `python3 /var/tmp/archy-a21-notice-build-IJHK1bgL/verify.py` exited 0 after extraction and writes `work/verification.json` and the artifact inventory. Its file comparisons are repeatable against the retained exact build output.

## Builder and commands

The approved image was addressed by immutable ID `sha256:8471fd2be9c91daf7278e3067744448f7c437878aefeb67705b6f16f53de6eeb`. A read-only, network-disabled probe reported UID/GID 1000 `archy-build`, Rust `1.99.1 (58cf8f9c3 2026-10-13)` (Arch `rust 1:1.99.1-1`), Cargo `1.99.1 (5f94df478 2026-08-27)`, Python `3.14.8-1`, makepkg/pacman `7.1.0`, libalpm `16.0.1`, and namcap `3.6.0-3`. The image is the existing Arch builder, not Omarchy.

The build invocation was:

```sh
docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges --read-only --memory 4g --cpus 4 --pids-limit 512 --tmpfs /tmp:rw,nosuid,size=512m \
  -v /var/tmp/archy-a21-notice-build-IJHK1bgL/archy-source.tar.gz:/source.tar.gz:ro \
  -v /var/tmp/archy-a21-notice-build-IJHK1bgL/snapshot/archy-source/packaging/arch/package.py:/package.py:ro \
  -v /var/tmp/archy-a21-notice-build-IJHK1bgL/work:/work \
  -e HOME=/work/home -e CARGO_HOME=/work/cargo-home -e PKGDEST=/work/artifacts -e CARGO_BUILD_JOBS=4 \
  sha256:8471fd2be9c91daf7278e3067744448f7c437878aefeb67705b6f16f53de6eeb /bin/bash /work/build.sh
```

The script ran `python3 /package.py recipe --archive /source.tar.gz --source-id 0.1.0_devec43ff8adbc28e058a0feaebb26be915044e078bb7c746be4c00283ea1f00060 --sha256 c7a8fcf375d3a86345e1deb3b456181dc2560f9ab27fdb511f1415218744bcd8 --output /work/PKGBUILD`, `makepkg --verifysource`, `makepkg --printsrcinfo`, and `makepkg --cleanbuild --log --noconfirm`. All exited 0. The recipe sets `RUSTUP_AUTO_INSTALL=0` and `CARGO_NET_OFFLINE=true`, runs `notices.py --source . --vendor vendor` before `cargo build --offline --locked --release -p archy-cli`, and stages the generated pair. The local `file:///source.tar.gz` retrieval printed “Downloading”; network was disabled and makepkg's SHA-256 check passed. `work/verifysource.log`, `work/srcinfo.txt`, `work/makepkg.log`, and makepkg's build/package logs are retained. The makepkg log SHA-256 is `407fa806dc226dedb43aef9c7a6c4d07d01c154875849f675cae6577c361b198`.

The emitted build-time notice inventory has 89 crate rows and `crate_count` 89. Its SHA-256 is `e4d95486832a64364e38fe4b565369ad8d075b6529edbf4d84973e69110cffd6`; its declared bundle SHA-256 matches the generated 2,442,033-byte text, `ea7019a4ff8e69a9b0d49c4e54ab5869c1c2a0726997c7c9e30cd425402da710`. Both exact files in the extracted package match the build-time files byte for byte. This inventory describes the locked normal dependency graph and the pinned Rust texts; it is not a complete linked-code or legal distribution audit.

Inspection used the same image with `--network none --cap-drop ALL --security-opt no-new-privileges --read-only --memory 2g --cpus 2 --pids-limit 256 --tmpfs /tmp:rw,nosuid,size=128m`, only the private work directory mounted at `/work`, and `HOME=/work/home`; `/bin/bash /work/inspect.sh` exited 0. The script ran `pacman -Qip` and `pacman -Qlp` on the package, `namcap` on both recipe and package, listed and extracted the package with `bsdtar`, hashed it and the extracted files, ran `readelf`, and ran the extracted binary's `--version`. These pacman commands inspected package files and did not perform a transaction. Captured output is in `work/package-info.txt`, `work/package-paths.txt`, `work/namcap-*.txt`, `work/archive-paths.txt`, `work/readelf-*.txt`, and `work/version.txt`.

`pacman -Qip` reports x86_64, MIT, runtime dependencies `glibc` and `libgcc`, no install script, `Unknown Packager`, and no signature. The original artifact `namcap` invocation exited 0 with empty output, but its noexec temporary mount suppressed an ELF check; that output does not establish a clean lint result. The executable-tempfs rerun and its warning are documented in the review response below. The PKGBUILD check exited 0 with `Missing Maintainer tag`. `readelf -d` reports NEEDED `libc.so.6`, `ld-linux-x86-64.so.2`, `libm.so.6`, and `libgcc_s.so.1`, with no RPATH/RUNPATH. The extracted binary's `--version` exited 0 and printed `archy 0.1.0`. The archive also contains the standard `.BUILDINFO`, `.MTREE`, and `.PKGINFO` metadata.

The verifier compared the 19 manifest destinations against both the regular files under extracted `usr` and `pacman -Qlp`, checked modes, compared all 18 nonbinary payloads byte for byte with the captured build source or admitted notices, checked all 19 stage-log digests against the files used by stage, and compared the bundle hash in the packaged JSON to the packaged bundle. Makepkg strips the binary after stage, so both the pre-strip and packaged binary hashes are recorded above. The complete post-strip owned file inventory is:

| Mode | SHA-256 | Owned path |
| --- | --- | --- |
| `0755` | `ba034490cb5511857ebf1da78d117f72924cf75d9c48a61e9e6da52825763ec7` | `usr/bin/archy` |
| `0644` | `4bdac00243fa6fe444216f687cbcaacb74977e40daa5c8e4103deeaea4a2d4c7` | `usr/share/archy/compatibility.json` |
| `0644` | `95efc31a76d9566da04d5402fbc90c10c7c713731e7db40fd138e8483301fc75` | `usr/share/archy/schemas/backend.schema.json` |
| `0644` | `79e1cf512797ecdb8223d3e8654fda8842a3212b826addcafa611ce084c98e08` | `usr/share/archy/schemas/desktop-capabilities.schema.json` |
| `0644` | `aef69ae283cc13cc8054ae568df21a7ee93d224d6088814e60aabb8f0b72e634` | `usr/share/archy/schemas/desktop-status.schema.json` |
| `0644` | `2254e6c2942d180bf5444db978370f2ce42c3744f40f05cdcb0c22900246a5c1` | `usr/share/archy/schemas/doctor.schema.json` |
| `0644` | `faddfc7462697c1b739c2b26718eb781c709a59cf14e23ac9318ff204013452b` | `usr/share/archy/schemas/error.schema.json` |
| `0644` | `432812d0263665c32c20c2f8e3825103dc5aa696039d9916dc634c89ca3f0532` | `usr/share/archy/schemas/evaluation.schema.json` |
| `0644` | `4917c85d3e8950f86cf1ba62640ed038ce4972bda489aae05298a946e593b62a` | `usr/share/archy/schemas/generation.schema.json` |
| `0644` | `103e235baddcb76d41676a386a0e8cbbe0a45ed59eb2ab44d773b2025763235d` | `usr/share/archy/schemas/normalized-project.schema.json` |
| `0644` | `9f090127d41393b9957c5578b14113927a5480d5d52c87892ede4ae389d5b61c` | `usr/share/archy/schemas/project.schema.json` |
| `0644` | `1ac061bf8c854800c07bfc64a4f928893b3507f0a2b90f82d37d7a77b135cc59` | `usr/share/archy/schemas/report.schema.json` |
| `0644` | `bfdfafb17ee9664518a7f9b2a50f0a8b19bddb4e2c47d43b447cccf864f931d4` | `usr/share/archy/schemas/revision.schema.json` |
| `0644` | `dba036ec616cdd646abf1f090cc6f93363b5d2ba6f23d53853a42a5c254fcb87` | `usr/share/archy/schemas/run-manifest.schema.json` |
| `0644` | `053c758f9feefed45b13e098e8a42b736e2d693a7f46e5b6eefc124694c8e3be` | `usr/share/archy/schemas/run-request.schema.json` |
| `0644` | `34b063814572be904e26a9da83b7c51495fa9bfe1b972e114bdea2b38b47922f` | `usr/share/archy/schemas/variant.schema.json` |
| `0644` | `e4d95486832a64364e38fe4b565369ad8d075b6529edbf4d84973e69110cffd6` | `usr/share/archy/third-party-notices.json` |
| `0644` | `d5fcc21f650d453ceb5525526eb8b76d44878657120d3c8395b1ed4d813e694d` | `usr/share/licenses/archy/LICENSE` |
| `0644` | `ea7019a4ff8e69a9b0d49c4e54ab5869c1c2a0726997c7c9e30cd425402da710` | `usr/share/licenses/archy/THIRD-PARTY-NOTICES.txt` |

## Warnings, checks, and scope

Makepkg warned `Package contains reference to $srcdir` for `usr/bin/archy`. The packaged binary contains 74 occurrences of `/work/src/` and zero occurrences of `/home/butahlecoq/`; this generic source-path evidence needs independent privacy and reproducibility review. No additional compiler warning or error was found in `work/makepkg.log`. The package is private and unsigned; no distribution, install, upgrade, removal, or package database transaction was performed.

`TMPDIR=/var/tmp/archy-a21-notice-build-IJHK1bgL/test-tmp python3 -B -m unittest discover -s tests/packaging -q` exited 0 with 23 tests. The first preliminary attempts to read `AGENTS.md` and use Git from `/home/butahlecoq/work/archy` exited 1 and 128 respectively because that directory is the workspace parent rather than a Git checkout; the requested worktree was then identified and all source and build commands used it. No source was changed to resolve those path errors.

The container reported kernel `6.6.87.2-microsoft-standard-WSL2`; this is an Arch container on WSL, not an installed Omarchy target. Actual target ABI and pacman install/upgrade/removal preservation, complete third-party and linked-code obligations, signed release provenance, and independent artifact review remain open. The private build and extracted inspection directory are retained for that review.


## Review response: namcap loader warning (29 October 2026)

[The independent build review](package-notice-build-review.md) identified that the original artifact namcap result was incomplete. The original Docker option `--tmpfs /tmp:rw,nosuid,size=128m` produced a mount with `noexec` in this environment. Namcap 3.6.0's installed `Namcap/rules/unusedsodepends.py` extracts each ELF to a temporary file and invokes `ldd -r -u`, capturing its output. When `ldd` emits `not a dynamic executable`, the rule returns without reporting a warning. On the noexec mount, an explicit `ldd -r -u /tmp/archy-probe` on the copied package binary exited 1 and reported both `you do not have execution permission` and `not a dynamic executable`; namcap itself exited 0 with empty output. The earlier no-findings claim was therefore incorrect as evidence of a completed ELF check.

A rerun of the same pinned package and image with `/tmp` bind-mounted to an executable SSD directory exited 0 and printed exactly:

```text
archy W: Unused shared library '/usr/lib64/ld-linux-x86-64.so.2' by file ('usr/bin/archy')
```

The corresponding `ldd -r -u /tmp/archy-probe` exited 1 and reported `Unused direct dependencies: /usr/lib64/ld-linux-x86-64.so.2`. The recipe namcap rerun exited 0 and reported `PKGBUILD (archy) W: Missing Maintainer tag`. This is a real namcap/ldd warning on this ELF; it is not a warning that the Arch package named `glibc` can be removed.

Reproduction used the same immutable image `sha256:8471fd2be9c91daf7278e3067744448f7c437878aefeb67705b6f16f53de6eeb`, the original package directory mounted read-only, `--network none --cap-drop ALL --security-opt no-new-privileges --read-only`, and `HOME=/tmp`. The executable case mounted `/var/tmp/archy-a21-namcap-review-qUC9ZXwN/exec-tmp:/tmp` and `findmnt -n -o TARGET,OPTIONS -T /tmp` showed `rw,relatime,discard,errors=remount-ro,data=ordered` with no `noexec`. The control used `--tmpfs /tmp:rw,nosuid,size=128m` and `findmnt` showed `rw,nosuid,nodev,noexec,relatime,size=131072k`. In both cases the commands were `namcap /input/artifacts/*.pkg.tar.zst`, then `cp /input/inspect/usr/bin/archy /tmp/archy-probe` and `ldd -r -u /tmp/archy-probe`. `namcap` and Docker both exited 0 in both cases; the two ldd invocations exited 1 for the distinct reasons above. The SSD directory and log files are private under mode-0700 `/var/tmp/archy-a21-namcap-review-qUC9ZXwN`.

The dependency evidence supports retaining the recipe as built:

- `readelf -lW` identifies `/lib64/ld-linux-x86-64.so.2` as the program interpreter. `readelf -d` lists `libc.so.6`, `ld-linux-x86-64.so.2`, `libm.so.6`, and `libgcc_s.so.1` as `DT_NEEDED`; plain `ldd` resolves all four in the builder image.
- `pacman -Qo` identifies the loader, `libc.so.6`, and `libm.so.6` as `glibc 2.44+r50+g1848099f063e-1`; it identifies `libgcc_s.so.1` as `libgcc 16.2.1+r23+gd564253eb6c8-1`. The recipe's direct `glibc` and `libgcc` package dependencies are still supported by these observations. `libgcc` itself declares a glibc dependency, but the binary directly needs libraries from each package.
- `readelf -VW` lists a `GLIBC_2.3` version requirement against the loader, and `readelf -Ws` lists `__tls_get_addr@GLIBC_2.3` as undefined in the binary; the loader exports that version. A search of `readelf -rW` found no named dynamic relocation for that symbol in the main binary. With `LD_DEBUG=bindings LD_BIND_NOW=1`, the actual `archy --version` execution exited 0 and showed both `libc.so.6` and `libgcc_s.so.1` binding `__tls_get_addr` to `/lib64/ld-linux-x86-64.so.2`. Thus the runtime loader is needed independently of whether its direct `DT_NEEDED` entry is redundant. These observations do not prove that removing that entry is safe across environments, and no linker or product change was made solely to silence the warning.
- The exact extracted binary also ran `archy --version` and `archy desktop capabilities --json` successfully in the isolated image. These smoke checks do not establish behavior on an Omarchy target.

The first optional binding probe used `grep` for a named relocation under `set -e`, exited before running the trace when no match was found, and a host `tee` pipeline masked that Docker exit. The corrected probe explicitly handled the no-match case, enabled host `pipefail`, and produced the binding evidence above. No input or artifact was changed by either probe.

Frozen for Gauss rereview: commit `ea6c37f5698f50b091e635945a55b8f5e41b8975`; `packaging/arch/package.py` SHA-256 `130719a86be79c5f181ad21ce30c9272ad79b3ff0790cd9f64101c504db7d7d7`; unchanged source archive `c7a8fcf375d3a86345e1deb3b456181dc2560f9ab27fdb511f1415218744bcd8`, PKGBUILD `617431485c0344f413ce17fa8e00b7d3cff47bbaa8c016572997d17aab9ec84f`, package `ff207478344593ccaf118c9c7019532178c527f0e61b3c962c3e2e4125424824`, and packaged binary `ba034490cb5511857ebf1da78d117f72924cf75d9c48a61e9e6da52825763ec7`. The retained review itself has SHA-256 `f1e8aa5fe7041b6a88e282a22661c997be030049be7ae4f4679762419c0fbaeb`.

Key fresh logs in `/var/tmp/archy-a21-namcap-review-qUC9ZXwN`:

| Log | SHA-256 |
| --- | --- |
| `recheck-exec.log` | `bfe73cb1ee6093c04662fb3a9c3e8cb11fd617c165a7661df72ea5a6dc42ca40` |
| `recheck-noexec.log` | `334544868f4ba4d2e3805bec33e1953b33a19b684270445862302490f6099842` |
| `namcap-exec.log` | `7b64d0b0a9e887a154dac6ab353c8401ef91d7f75f7b30e00ba84582805e70d4` |
| `dependencies.log` | `d47f71578ff31a9e4f27f54c311966b7b705ae9e97a489a8eb47132e6453656a` |
| `elf-versions.log` | `ff7e5e3b10c0e94282bce16c08d1de6db964658c7bb5b761f0ffb965319105bf` |
| `loader-symbols.log` | `b55d8ca4c21abc86a32008c9c6cf9ceaaa68b9264004a0972dbbdbfe936b4be9` |
| `runtime-binding.log` | `cab3f03fa3b179d3b02c11f91e10bd71b84791e7025a4fe0434f2427ca90994a` |
| `runtime.log` | `cf4df07e2549b46e8ada2f1eed1f73f6b1b66c1859e691c26c6f8458557db7e0` |

No recipe or product source changed, so no replacement archive or package was built. This is a triage of the exact private build, with separate independent rereview pending; no package installation, pacman transaction, distribution, commit, or push occurred. Actual Omarchy and complete legal/release gates remain open.
