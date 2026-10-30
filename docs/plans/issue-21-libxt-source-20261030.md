# Issue #21: libXt source candidate

Scope: source and provenance for the `libXt.so.6` edge in Blender 4.5.15's bundled MaterialX 1.39.2 libraries. This branch starts at `0cf4d1e7f02009fc96732b50aa5b615523fb9ec2`. It adds a source-only build probe and preserves all runtime review gates. No runtime payload, package, trusted review, target validation, or distribution approval is produced.

## Source and notice

* Upstream: X.Org `libXt` 1.3.1, `https://www.x.org/releases/individual/lib/libXt-1.3.1.tar.xz`; SHA-256 `e0a774b33324f4d4c05b199ea45050f87206586d81655f8bef4dba434d931288`; SHA-512 `c220292f60b0f53134cf9364831a32bbaa9fa6bbb3a7143e917920957b7a48c616e946042747089f29ea9d8a18ecd64de620bcaf56d82462e7107de906f5db38`.
* Arch packaging reference: `https://gitlab.archlinux.org/archlinux/packaging/packages/libxt/-/raw/main/PKGBUILD`, fetched SHA-256 `663cde7a703c914bfb038a95fdf13be602794f5441237082a9bf8dbebcb8b629`. It pins 1.3.1-1, the same SHA-512, and the upstream signing primary key `3BB639E56F861FA2E86505690FDD682D974CA72A`. The earlier source-signature verification is recorded in `docs/plans/a23-blender-cpu-closure-followon-20261030.md:21`. This run checked the downloaded tarball's SHA-256 and SHA-512, but did not independently rerun GPG signature verification.
* The tarball's sole `COPYING` file is committed byte for byte at `packaging/arch/libxt-source/COPYING`, SHA-256 `75c5574ca04731d739b1420f55f2b7b47f30df895817f1b03d0d7f5c1fbee534`. The probe checks both this copy and the extracted source notice. Arch labels the component `MIT`, `HPND-sell-variant`, `SMLNJ`, `MIT-open-group`, and `X11`; these labels need legal review against the full text and any eventual distribution format. They do not cover the transitive libraries.

## Build and ELF dependency findings

`configure.ac` requires pkg-config modules `sm ice x11 xproto kbproto`; the release tarball already contains generated `configure`, so rebuilding autotools is unnecessary. Arch's package metadata lists runtime dependencies `glibc`, `libice`, `libsm`, `libx11`, `xorgproto`, and build dependency `xorg-util-macros`. The source build also needs a C compiler, make, pkg-config implementation, linker, and the dependency headers and libraries. The source-only recipe uses `--prefix=/usr --sysconfdir=/etc --disable-static` and disables documentation generators. It requires a new output directory and never installs to the host.

The actual scratch build on this Ubuntu host produced `stage/usr/lib/libXt.so.6.0.0` with SONAME `libXt.so.6`. `readelf -d` showed direct DT_NEEDED: `libSM.so.6`, `libICE.so.6`, `libX11.so.6`, `libc.so.6`; no RPATH/RUNPATH. The first-level observed host closure is `libSM -> libICE, libuuid, libc`; `libICE -> libc`; `libX11 -> libxcb, libc`; `libxcb -> libXau, libXdmcp, libc`; `libXau/libXdmcp -> libc`; `libuuid -> libc, ld-linux-x86-64.so.2`. This is an observation on host binaries, not a pin or proof for Arch runtime packages. Source and notice provenance for these transitive libraries remains open.

The host lacks `pkg-config` and `xproto.pc`/`kbproto.pc`. For a feasibility probe only, `pkgconf` 2.3.0 was built in private scratch from `https://distfiles.ariadne.space/pkgconf/pkgconf-2.3.0.tar.xz`, SHA-256 `3a9080ac51d03615e7c1910a0a2a8df08424892b5f13b0628a204d3fcce0ea8b`. `XT_CFLAGS=-I/usr/include` and `XT_LIBS='-lSM -lICE -lX11'` bypassed module checks using existing host headers and libraries. The first manual host candidate SHA-256 was `e219ba432d1bc372860ad04b090b75be63efe60c0e5a8d53ee949b937b1d6ab8`; the later recipe run at a different scratch path produced `94e5fdcbc1f368fa45a3e8c60baba5b6d20014a9f03c5365b3c832b6a7c57c75`. Both had the same expected SONAME and direct dependencies, but different build IDs and bytes. A reproducible binary build is not established. The ordinary recipe path clears those overrides; `--host-probe` must be explicit to use them.

## Reproduce the scoped probe

Download the pinned upstream tarball into a private scratch directory, verify its SHA-256 above, then on an Arch build environment with the listed build dependencies and no host installation:

```sh
python3 packaging/arch/libxt_source.py \
  --tarball /private/scratch/libXt-1.3.1.tar.xz \
  --output /private/scratch/libxt-candidate
```

`result.json` records source, notice, candidate digest, SONAME and direct DT_NEEDED, with `status: source_candidate_only`. The recipe does not download sources, alter the main runtime recipe, or declare the package ready. The positive test can run with `LIBXT_SOURCE_TARBALL` set; a negative digest test runs without it. This run set `LIBXT_HOST_PROBE=1` for the positive test and used `--host-probe` for the persistent recipe run, plus the scratch-built `PKG_CONFIG` and two `XT_*` overrides above. The staged output, `repro/result.json`, and raw configure/build logs stay in `/home/butahlecoq/work/archy/.t/a23-issue21-libxt-source-20261030`.

## Remaining gates

Independent review is required for the recipe, notice and source identity. An isolated Arch build with pinned build inputs, package and transitive source/notice review, actual staged-payload integration, all-ELF closure, and the broader CPU journey are still needed. Existing missing `libcrypt.so.1`, optional HIP dependency, and MaterialX import-order gate behavior are separate blockers recorded in the prior handoffs. The runtime manifest remains `build_ready: false`; A12 and distribution trusted review allowlists remain empty. No host package or service was changed, and no distribution, legal, or target approval is implied.
