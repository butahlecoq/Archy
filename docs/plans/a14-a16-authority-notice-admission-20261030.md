# A14/A16 authority notice admission

30 October 2026. Base: `7dde28894f5fba99223be7e7e801d05d05bb83b0`.

## Scope and distribution decision

The current Arch package still stages only `archy` and `archy-service-gate`. `packaging/arch/files.txt` has no authority binary, systemd unit, or D-Bus policy. The new authority notice command produces review evidence for a future distribution decision; it does not authorize that distribution or install/enable a service. Any later package change must pair the actual authority binaries with this admitted notice graph and have target-specific service/policy and human review.

## Source and text evidence

`cargo +1.99.1 metadata --locked --offline --format-version 1 --filter-platform x86_64-unknown-linux-gnu` identifies 158 registry crates in the authority normal graph. The CLI/gate graph has 106. The 52 additional crate identities and their 96 actual license/notice files are recorded in `packaging/arch/authority-notice-inputs.json` with the exact Cargo.lock archive SHA-256, declared Cargo license, filename, and file SHA-256. This includes `tracing-core 0.1.36/src/spin/LICENSE` (SHA-256 `58545fed1565e42d687aecec6897d35c6d37ccb71479a137c0deb2203e125c79`) inside the archive; scanning only crate roots would miss it. The pin also records all eight local packages in the authority graph. The files were read from each locally cached `.crate` archive after checking the whole archive against Cargo.lock. The existing seven external primary-source texts for CLI/gate crates are carried into the authority output with their byte, Git blob, and crate VCS commit checks. Rust 1.99.1 standard-library texts remain sourced from the existing pinned inputs.

The patched local zbus 5.19.0 is a separate 159th entry. Its original crates.io archive is included at `packaging/arch/source-archives/zbus-5.19.0.crate` because Cargo resolves the local patch without caching the upstream archive on a fresh runner. The archive SHA-256 is `5db4be7c075cb421e4b7ee645541604239bd243ba7c357511f4ff3a74b555907`; its unchanged MIT `LICENSE` SHA-256 is `260e4ff3fc493695880c90774283eb1e3d018f898fabe655bbd52dd2cbd2fcad`. The source comparison found exactly nine changed upstream files and the added `ARCHY-PATCH.md`; all ten paths and candidate SHA-256 values are pinned in the input JSON. The admission command verifies the included archive against its pin, every other source file against that archive, and rejects missing or extra files. Cargo's `.cargo-ok` extraction marker is ignored as generated cache state.

The pin JSON SHA-256 at this review is `a7528813d1cfd92027fd84008e9c98c63f65094fc0e7dbab4fd819681a541a29`. The resulting full notice bundle SHA-256 is `3da910a7f00d133ed0af906f1acb10c2ddc97356658a3194d467193eddcf64ca`; its JSON inventory SHA-256 is `90624dffd4a8d1d9c726798ea02bd35119f0970dc5325658038d2d7c85514f2a`. Both output hashes are enforced by the pin file. These outputs are generated artifacts, not installed package files.

## Reproduction

On an ext4 temporary directory after fetching the locked crates with Rust 1.99.1:

```sh
export TMPDIR=/home/butahlecoq/work/archy CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true
mkdir -p "$TMPDIR/a14-a16-authority-notices"
python3 -B packaging/arch/authority_notices.py --source . \
  --cache "$HOME/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f" \
  --output-dir "$TMPDIR/a14-a16-authority-notices"
python3 -B -m unittest discover -s tests/packaging -q
```

The command reruns against identical outputs. It refuses a stale output, changed/missing archive, mismatched license file set or bytes, changed zbus patch, and a new registry dependency without a new reviewed pin. CI repeats this admission after the locked workspace test has populated the registry cache. A packaging test admits with a separate registry cache that deliberately has no upstream zbus archive.

This is source and notice evidence, not legal clearance. Distribution remains gated on independent license review, exact authority package contents, target Omarchy/system-bus validation, security review, and explicit human approval of the proposed service and D-Bus policy.
