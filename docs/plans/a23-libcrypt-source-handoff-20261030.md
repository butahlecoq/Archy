# A23 issue 21: libcrypt.so.1 source and ABI evidence

Scope: source and provenance for Blender 4.5.15's bundled Python `_crypt` extension. This branch starts at PR head `0cf4d1e7f02009fc96732b50aa5b615523fb9ec2`. All downloads, extraction, and builds used private scratch at `.t/a23-libcrypt-source-20261030` (mode 0700). Nothing here changes runtime allowlists, the runtime recipe, `build_ready`, or the distributed payload.

Bounded plan: pin upstream and Arch source identities; build and test the compatibility configuration offline; compare the resulting ELF contract with Blender's requester and the signed-package lead; record remaining review gates. Independent code and provenance review is pending.

## Source identity and build

Arch's [libxcrypt `PKGBUILD`](https://gitlab.archlinux.org/archlinux/packaging/packages/libxcrypt/-/blob/main/PKGBUILD) pins upstream [v4.5.2](https://github.com/besser82/libxcrypt/releases/tag/v4.5.2), SHA-256 `71513a31c01a428bccd5367a32fd95f115d6dac50fb5b60c779d5c7942aec071`. The downloaded upstream tarball matched. The downloaded `PKGBUILD` SHA-256 was `db145f4d491ae06887464516800ffef3837e2a7356c74de2664100cc77839439`; this digest identifies the inspected file, not a pin on Arch's moving `main` branch. The detached tarball signature SHA-256 was `11df2b889929d4923eed6298ba44c6e746c9ca5b9de41bd9b1b1a694a2aea65e`. GPG verified it against key fingerprint `678CE3FEE430311596DB8C16F52E98007594C21D`, named by the Arch recipe and present in the upstream maintainer's GitHub key export. That is source signature evidence, not independent identity clearance.

Arch builds the split compatibility library with `--prefix=/usr --disable-static --enable-hashes=strong,glibc --enable-obsolete-api=glibc --disable-failure-tokens`. The new offline [source check](../../packaging/arch/libxcrypt_compat_source.sh) pins the tarball and license file hashes, runs that configuration in a caller-owned fresh directory, runs `make check`, installs only to `DESTDIR`, then checks SONAME, direct ELF needs, and `crypt_r@@XCRYPT_2.0`. Run it with:

```sh
packaging/arch/libxcrypt_compat_source.sh \
  /path/to/libxcrypt-4.5.2.tar.xz \
  /private/scratch/new-libxcrypt-build
```

This Ubuntu host has GCC 15.2.0. The unmodified configure flags failed at `crypt-gost-yescrypt.c:134` and `crypt-sm3-yescrypt.c:139` because `-Werror=discarded-qualifiers` rejects upstream assignments. The source check adds `CFLAGS='-O2 -Wno-error=discarded-qualifiers'`; it changes warning handling only. The first successful local build produced `libcrypt.so.1.1.0` SHA-256 `e61c50236145532adbab484d26494fc011391f0d4f0eac9418a9d08b3a29b644`. Upstream check results: 55 total, 41 pass, 14 skip, 0 fail, 0 error. The installed `DESTDIR` copy had the same hash. The scripted repeat passed the same 55 tests and produced SHA-256 `2147324272b5712fa430e0e840e225e12827847b22fcea55101f6d1d106441e8`. These host build bytes are not reproducible across the two build paths; neither is a candidate payload.

The local build's SONAME is `libcrypt.so.1`; its only direct ELF need is `libc.so.6`; it exports `crypt_r@@XCRYPT_2.0`. It requires `GLIBC_2.43`, however, so its ABI cannot be assumed compatible with the intended Arch/Omarchy target. Its bytes do not match the Arch package binary. The build validates the source and requested symbol, not reproducible Arch package bytes or target suitability.

## Binary requester and Arch package lead

Blender archive SHA-256 is `0d2b8bb2efe97e6c8eb57de6b7322f8800e1984e3125b60decca5f6771e4773e`. Extracted member `4.5/python/lib/python3.11/lib-dynload/_crypt.cpython-311-x86_64-linux-gnu.so` SHA-256 is `e47eed4df103d39320bdf76e05b54504dadd125568005997c8a01e8e02c797ac`. Its direct needs are `libcrypt.so.1`, `libpthread.so.0`, and `libc.so.6`; its `libcrypt.so.1` version need is `XCRYPT_2.0`, specifically `crypt_r@XCRYPT_2.0`. Its RUNPATH is `$ORIGIN/../../../../../lib`.

