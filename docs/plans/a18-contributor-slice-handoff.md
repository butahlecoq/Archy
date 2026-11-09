# A18 contributor handoff slice — 30 October 2026

## Scope and state

Base is published `1e2313fe0bf20f69f246836334adb761276d2390`. The isolated, detached worktree is `/home/butahlecoq/work/archy/.worktrees/a18-contributor-handoff-20261030-1e2313`. The original checkout and other worktrees were not edited. This patch is uncommitted and unpushed, frozen for independent review; review status is **pending**. Issue #18 remains open.

Changed: `README.md` and `CONTRIBUTING.md` link a new source-development quickstart; `docs/contributor-quickstart.md` gives offline commands, the checked-in version matrix, real artifact inspection, and the source-versus-Omarchy boundary; `docs/templates/session-handoff.md` records evidence and review state; `docs/plans/a18-contributor-slice-plan.md` states scope and risks; this handoff and `a18-contributor-demo-artifacts.sha256` preserve the run evidence. No runtime, package, desktop, IFC, authority, renderer, or existing demo-driver code changed. No dependency was added.

## Environment and commands

Host: Ubuntu/WSL2, Linux `6.6.87.2-microsoft-standard-WSL2`, x86_64. Effective `rustc 1.99.1`, `cargo 1.99.1`, Python `3.14.4`. No Omarchy machine, target package transaction, or backend smoke test was used. Hardware timings and throughput were not measured.

From this worktree:

1. `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a18-contributor-20261030-1e2313 cargo build --offline --locked -p archy-cli` — exit 0; `Finished dev profile` in 2m 54s. This is compile wall time, not a benchmark.
2. `ARCHY_BIN=/home/butahlecoq/work/archy/.cargo-target-a18-contributor-20261030-1e2313/debug/archy python3 -m unittest discover -s tests/demo -v` — exit 0, six tests, 16.486s. The tests exercise real CLI core commands through the existing demo harness.
3. First explicit-path `python3 scripts/demo.py ... --project-dir /var/tmp/archy-a18-contributor-20261030-1e2313/demo` — exit 2 before init: `project parent directory does not exist`. This was a setup mistake and created no project.
4. `mkdir -p /var/tmp/archy-a18-contributor-20261030-1e2313`, then the same demo command — exit 2 at the expected `UNAVAILABLE: archy render is absent`. Help, init, validate, generate, evaluate, and both compare calls exited 0. Transcript: `/var/tmp/archy-a18-contributor-20261030-1e2313.log`, 29,819 bytes, SHA-256 `c111f3f7c966de4bea1329802641bdb69f2e73d44933e19a5bc9c291a02b34dd`.
5. `sha256sum -c docs/plans/a18-contributor-demo-artifacts.sha256` from the demo project — exit 0 for all 32 regular files. `git diff --check` — exit 0. Local links in the four new/changed entry docs were checked with a Python standard-library path scan — exit 0. Version matrix values were compared directly to `packaging/arch/compatibility.json` — exit 0.

Demo project: `/var/tmp/archy-a18-contributor-20261030-1e2313/demo`. Generation run `04d0775c-a0d8-49db-98e9-d10862e450e6`; evaluation run `2aa6b394-50f7-49a8-beab-c8ee24655ad8`. `comparison.json` contains three candidates. `project.yaml` is 486 bytes, SHA-256 `c534bf5f9c4c459b4e06171b0d3969e07d7f4e83a6791010fc83b2abe78a611a`; `comparison.json` is 19,324 bytes, SHA-256 `cffa8cd7ce960767e68e659afe0eb8ed22f3709264b2ad67ed657650022807a3`; `comparison.md` is 6,737 bytes, SHA-256 `c287f8ba137c46a0227f3b17c259e12c33673b0976b99ad01d6b8d236d954cde`. No GLB, PNG, or IFC file exists in the project. The retained run files are listed with project-relative hashes in the artifact manifest. The `/var/tmp` project and transcript are local review evidence, not a published release asset.

## Limits and next action

This verifies the source CLI path on WSL only. The documented Omarchy 4.0.6 baseline is not an installed or accepted Archy target. Blender 4.5.15 and Python 3.12.16/IfcOpenShell 0.9.0.post1 remain unverified proposals in the checked-in compatibility manifest. Supervised rendering and IFC CLI publication, explicit human selection and approval, clean Omarchy install, backend and visual inspection, containment, recovery, and release acceptance remain open. An agent cannot approve a variant.

Next: an independent reviewer should inspect this bounded docs patch, rerun the links and version-matrix check, compare the quickstart against the real CLI/demo transcript and package boundary, and record findings before integration. No review is claimed here.
