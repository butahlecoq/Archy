# A21 private notice-enabled package review

**Verdict: ISSUES for the exact private artifact** with SHA-256 `ff207478344593ccaf118c9c7019532178c527f0e61b3c962c3e2e4125424824`. Its source, payload, and notice inventories matched the evidence I checked. A P2 inspection gap remains in the recorded clean `namcap` result.

## P2. The original inspection suppressed an unused-library warning

The handoff records an empty artifact `namcap` log. I reproduced that output using the same immutable image and its original `/tmp:rw,nosuid,size=128m` mount. `findmnt` reports that Docker mounted it `noexec`. With an otherwise equivalent executable tmpfs, `namcap` exited 0 but printed:

```text
archy W: Unused shared library '/usr/lib64/ld-linux-x86-64.so.2' by file ('usr/bin/archy')
```

The image's `Namcap/rules/unusedsodepends.py` extracts ELF files into a temporary file and runs `ldd -r -u`. On the original `noexec` tmpfs, running that command against the extracted binary printed `not a dynamic executable` and an execution-permission warning. On executable tmpfs, it reported the unused direct loader dependency. Thus the empty retained log is accurate for that invocation but is not evidence that the unused-library rule completed successfully. The [packaging plan](omarchy-packaging-plan.md) calls for lint warnings to be reviewed.

The executable-tmpfs inspection can be repeated with the exact artifact and image:

```sh
docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges --read-only \
  --tmpfs /tmp:rw,exec,nosuid,nodev,size=128m,mode=1777 \
  --mount type=bind,src=/var/tmp/archy-a21-notice-build-IJHK1bgL/work,dst=/work,readonly \
  -e HOME=/work/home \
  sha256:8471fd2be9c91daf7278e3067744448f7c437878aefeb67705b6f16f53de6eeb \
  /bin/bash -c 'set -eu; pkg=(/work/artifacts/*.pkg.tar.zst); cd /work; namcap "${pkg[0]}"'
```

Triage and record the loader warning for this artifact, or fix and rebuild if it identifies a problem. I did not establish that the warning causes a runtime failure: the packaged binary ran `--version` and `desktop capabilities --json` successfully in the isolated image. A clean `namcap` claim should be based on an inspection environment in which its ELF checks can run.

## Verified artifact and source

- The worktree was on `codex/a21-notice-package` at commit `ea6c37f5698f50b091e635945a55b8f5e41b8975`, tree `ffd3884137eea78a12de248676bf95a0f8533b81`. Its only reported change before this review was the untracked build handoff. I independently compared the archive's 73 selected tracked build inputs with `git archive` of that commit, and all 6,756 vendor file hashes with `/tmp/archy-core-arch-source/vendor`. The 6,830 archive files matched the recorded inventory's modes and hashes, including the relative Cargo vendor config. This supports the handoff's source identity; the untracked handoff is not part of the archive.
- The source archive SHA-256 was `c7a8fcf375d3a86345e1deb3b456181dc2560f9ab27fdb511f1415218744bcd8`, and the source inventory was `ec43ff8adbc28e058a0feaebb26be915044e078bb7c746be4c00283ea1f00060`. The PKGBUILD was `617431485c0344f413ce17fa8e00b7d3cff47bbaa8c016572997d17aab9ec84f`. Its source hash, offline locked build, notice admission before build, and stage arguments match the handoff. The retained makepkg log hash was `407fa806dc226dedb43aef9c7a6c4d07d01c154875849f675cae6577c361b198`; the build itself was not repeated in this review.
- I extracted the exact package in the immutable Arch image with network disabled and the evidence bind read-only. All 19 owned paths, modes, and SHA-256 values matched `artifact-inventory.tsv` and the source manifest. The 18 nonbinary files matched the archive inputs or generated notices byte for byte. The archive had only those 19 files and `.BUILDINFO`, `.MTREE`, and `.PKGINFO`; it had no install script. The packaged binary hash was `ba034490cb5511857ebf1da78d117f72924cf75d9c48a61e9e6da52825763ec7`; the retained pre-strip binary hash was `d0e6ac7f3aef561988a839adbf79eada0a503da3b1e058f59fb1cc7e3aacba7e`.
- The packaged notice JSON hash was `e4d95486832a64364e38fe4b565369ad8d075b6529edbf4d84973e69110cffd6`. Its bundle hash matched the packaged text, `ea7019a4ff8e69a9b0d49c4e54ab5869c1c2a0726997c7c9e30cd425402da710`. All 89 crate source checksums matched the captured Cargo.lock and vendor checksum manifests, and all 173 recorded crate text hashes matched captured files. I independently ran `cargo metadata --offline --locked` against a disposable extraction of the captured archive in the network-disabled image; its `archy-cli` normal dependency closure exactly matched the 89 package inventory rows. The seven primary and three Rust text inputs were present.
- The source archive and extracted package had no matches in my bounded scan for private-key headers, common AWS, OpenAI, GitHub, or Slack token patterns, and selected sensitive filenames. This is not proof that every possible secret or proprietary file is absent. The packaged binary contained 74 `/work/src/` strings and no `/home/butahlecoq/` string, matching the retained makepkg `$srcdir` warning. Those generic build paths remain a privacy and reproducibility review item.

## Remaining inspection results and limits

On executable tmpfs, `namcap` on the PKGBUILD exited 0 with `Missing Maintainer tag`. `pacman -Qip` reported x86_64, `glibc` and `libgcc`, no install script, `Unknown Packager`, and no signature. `readelf -d` reported `libc.so.6`, `ld-linux-x86-64.so.2`, `libm.so.6`, and `libgcc_s.so.1`, with no RPATH or RUNPATH. The packaged binary printed `archy 0.1.0` and returned the two expected desktop status capabilities. These are independent development-image checks. `pacman -Qip` inspected the archive; it did not install it.

No fresh Arch build, package distribution, host installation, pacman transaction, complete linked-code or legal audit, or actual Omarchy target acceptance was performed. The prior `noexec` result and the executable-tmpfs warning should both remain in the evidence until the warning is resolved or explicitly justified.
