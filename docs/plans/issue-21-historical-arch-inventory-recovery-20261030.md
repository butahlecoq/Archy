# Issue #21: historical Arch system library recovery

Base: `211c816f0ff20b515e19f6167b05c4648efe59be`. This source-only recovery used a new isolated worktree, private `/var/tmp` scratch, and read-only existing artifacts. It did not rebuild IfcOpenShell or a runtime package.

## Artifact chain

The historical runtime handoff identifies derived Arch image `sha256:98bafd0772026dd0c92b4beaffe415163272a6b9f21d4f3b089ba89affcc759f` and a superseded runtime package SHA-256 `706fd3ae441b4272453a45656beb79e32035f65a2f38d1a81b23283665dfda42`. Both exact artifacts remain on local disk. The package is at `.worktrees/a23-runtime-packaging-20261030-unique/.runtime-assets/archy-runtime-0.1.0-1-x86_64.pkg.tar.zst`; the image ID was confirmed by `docker image inspect`. Hashing the package matched the handoff. Extracting `usr/lib/archy/runtime/inventory.json` from that package with image-provided `bsdtar -xOf` yielded SHA-256 `2cb7c672f6a1866cc9b0214522b9e6f26b669a47db135f6a4fca86d90857afb4`, also the handoff value. It matched the existing staged inventory byte for byte. The inventory contains 16,715 payload file rows and exactly 54 `system_files` rows.

[Recovered 54-row record](../evidence/issue21/arch-system-files-54.json) has SHA-256 `2c077c01a7f9ee8e5bb9dd788ec627b67fbc4acf9f1d4ff01ac7ec7d967fb824`. Every historical path, SHA-256 and mode was checked against the exact file in the pinned image. `pacman -Qoq` and `pacman -Q` in that image supplied the owner and installed version; `readelf -dW` and `readelf --version-info -W` supplied SONAME, direct needs, search paths and symbol-version requirements. The record has 54 unique resolved `/usr/lib` paths, 54 unique SONAMEs and 36 package owners. Every direct `DT_NEEDED` name in those 54 files has a provider among those 54 SONAMEs. This is the **system-file subgraph** from the historical package; it does not establish that every packaged ELF or lazy load is covered.

| SONAME | Resolved path | Owner and version | SHA-256 |
| --- | --- | --- | --- |
| `libSM.so.6` | `/usr/lib/libSM.so.6.0.1` | `libsm 1.2.6-1` | `1366597b21814c4290eda5f83d8bd63144273f99ab15197f49fc26037459d4c3` |
| `libICE.so.6` | `/usr/lib/libICE.so.6.3.0` | `libice 1.1.2-1` | `71c71c05e3cbd25d19460ef5bdf3b3a4538c95a86473a4e4222bf85c2c419a9c` |
| `libX11.so.6` | `/usr/lib/libX11.so.6.4.0` | `libx11 1.8.14-1` | `8c202bd93c4f4f3967ac66a20588da1e8f44a17b57ecc292d5cce51aa1168b74` |
| `libxcb.so.1` | `/usr/lib/libxcb.so.1.1.0` | `libxcb 1.17.0-1` | `40df5eff95408b67d969e559f2689b3e50e6151251b1f4e03608ce5cd9c9a133` |
| `libXau.so.6` | `/usr/lib/libXau.so.6.0.0` | `libxau 1.0.12-1` | `3e229dba702d7b56f1c58ac0e004ae691ae2ad5ede7b50968e1cd7d8b2ccdecc` |
| `libXdmcp.so.6` | `/usr/lib/libXdmcp.so.6.0.0` | `libxdmcp 1.1.5-2` | `0c572ba75de0f967c9c4d088161d3e435ea721bd99c169aae707aba6870a71b1` |
| `libuuid.so.1` | `/usr/lib/libuuid.so.1.3.0` | `util-linux-libs 2.42.4-1` | `7881e0279d5ce79b0d8ace923e94df9b543bc8dd8152df0894358b862e1725be` |

`libSM.so.6` directly needs `libICE.so.6`, `libuuid.so.1`, and `libc.so.6`; `libX11.so.6` directly needs `libxcb.so.1` and `libc.so.6`; `libxcb.so.1` directly needs `libXau.so.6`, `libXdmcp.so.6`, and `libc.so.6`. `libICE`, `libXau` and `libXdmcp` each need `libc.so.6`; `libuuid.so.1` needs `libc.so.6` and `ld-linux-x86-64.so.2`. The exact glibc and loader rows and all symbol versions are in the JSON. These are observed Arch ELF edges, superseding the conditional Ubuntu graph for this historical image.

## Reproduction

From this worktree, use the existing package path below. The exact image and package are prerequisites. The checked-in [inspection script](../../packaging/arch/inspect_issue21_arch_libs.py) aborts on any historical path, mode or hash mismatch:

```sh
scratch_dir=$(mktemp -d /var/tmp/archy-issue21-recheck-XXXXXX)
chmod 700 "$scratch_dir"
package_path=/home/butahlecoq/work/archy/.worktrees/a23-runtime-packaging-20261030-unique/.runtime-assets/archy-runtime-0.1.0-1-x86_64.pkg.tar.zst
image_id=sha256:98bafd0772026dd0c92b4beaffe415163272a6b9f21d4f3b089ba89affcc759f
printf '%s  %s\n' 706fd3ae441b4272453a45656beb79e32035f65a2f38d1a81b23283665dfda42 "$package_path" | sha256sum -c -
docker image inspect "$image_id" --format '{{.Id}}'
docker run --rm --read-only --network none \
  --mount type=bind,src="$package_path",dst=/evidence/package.pkg.tar.zst,readonly \
  --entrypoint /usr/bin/bsdtar "$image_id" \
  -xOf /evidence/package.pkg.tar.zst usr/lib/archy/runtime/inventory.json > "$scratch_dir/inventory.json"
printf '%s  %s\n' 2cb7c672f6a1866cc9b0214522b9e6f26b669a47db135f6a4fca86d90857afb4 "$scratch_dir/inventory.json" | sha256sum -c -
docker run --rm -i --read-only --network none \
  --mount type=bind,src="$scratch_dir/inventory.json",dst=/evidence/inventory.json,readonly \
  --entrypoint /usr/bin/python3 \
  "$image_id" \
  - /evidence/inventory.json < packaging/arch/inspect_issue21_arch_libs.py > "$scratch_dir/recovered-54.json"
sha256sum "$scratch_dir/recovered-54.json"
cmp "$scratch_dir/recovered-54.json" docs/evidence/issue21/arch-system-files-54.json
python3 -m unittest tests/packaging/test_issue21_arch_inventory.py
```

The source-only test checks the committed record's uniqueness and direct-need closure. It is not a substitute for rerunning the image comparison.

## Remaining limits

The package is explicitly superseded by later A12 work. Its 54 rows prove its historical build-image bytes, not current package bytes or Omarchy availability. Package owner/version comes from the image's installed pacman database; corresponding upstream source archives, Arch recipe and detached-signature evidence, and full third-party notices were not recovered or validated here. No source-to-binary correspondence, complete runtime/lazy-load closure, distribution conclusion, target acceptance, legal conclusion or human approval follows from this record. A changed runtime package or build image requires a new inventory and review.
