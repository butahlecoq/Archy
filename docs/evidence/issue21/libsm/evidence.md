# Historical libSM source and notice evidence

Scope: provisional inventory commit `1b10a901dfea3545723e1491601766c7d2b198cf`, historical image `sha256:98bafd0772026dd0c92b4beaffe415163272a6b9f21d4f3b089ba89affcc759f`. This is evidence for that image, not a current target assertion.

| Item | Observed value |
| --- | --- |
| Inventory row | `/usr/lib/libSM.so.6.0.1`, `libsm 1.2.6-1`, SHA-256 `1366597b21814c4290eda5f83d8bd63144273f99ab15197f49fc26037459d4c3` |
| [Arch package](https://archive.archlinux.org/packages/l/libsm/libsm-1.2.6-1-x86_64.pkg.tar.zst) | SHA-256 `ce334b07a9701ba6ef4d610257500f7b035dc1cdc0a658e056d293547cde0976` |
| [Arch package signature](https://archive.archlinux.org/packages/l/libsm/libsm-1.2.6-1-x86_64.pkg.tar.zst.sig) | SHA-256 `51b51bfb204de1202336b1b901da4a9d33b6ef57d3dc458bc408a6154d32c01a`; `gpgv` reported a good signature by Andreas Radke, key `ADC8A1FCC15E01D45310419E94657AB20F2A092B`, against the historical image's `/etc/pacman.d/gnupg/pubring.gpg` |
| [Arch recipe](https://gitlab.archlinux.org/archlinux/packaging/packages/libsm/-/tree/1.2.6-1) | tag `1.2.6-1`, commit `730ddcfb3fcfc4d7a7ac4c6c7e3c36fcc64a6b1a`; [local snapshot](PKGBUILD) SHA-256 `e822a1dd5331de6f8abfc19bb96c1782d224dbea0076574a5e24797ad1d1240d` |
| [X.Org source](https://xorg.freedesktop.org/releases/individual/lib/libSM-1.2.6.tar.xz) | SHA-256 `be7c0abdb15cbfd29ac62573c1c82e877f9d4047ad15321e7ea97d1e43d835be`; SHA-512 `e544a1dc49a03390f76af35837bfd01f749b806d88d3ee806f20311c47ff53d0aeea4744feb875958031b17d50b57a89dcc41d81241c09333c88b268c44653a7` matches recipe |
| [X.Org source signature](https://xorg.freedesktop.org/releases/individual/lib/libSM-1.2.6.tar.xz.sig) | SHA-256 `3e01af31a5a36aeb22cb33f9565b2c54d36403a0031fbb268749eff5b71772ad`; good cryptographic signature from Alan Coopersmith signing subkey `2F7DBCAD5B30E668BBD516C1CFDF148828C642A7`, under recipe key `4A193C06D35E7C670FA4EF0BA2FB9E081F2D130E`; GnuPG currently marks the key expired |
| [Package metadata](PKGINFO) | SHA-256 `3244bf701de9ddf13d2bab4c129d507e625f5a806cdf7ac77a721fc544a8c692`; declares `libsm 1.2.6-1` |
| [Build metadata](BUILDINFO) | SHA-256 `da93adac842a0f0d1caee40985e94188309b7bf5b97d709d76a9b167ac4e0842`; embedded `pkgbuild_sha256sum` exactly matches the tagged `PKGBUILD` snapshot |
| [Full packaged notice](COPYING) | SHA-256 `136fb258a67a2810302c6cd99525b068e6f875b8f1031a93b34edad3ffc740a5`; byte identical to `libSM-1.2.6/COPYING` in the signed source archive |

The archive member `usr/lib/libSM.so.6.0.1` hashes exactly to the provisional image row. The package also contains `usr/lib/libSM.so.6` and `usr/lib/libSM.so` aliases. The recipe declares `libice`, `util-linux-libs`, `xorgproto`, and `glibc` dependencies, with `xorg-util-macros` and `xtrans` as build dependencies. Its `package()` installs `COPYING` to `usr/share/licenses/libsm/COPYING`, and that exact package member is preserved here.

To repeat the key checks, download the two Arch package files and two X.Org source files at the links above; hash them, then run `gpgv --keyring <historical-image-pubring.gpg> <package>.sig <package>` and `gpg --verify <source>.sig <source>` after importing the recipe's `keys/pgp/4A193C06D35E7C670FA4EF0BA2FB9E081F2D130E.asc` into a private GnuPG home. Extract `usr/lib/libSM.so.6.0.1`, `.PKGINFO`, `.BUILDINFO`, and `usr/share/licenses/libsm/COPYING` from the Arch archive and compare their SHA-256s and bytes to the inventory and local snapshots. The inspected archive and source files remain in private disk scratch; they are not release payloads.

This proves package-byte correspondence to the historical image and the recipe's signed source pin. It does not prove a reproducible rebuild, complete source-to-binary derivation, notice sufficiency, current target availability, or legal approval.
