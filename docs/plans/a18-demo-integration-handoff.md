# A18 partial demo integration handoff — 29 October 2026

## Scope and decision

Prepared in the new detached worktree `/home/butahlecoq/work/archy/.worktrees/a18-integration-20261029` at `origin/feat/release-0.1` = `ae6e6a6923fad5a4aae343cea67fcf40f62fd43e`. This patch is uncommitted, unpushed, and awaiting review. The original A18 and source worktrees were not edited. No package was installed or child agent used.

Plan: verify the two independent reviews and their source freeze; carry the reviewed partial driver and tests; correct the round-two CLI documentation finding; run the real binary and six tests from this tree; expose those tests in CI. No backend or human-decision command is included. The standard-library demo has no added runtime or test dependency.

The two independent reports are [round one](a18-demo-independent-review.md) and [round two](a18-demo-independent-review-round2.md). Round one found a false export-success path and an uncontrolled malformed-receipt traceback. Round two verified their repair and identified only the low-severity `docs/cli.md` mismatch. The driver and tests here are byte-identical to the round-two reviewed files. The CLI documentation now states that the driver stops after comparison even if `render` is advertised. `--export-variant` records intent and does not export. README wording was aligned to the same boundary and current base. Both review reports are preserved verbatim as review evidence; their host paths and old transcript references describe the original review environment.

The original 825-line transcripts were intentionally not copied into product docs. The original handoff and transcripts remain in `.worktrees/a18-demo-20261029/docs/plans/` in this workspace. The 32-file inventory below covers the new run and is portable relative to its project directory.

## Reviewed snapshot and integration hashes

Round-two reviewed source hashes, independently rechecked before copying and identical here:

```text
2cc20930027a10e6a8fd33b13764caa052a55fb8b8c421cba27767d3603b9fb3  scripts/demo.py
b7aa540e205df1b2b268c1bd890ac7b688c93c7b33e561ca62cd1096ba5eb6b4  tests/demo/test_demo.py
e21e60d4a2899e9c86b3d10e03b0d7a6ea7b49189bdaca8c7d315c2c87fa7c17  tests/demo/review_shim.py
2961bf7a7b95ab63f9d02c7c110a96c6eaad159c407b211194e2663d1032286d  CONTRIBUTING.md
```

The original README and CLI doc hashes were `a8042b3fd651cadd76f1cf0268471768901881bb222190b960eb661ef6371d9c` and `89f3e4849e5a5ca3fee394c3c296e92862d091eac792b183c2ae6c4592d94386`. Their integration hashes changed for the documented current boundary. The new reviewable file hashes are:

```text
09e2216e8560dc2adc6eaa2720f01f83238e9890e482d64abdcb59bde7bb3d61  README.md
417ef8ab15dac6e3bd92d76a5885f0e1b0f708019c982824bd06a4d3521bb0af  docs/cli.md
a0088e3a90852d80b1a2c72f4fdb5e0640f242077b58f81a9edd941da3fd4859  .github/workflows/ci.yml
557f70f2531ffa0b12b8b1ac76383c565e4e5ae49d0daf2a68c01c5cf00b6f63  docs/plans/a18-demo-independent-review.md
8381bbf5a590dd8c0e70f3ed8cdb97562dc559cc1fdc7321a2d3905d63f4d2a4  docs/plans/a18-demo-independent-review-round2.md
d22c5a1e34072fb495d95944ee599bb5bb7f1fa39454876219bc9c8eb0c7e85b  docs/plans/a18-demo-integration-artifacts.sha256
```

The reviewed transcript and inventory hashes also matched the original handoff before integration: `1f6db72d...` and `a98c4fc7...` for the first run, `a4c739be...` and `e64b0e20...` for the repair run. No A18 source path existed in the `ae6e6a6` tree; only `README.md` and `.github/workflows/ci.yml` are tracked modifications in this patch.

## New real-CLI evidence

Built with `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a18-integration-20261029 cargo build --locked -p archy-cli` (exit 0). The executable hash was `7934b073d221bd97734e0e346fb91b5b0505ae3663d952ea626ec651e453068c` and `--help` listed `desktop`, `doctor`, `init`, `validate`, `generate`, `compare`, and `evaluate`, without render/export/approval. `ARCHY_BIN=/home/butahlecoq/work/archy/.cargo-target-a18-integration-20261029/debug/archy python3 -m unittest discover -s tests/demo -v` exited 0: **6 tests passed**. These include existing-directory and symlink collisions, malformed evaluation receipts, an advertised backend, and an empty-success backend shim. Each shim delegates real core commands to this newly built CLI.

Ran `python3 scripts/demo.py --archy /home/butahlecoq/work/archy/.cargo-target-a18-integration-20261029/debug/archy --project-dir /var/tmp/archy-a18-integration-WNyQ3q/demo`. Help, init, validate, generate, evaluate, and both compare commands exited 0. The demo exited **2** at `UNAVAILABLE: archy render is absent`. Generation run `15bb8d96-3adb-4e52-8646-e5b747a2aeeb` and evaluation run `ce386b07-7a14-424d-be18-b6e98fbb849e` were retained with `comparison.json` and `comparison.md`. The project has 32 regular files and zero GLB, PNG, or IFC files. The log is retained beside the project at `/var/tmp/archy-a18-integration-WNyQ3q/demo.log`; SHA-256 `b53c0bf219f22a31d979cb5c04d84c80efa8b69f69c81f2d571c1281b23a83ba`. `sha256sum -c docs/plans/a18-demo-integration-artifacts.sha256` from the project passed all 32 entries.

The CI workflow now builds `archy-cli` after Rust tests and invokes the six Python standard-library tests using `$PWD/target/debug/archy`, which is the binary produced by that same job. It uses the existing Ubuntu runner, pinned Rust toolchain and locked Cargo graph. It needs neither Blender/IFC packages nor a new Python package. This workflow change has been inspected locally; no hosted CI run is claimed.

## Review and open gates

Review the narrow CI wiring and revised README/CLI wording against the current driver, then inspect the two preserved review reports and hashes. The CLI backend receipt, artifact layout, supervised render/export integration, explicit local human decision, Omarchy installation and containment, visual review, independent IFC inspection, and release acceptance remain open. This partial demo is evidence for the local core workflow only and does not approve A18 or release 0.1.