The previously signature-verified Arch [libxcrypt-compat 4.5.2-1 package](https://archlinux.org/packages/core/x86_64/libxcrypt-compat/) was downloaded again from the Arch archive. Its SHA-256 is `89476037bf6a96561b86b8b61a2b1d5c34ea2d07036bc9427abdf799d57e934c`; its extracted `usr/lib/libcrypt.so.1.1.0` SHA-256 is `f4c8627f8c9069936abbade993946c05e55dea3ea9b0d92044aff95d2db0f3d4`. This binary has SONAME `libcrypt.so.1`, only `libc.so.6` as a direct ELF need, and exports `crypt_r@@XCRYPT_2.0`. Its highest observed required GLIBC version is `GLIBC_2.38`; the local host build reaches `GLIBC_2.43`. Arch package metadata declares `libxcrypt` as a package dependency even though the compatibility ELF has no direct `libcrypt.so.2` need. This does not establish exact source-to-package reproducibility.

## License and notice inventory

The pinned source's `LICENSING` SHA-256 is `9144e66d8140feb84f0e4e344906ac425378896320032e0248567ef195cf0135`; `COPYING.LIB` SHA-256 is `dc626520dcd53a22f727af3ee42c770e56c97a64fe3adb063799d8ab032fe551`. `LICENSING` identifies the overall library as LGPL-2.1-or-later and lists LGPL, BSD 0/2/3-clause and public-domain source contributions; build machinery has additional licenses and exceptions. Preserve both complete files with any eventual source/binary notice set and review the individual file terms. The Arch compatibility package contains only the library and symlink, not those notice texts. Its `libc.so.6` dependency and any wider packaged runtime need separate notice closure. No legal or distribution approval is inferred.

## Reproduction and remaining gates

The commands below were run from private scratch; the source check above is the repeatable build/ABI check. The prior Arch signature verification is recorded in `a23-runtime-rereview-fixes-20261030.md`.

```sh
curl -fL -o PKGBUILD https://gitlab.archlinux.org/archlinux/packaging/packages/libxcrypt/-/raw/main/PKGBUILD
curl -fL -o libxcrypt-4.5.2.tar.xz https://github.com/besser82/libxcrypt/releases/download/v4.5.2/libxcrypt-4.5.2.tar.xz
curl -fL -o libxcrypt-4.5.2.tar.xz.asc https://github.com/besser82/libxcrypt/releases/download/v4.5.2/libxcrypt-4.5.2.tar.xz.asc
curl -fL -o upstream-keys.asc https://github.com/besser82.gpg
GNUPGHOME=/private/scratch/gnupg gpg --import upstream-keys.asc
GNUPGHOME=/private/scratch/gnupg gpg --verify libxcrypt-4.5.2.tar.xz.asc libxcrypt-4.5.2.tar.xz
curl -fL -o libxcrypt-compat-4.5.2-1-x86_64.pkg.tar.zst https://archive.archlinux.org/packages/l/libxcrypt-compat/libxcrypt-compat-4.5.2-1-x86_64.pkg.tar.zst
sha256sum PKGBUILD libxcrypt-4.5.2.tar.xz libxcrypt-4.5.2.tar.xz.asc libxcrypt-compat-4.5.2-1-x86_64.pkg.tar.zst
readelf -d /private/scratch/new-libxcrypt-build/stage/usr/lib/libcrypt.so.1.1.0
readelf --version-info /private/scratch/new-libxcrypt-build/stage/usr/lib/libcrypt.so.1.1.0
```

Review is still required for source/key trust, target-native build and exact source-to-package correspondence, complete bundled and transitive notices, the conservative all-ELF MaterialX failure described in `a23-runtime-rereview-fixes-20261030.md`, and the broader Blender runtime acceptance. The parent PR's `build_ready=false`, trusted review allowlists, and distribution gates remain untouched. This evidence is not an approval to ship the Arch binary or the host-built binary.
