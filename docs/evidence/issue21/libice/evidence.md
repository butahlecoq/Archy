# Historical libICE source and notice evidence

Scope: provisional inventory commit `1b10a901dfea3545723e1491601766c7d2b198cf`, historical image `sha256:98bafd0772026dd0c92b4beaffe415163272a6b9f21d4f3b089ba89affcc759f`. This is evidence for that image, not a current target assertion.

| Item | Observed value |
| --- | --- |
| Inventory row | `/usr/lib/libICE.so.6.3.0`, `libice 1.1.2-1`, SHA-256 `71c71c05e3cbd25d19460ef5bdf3b3a4538c95a86473a4e4222bf85c2c419a9c` |
| [Arch package](https://archive.archlinux.org/packages/l/libice/libice-1.1.2-1-x86_64.pkg.tar.zst) | SHA-256 `bb613be39e5bc1707a39f895c178674bbda52f022da51612e7a10a386608e107` |
| [Arch package signature](https://archive.archlinux.org/packages/l/libice/libice-1.1.2-1-x86_64.pkg.tar.zst.sig) | SHA-256 `61bc66967145883780a56a1eb8c726e9cdf8b9c74ee984efd9a5d4d6e1682c62`; `gpgv` reported a good signature by Andreas Radke, key `ADC8A1FCC15E01D45310419E94657AB20F2A092B`, against the historical image's `/etc/pacman.d/gnupg/pubring.gpg` |
| [Arch recipe](https://gitlab.archlinux.org/archlinux/packaging/packages/libice/-/tree/1.1.2-1) | tag `1.1.2-1`, commit `a1f6b02cf007e620fdf5a42078d7032d48a71bd4`; [local snapshot](PKGBUILD) SHA-256 `dfb72b3bac6e0e233e9f3ac2f2f1594be74672a644573556b9ed691720df7bcd` |
| [X.Org source](https://xorg.freedesktop.org/releases/individual/lib/libICE-1.1.2.tar.xz) | SHA-256 `974e4ed414225eb3c716985df9709f4da8d22a67a2890066bc6dfc89ad298625`; SHA-512 `340f51ffa1f14ed442ab8bcea92dd63df147c48242e232e818cafe02f43de7ab6e99c5430b9cb8d0dc661295239d2b3f6bdb6a092ce51a98afa06235257e9b1f` matches recipe |
| [X.Org source signature](https://xorg.freedesktop.org/releases/individual/lib/libICE-1.1.2.tar.xz.sig) | SHA-256 `b1edd37141e367a6f41e7edce5a614d6c3ab73be3a28060214c3ed6c737124c4`; good cryptographic signature from Alan Coopersmith signing subkey `2F7DBCAD5B30E668BBD516C1CFDF148828C642A7`, under recipe key `4A193C06D35E7C670FA4EF0BA2FB9E081F2D130E`; GnuPG currently marks the key expired |
| [Package metadata](PKGINFO) | SHA-256 `f79a47de89c897e2963fc31e81506f58894ed87acd18fcc579665c2591f9d937`; declares `libice 1.1.2-1` |
| [Build metadata](BUILDINFO) | SHA-256 `ece810b345e2d5995b12d30fa51dd4e70880c3614b0f20c194aa60d132368abc`; embedded `pkgbuild_sha256sum` exactly matches the tagged `PKGBUILD` snapshot |
| [Full packaged notice](COPYING) | SHA-256 `60105b7ea93cb07a67fee8443b092b727e3db7f0dff4fbe05bc6cd7747fb53c8`; byte identical to `libICE-1.1.2/COPYING` in the signed source archive |

The archive member `usr/lib/libICE.so.6.3.0` hashes exactly to the provisional image row. The package also contains `usr/lib/libICE.so.6` and `usr/lib/libICE.so` aliases. The recipe declares `glibc` and `xorgproto` dependencies, with `xtrans` and `xorg-util-macros` as build dependencies. Its `package()` installs `COPYING` to `usr/share/licenses/libice/COPYING`, and that exact package member is preserved here.

To repeat the key checks, download the two Arch package files and two X.Org source files at the links above; hash them, then run `gpgv --keyring <historical-image-pubring.gpg> <package>.sig <package>` and `gpg --verify <source>.sig <source>` after importing the recipe's `keys/pgp/4A193C06D35E7C670FA4EF0BA2FB9E081F2D130E.asc` into a private GnuPG home. Extract `usr/lib/libICE.so.6.3.0`, `.PKGINFO`, `.BUILDINFO`, and `usr/share/licenses/libice/COPYING` from the Arch archive and compare their SHA-256s and bytes to the inventory and local snapshots. The inspected archive and source files remain in private disk scratch; they are not release payloads.

This proves package-byte correspondence to the historical image and the recipe's signed source pin. It does not prove a reproducible rebuild, complete source-to-binary derivation, notice sufficiency, current target availability, or legal approval.
