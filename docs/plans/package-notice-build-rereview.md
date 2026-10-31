# A21 package warning disposition rereview

**Scoped verdict: P2 ADDRESSED for artifact `ff207478344593ccaf118c9c7019532178c527f0e61b3c962c3e2e4125424824`.** The handoff now discloses that the original `noexec` inspection suppressed the ELF check, records the real warning, and explains the dependency evidence. The warning remains present; the disposition does not claim a clean `namcap` result or prove that the direct loader entry can safely be removed.

I reran the exact package in immutable image `sha256:8471fd2be9c91daf7278e3067744448f7c437878aefeb67705b6f16f53de6eeb`, with the network disabled, the evidence mounted read-only, and an executable temporary filesystem. `namcap` exited 0 and reported `archy W: Unused shared library '/usr/lib64/ld-linux-x86-64.so.2' by file ('usr/bin/archy')`. The recipe check exited 0 with `Missing Maintainer tag`. With the original `--tmpfs /tmp:rw,nosuid,size=128m`, `findmnt` showed `noexec`; `namcap` again produced no artifact output, while `ldd -r -u` on the copied binary exited 1 with an execution-permission warning and `not a dynamic executable`. The corrected [handoff](package-notice-build-handoff.md) explicitly records both results.

The executable-tempfs check found `/lib64/ld-linux-x86-64.so.2` as the ELF interpreter and `libc.so.6`, `ld-linux-x86-64.so.2`, `libm.so.6`, and `libgcc_s.so.1` as direct `DT_NEEDED` entries. Plain `ldd` resolved them in the image. `pacman -Qo` identified the loader, libc, and libm as `glibc 2.44+r50+g1848099f063e-1`, and libgcc_s as `libgcc 16.2.1+r23+gd564253eb6c8-1`. The unchanged PKGBUILD retains both `glibc` and `libgcc` dependencies.

`readelf` showed the binary's undefined `__tls_get_addr@GLIBC_2.3`, a loader version requirement, and no named dynamic relocation for that symbol in the main binary. The loader exports the symbol. With `LD_DEBUG=bindings` and `LD_BIND_NOW=1`, `archy --version` exited 0 and the trace showed libc and libgcc_s binding `__tls_get_addr` to the loader. This establishes a runtime role for the loader in this image; it does not establish whether the main binary's direct `DT_NEEDED` entry is necessary on every target. Retaining the declared package dependencies and documenting the warning is supported by the observed evidence. No linker or dependency removal was needed for this disposition.

## Identity and prior inventory

The handoff SHA-256 is `5581f79823fe9ce87f399b603415d3492b2ffeb83f8c7b4bd221c69e6671e474`, matching the supplied final value. The review and source identities remain:

| Item | SHA-256 |
| --- | --- |
| Original independent review | `f1e8aa5fe7041b6a88e282a22661c997be030049be7ae4f4679762419c0fbaeb` |
| Source archive | `c7a8fcf375d3a86345e1deb3b456181dc2560f9ab27fdb511f1415218744bcd8` |
| Source inventory | `ec43ff8adbc28e058a0feaebb26be915044e078bb7c746be4c00283ea1f00060` |
| `packaging/arch/package.py` | `130719a86be79c5f181ad21ce30c9272ad79b3ff0790cd9f64101c504db7d7d7` |
| Generated PKGBUILD | `617431485c0344f413ce17fa8e00b7d3cff47bbaa8c016572997d17aab9ec84f` |
| Package artifact | `ff207478344593ccaf118c9c7019532178c527f0e61b3c962c3e2e4125424824` |
| Packaged binary | `ba034490cb5511857ebf1da78d117f72924cf75d9c48a61e9e6da52825763ec7` |
| Artifact inventory | `5f55e3464597a879c488a0951ab08b53945407b97bead47bac69e8db29f4d8ab` |

The worktree remained at commit `ea6c37f5698f50b091e635945a55b8f5e41b8975`. I extracted the package again in the isolated image and checked all 19 owned paths, modes, and hashes against the prior inventory. The packaged notice JSON still lists 89 crates and its bundle digest matches the packaged text. The binary still contains 74 occurrences of `/work/src/` and none of `/home/butahlecoq/`; the generic build-source path warning remains a privacy and reproducibility review item.

This verdict addresses the warning investigation for the exact private artifact. It does not establish an Arch or Omarchy installation, package distribution, target acceptance, or a complete linked-code and legal audit. No product source, commit, push, child agent, host installation, or package transaction was changed by this rereview.
