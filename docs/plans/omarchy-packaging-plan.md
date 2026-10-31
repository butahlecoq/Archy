# A21 Omarchy doctor and packaging implementation plan

**Status:** research and implementation plan only, 2026-10-29. No product code, installation, target execution, commits, or agent delegation performed. This document is the only file written for this task. Implementation requires a subsequent authorized slice; the checkboxes below are future work.

**Goal:** provide honest, nonmutating capability diagnostics and reproducible installation for Omarchy while preserving project data and user configuration.

**Architecture:** Rust CLI collects bounded observations and evaluates them against a reviewed compatibility manifest. Project-local mise selects development tools; Arch packages own installed code, while separate locked backend runtimes retain their interpreter boundaries. Desktop integration and target acceptance orchestration remain A22/A23 responsibilities.

**Inputs:** [AGENTS.md](../../AGENTS.md), [release specification](../specs/release-0.1.txt) sections 3, 5, 8–10, [backlog A21–A23](backlog.md), [existing Omarchy research](../research/omarchy-and-reuse.md), [backend research](backend-research.md), and [current license decision](../license-status.md). A21 depends on A01, A06, A11 and A16. Do not alter task02/task03 files or their implementations.

## Verified primary-source findings

These are documentation/source observations, not installed-machine acceptance. Links were independently read during this task; moving documentation must be rechecked when implementation pins are selected.

| Finding | Evidence and consequence |
| --- | --- |
| Stable baseline is v4.0.6; v4.5.0-rc2 is prerelease at inspection | [Official releases](https://github.com/omacom/omarchy/releases), [v4.0.6 release](https://github.com/omacom/omarchy/releases/tag/v4.0.6), and [official homepage download](https://omarchy.org/) agree. Keep 4.5 migration explicit; neither a newer version nor the default branch automatically passes compatibility. |
| Installed version is package-based, with a development-path exception | [Tagged omarchy-version source](https://raw.githubusercontent.com/omacom/omarchy/v4.0.6/bin/omarchy-version) queries `omarchy-dev` before `omarchy` through `pacman -Q`; nonstandard `OMARCHY_PATH` produces a development identity. Preserve package release suffixes and raw output; do not parse `dev` as a stable release. |
| Omarchy uses mise for many development environments | [Tagged development manual](https://raw.githubusercontent.com/omacom/omarchy/v4.0.6/manual/18-development-tools.md). Its global setup examples are not Archy's policy. It also documents lazy-installing command stubs: discovering a PATH entry does not authorize running it. |
| Package defaults and user configuration have different ownership | [Tagged dotfiles manual](https://raw.githubusercontent.com/omacom/omarchy/v4.0.6/manual/31-dotfiles.md) identifies `/usr/share/omarchy` as upstream-owned and `~/.config` as user-owned, including Hyprland `bindings.lua`. Hook directories include `post-update.d` and `theme-set.d`; menu extensions use `~/.config/omarchy/extensions/omarchy-menu.jsonc`. |
| Native shell extension contract exists in this tag | [Tagged shell README](https://raw.githubusercontent.com/omacom/omarchy/v4.0.6/shell/README.md): schemaVersion 1, declared QML entry points, manual plugin installation, shell IPC, and hot reload. `omarchy-shell shell ping` queries the running shell; the wrapper does not start it. |
| Plugin installation has explicit boundaries | [Tagged plugin manual](https://raw.githubusercontent.com/omacom/omarchy/v4.0.6/manual/32-shell-plugins.md): user plugins live under `~/.config/omarchy/plugins/`; `omarchy.` IDs are reserved; `omarchy plugin add` expects a repository-root manifest. Validation checks entry points and rejects symlinks. Plugins execute unsandboxed. A22 must ship a standalone/local plugin root, never advertise add against the monorepo root. |
| Quickshell supplies subprocess primitives | [Versioned Process API](https://quickshell.org/docs/v0.2.1/types/Quickshell.Io/Process/) exposes command arguments, environment and output channels. This verifies an API reference, **not** that Omarchy installs Quickshell 0.2.1. A22 must check the installed API before adopting it. |
| Project-local mise and explicit tasks are supported | [Configuration](https://mise.jdx.dev/configuration.html), [TOML tasks](https://mise.jdx.dev/tasks/toml-tasks.html), [exec](https://mise.jdx.dev/cli/exec.html). `.mise.toml` is supported; inherited/local overrides can affect resolution. Keep actual tool-version checks in validation. |
| Rust and Python environments need explicit separation | [mise Rust](https://mise.jdx.dev/lang/rust.html) documents `RUSTUP_TOOLCHAIN` precedence over rust-toolchain.toml. [mise Python](https://mise.jdx.dev/lang/python.html) supports existing venv activation and optional automatic creation. Avoid automatic creation during diagnostics. [uv locking/sync](https://docs.astral.sh/uv/concepts/projects/sync/) distinguishes locked validation from automatic synchronization. |
| Arch provides standard build and inspection mechanisms | [PKGBUILD](https://man.archlinux.org/man/PKGBUILD.5.en), [makepkg](https://man.archlinux.org/man/makepkg.8.en), [pacman](https://man.archlinux.org/man/pacman.8.en), [namcap](https://man.archlinux.org/man/namcap.1.en). Use staged package ownership, explicit dependencies and notices, package inventory, and lint review. |
| Isolation tools do not prove isolation policy | [Bubblewrap upstream](https://github.com/containers/bubblewrap) makes the invoking program responsible for policy. [systemd resource control](https://www.freedesktop.org/software/systemd/man/latest/systemd.resource-control.html) documents cgroup-backed controls. Executable presence and readable controller files cannot prove effective limits or descendant cancellation. |

### Unresolved and unavailable evidence

- No actual Omarchy machine was exercised. Installed Omarchy package revision, Hyprland, Quickshell, Qt, mise, systemd, bubblewrap, kernel, architecture, glibc, GPU and driver versions are **unknown**. Do not infer them from documentation headers or the OS release number.
- Existing files name Rust **1.99.1** and propose Blender **4.5.15 LTS**, CPython **3.12.16**, and IfcOpenShell **0.9.0.post1**. These are inherited repository pins/proposals, not independently reverified backend artifacts in this bounded task. A06/A11 must supply authoritative artifacts, hashes, complete locks, supported platform and smoke evidence before A21 marks them supported. No substitute versions are proposed here.
- After the network-enabled update, this session's retry of the unauthenticated releases API returned HTTP 403; `gh api` could not connect; `git ls-remote` could not resolve github.com. The web tool and tagged raw files remained accessible. The release page exposes short commit `237f1b4`, but a full independently resolved tag commit was not obtained. Resolve and record it before freezing source fixtures; no invented SHA.
- `https://omarchy.org/download` returned 404 on retry. The accessible homepage above supplies the verified download link. The ArchWiki Rust packaging page returned an access-denied page; the accessible official Arch manuals support the packaging mechanisms used here.
- An exact tested mise/uv version and runtime package dependency list remain implementation gates. Current documentation describes mechanisms, not a guarantee for an unknown installed version.

## Doctor contract and nonmutation boundary

Proposed public commands: `archy doctor --json` and `archy doctor` (same observations, readable text). No required project argument; no repair/install mode in A21. The default report evaluates Omarchy readiness even on WSL, explicitly identifying WSL as development evidence.

Proposed report schema `schema_version: "1"`:

- `host`: OS ID, kernel, architecture, `environment` (`omarchy`, `wsl`, `other-linux`, `unknown`), and evidence sources. Parse os-release as data; never source it.
- `platform`: raw package identities, parsed upstream version when valid, development override indicator, supported baseline and compatibility result.
- `checks[]`: stable `id`, `required_for[]`, `status`, `observed`, `expected`, `evidence[]`, `reason`, and `remedy`. Absent versions are JSON null, never guessed strings.
- `capabilities`: separate `core`, `blender`, `ifc`, `containment`, and `desktop` readiness. Desktop absence must not disable otherwise usable core commands.
- `target_gates[]`: stable gate ID, status and reason. `target_acceptance` stays `deferred` without actual acceptance evidence; doctor never signs off release acceptance.

Statuses: `available` means the specific observation succeeded, `missing` means absence was established, `unsupported` means an observed value conflicts with policy, `unavailable` means the observation could not be obtained, and `deferred` means an active test was deliberately not performed. Do not collapse permission errors, timeouts or malformed output into absence. Sort checks by ID. Remedies are explanatory strings, never executed commands.

Exit 0 means required **diagnostic prerequisites** matched policy, not target acceptance; exit 1 means a required prerequisite is missing, unsupported, unavailable or deferred; exit 2 means invocation/report-generation failure. Missing optional desktop checks do not change exit status. Include `ready: false` for each capability with unresolved required prerequisites; aggregate readiness must not hide them. Reuse existing structured Archy errors for internal failures without adding speculative global error codes.

| Check IDs | Allowed observations | What remains unproven |
| --- | --- | --- |
| `platform.os`, `platform.omarchy` | Bounded reads of OS/kernel information; trusted `/usr/bin/pacman -Q omarchy omarchy-dev` with per-package outcomes; record custom OMARCHY_PATH as untrusted/development metadata | Matching an OS label alone is not support; reject ambiguous package combinations pending review |
| `tools.*` | Inspect configured paths and package ownership; run only reviewed absolute, installed binaries with known read-only version arguments | PATH scripts, mise shims, rustup proxies and lazy installers must not execute automatically |
| `backend.blender`, `backend.ifc` | Read installation manifest, pinned runtime/worker metadata and lock identities; compare expected/observed versions; classify missing proof separately | Do not start Blender, import bpy/IfcOpenShell, create caches or venvs, or claim successful backend execution |
| `containment.user-manager` | Bounded read-only query to existing user systemd manager; return unavailable when bus/session cannot be reached | No enable/start/reset-failed, transient units or resource changes |
| `containment.cgroups`, `containment.bwrap` | Read mount/controller/current-cgroup metadata and installed bubblewrap identity | Effective delegation, namespace creation, memory/CPU/tasks enforcement and cancellation stay deferred to A16 target tests |
| `desktop.shell`, `desktop.plugin-api` | Installed tagged-feature inventory; optionally audited `omarchy-shell shell ping` and list-only IPC against an existing session | Never rescan, reload, enable, install or restart from doctor; live A22 behavior remains deferred |

Collection must use fixed argv without a shell, a minimal environment, bounded reads/output and timeouts (proposed 2 seconds per subprocess, 64 KiB per output stream, 15 seconds total). No network, sudo, package refresh, mise execution/trust, config loading as code, project creation, logs/cache writes or project migrations. Do not invoke arbitrary project-selected binaries. If provenance/read-only behavior cannot be established, report unavailable. No persistent writes initiated by Archy; ordinary OS accounting/access times are outside that promise. Separate nonmutating inventory from active acceptance tests rather than weakening this boundary.

## Project-local environments and package ownership

1. Add repository `.mise.toml` with exact Rust pin matching rust-toolchain.toml, and Python only after A11 confirms the interpreter pin. Use explicit local tasks, no global `mise use -g`, shell-profile edits, automatic environment hooks, or system Python pip installs. Existing global tool stores may be reused, but global defaults must remain unchanged.
2. Tasks: `fmt` runs `cargo fmt --all -- --check`; `lint` runs `cargo clippy --workspace --all-targets --locked -- -D warnings`; `test:core` runs `cargo test --workspace --locked`; `build` runs `cargo build -p archy-cli --release --locked`. Task versions must be checked against the pin so global/local overrides fail visibly.
3. An explicitly mutating `setup:ifc` creates only the designated development venv from the A11 lock using the reviewed interpreter and locked dependency installer. A separate `test:ifc` uses that existing venv; neither is a dependency of core tests or doctor. Blender always runs its bundled Python. No automatic runtime fallback to the rolling Arch Blender package.
4. Installed `/usr/bin/archy` must run without mise, Cargo or an activated development venv. Ship trusted workers under `/usr/lib/archy/workers/` when available, schemas/docs under `/usr/share/archy/`, and applicable notices under `/usr/share/licenses/archy/`. These paths are a packaging proposal, not existing files.
5. Backend runtime installation uses separately owned, versioned packages or explicit user provisioning, selected with A06/A11. Do not build a relocatable package by copying a development venv. If pinned archives are packaged under `/opt/archy/backends/`, record exact ownership, loader dependencies and hashes. A21 cannot close backend packaging until that choice and licenses are reviewed.
6. `packaging/arch/PKGBUILD` builds only from an identified release source and Cargo.lock, stages into `$pkgdir`, and declares actual build/runtime dependencies. Fetch/verify sources before offline build where practical; package install hooks must never download tools or resolve Python dependencies. A core-only package may report backends missing honestly; it is not full golden-workflow acceptance.
7. `archy.install`, if retained, is informational only. No home-directory traversal, user service activation, project migration, desktop edits or runtime downloads in install/upgrade/remove hooks. Omit executable hooks if no informational action is needed.
8. `pacman -R archy` removes only package-owned files. Preserve projects, immutable runs, approvals, reports, user configuration, integration backups and user-provisioned runtimes. Do not use uninstall as a purge mechanism. A22 owns explicit reversible desktop removal: compare recorded hashes, restore only safe unchanged edits, and preserve conflicts for human resolution. Never delete all of shell.json or an entire user plugin/config tree.
9. Keep MIT attribution for original core code consistent with the current repository license decision. Audit each shipped worker, binary, dependency and asset separately; include GPL/LGPL notices and applicable source/replacement obligations. The core license does not relicense Blender or IfcOpenShell.

## Actionable implementation slices

Paths below are proposed future edits, not changes made by this research. Coordinate shared CLI files after task02/task03 integration; do not overwrite their concurrent changes. Reuse existing serde/clap/schema facilities and standard-library probes; any added dependency needs a recorded rationale.

### 1. Typed report and compatibility policy

**Files:** create `crates/archy-cli/src/doctor.rs`, `schemas/doctor.schema.json`, `tests/doctor/`; later wire `crates/archy-cli/src/main.rs` and `crates/archy-cli/tests/doctor.rs`.

- [ ] Define `DoctorReport`, `CheckResult`, `CheckStatus`, `HostObservation`, `CompatibilityPolicy`; implement pure `evaluate(observations, policy) -> DoctorReport` before live probes. Separate injected observations from production collection; no public environment switch that can fabricate acceptance.
- [ ] Add fixtures for stable package with Arch release suffix, dev override, prerelease 4.5, unknown version, WSL, missing package, denied query, malformed output, timeout and conflicting identities. Verify null unknown versions and deterministic JSON/text output.
- [ ] Specify a checked-in compatibility manifest at `packaging/arch/compatibility.json` containing evidence status and approved identities. Do not fill unknown backend hashes with placeholders that can pass validation.
- [ ] Run `cargo test -p archy-cli --locked --test doctor` after creating that integration target. Expect fixtures to assert exact statuses and exit behavior, including required unknowns never yielding ready.

### 2. Read-only probes and CLI integration

**Files:** doctor module and integration test above; fixture inputs under `tests/doctor/`.

- [ ] Implement `collect() -> HostObservation` using the allowlisted probe policy and bounded subprocess runner. Wire text/JSON dispatch without changing existing init/validate behavior.
- [ ] Test hostile PATH installers, inherited Python/mise variables, executable paths containing spaces, partial package queries, huge output, hung probes and missing user bus. Assert that unsupported Blender blocks its capability and does not switch versions.
- [ ] Test filesystem nonmutation using an isolated fixture home/project/config with sentinel hashes, file inventories and process-call recording; deny network and writes in the test sandbox. Assert no repair, hook, worker, package-manager mutation or mise invocation occurs. Do not expose production fixture injection.
- [ ] Run the doctor test target plus existing CLI tests. Archive JSON/text snapshots and exact test output; a WSL report must identify development scope and deferred target gates.

### 3. Local tool tasks and environment separation

**Files:** `.mise.toml`, `tests/doctor/mise_contract.py`, `docs/omarchy/install.md`; A11 owns worker dependency locks.

- [ ] Implement the task table above and document explicit setup/trust as user actions, separate from doctor. Check `.mise.toml` Rust against rust-toolchain.toml and resolve inherited overrides before running tasks.
- [ ] Add contract checks for exact pins, no global writes, no auto-create venv directive, and no setup/worker task dependency from core tests. Verify core tests work with backends absent.
- [ ] In a disposable development environment, record selected Rust/Python executables, unchanged global defaults, locked IFC setup, and Blender/IFC interpreter separation. If tools or A11 locks are missing, record unavailable rather than installing substitutes.
- [ ] Run `python3 tests/doctor/mise_contract.py`, then the implemented mise tasks where prerequisites exist. This is WSL/development evidence only.

### 4. Arch package, notices and preservation tests

**Files:** `packaging/arch/{PKGBUILD,archy.install,compatibility.json,files.txt}`, `tests/packaging/test_manifest.py`, `docs/omarchy/install.md`.

- [ ] Describe every owned path in `files.txt`; implement staged install and the informational/no-op hook policy. Derive library dependencies from actual built artifacts; do not guess package names/versions.
- [ ] Add package contract tests rejecting home/config/project writes, network/install commands in hooks, escaping symlinks and unowned runtime deletion. Stage package files around project/approval/config/backup sentinels; simulate removal of precisely owned paths and verify all sentinels survive. Label this simulation, not pacman evidence.
- [ ] Run `python3 tests/packaging/test_manifest.py`. In an available disposable Arch build environment, run `makepkg --verifysource`, `makepkg --printsrcinfo`, `makepkg`, and `namcap` on the PKGBUILD and resulting artifact. Inspect `pacman -Qip` and `pacman -Qlp` for that exact artifact. Review lint warnings; do not suppress unexplained findings.
- [ ] Record artifact digest, source revision, dependency/notice inventory, build environment and logs. Actual package transaction and target readiness remain gated below.

### 5. Integration handoff

- [ ] Give A22 the report schema, fixed installed CLI path and local plugin-root packaging contract. Keep plugin enablement/hooks out of A21.
- [ ] Give A23 the gate IDs below and documented report/exit semantics. A23 owns the executable target harness; do not introduce a competing harness here.
- [ ] Obtain independent review at implementation time as required by AGENTS.md; this research used self-review only because the user prohibited agents. Record unresolved gates in the implementation handoff, without marking A21/A19/A20 complete.

## Deferred target gates

All gates below are **DEFERRED / NOT RUN** in this task. WSL fixtures, Arch build containers, documentation and mocks cannot close them. Later execution requires an actual Omarchy target and the relevant implemented components; no Omarchy installation on the current Windows host is authorized.

| ID / owner | Required later action and evidence |
| --- | --- |
| `OM-A21-01` / A21 | On clean Omarchy, record `pacman -Q omarchy omarchy-dev` outcomes, OS/kernel/architecture and installed tool identities. Install the exact reviewed package with `sudo pacman -U <artifact>` on the disposable target; archive inventory and artifact hash. |
| `OM-A21-02` / A21 | Run installed `/usr/bin/archy doctor --json` and text form without mise activation. Capture stdout/stderr/exit and independent nonmutation evidence; exercise missing/unsupported prerequisites. |
| `OM-A21-03` / A06/A11/A21 | Verify exact backend hashes/locks and run trusted worker smoke fixtures with isolated interpreters. Record actual Blender bundled Python and IFC Python/library identity, native loader compatibility and output verification. |
| `OM-A21-04` / A16 | Run the reviewed containment harness: namespaces, denied network/home access, effective memory/CPU/tasks limits, timeout and whole-descendant cancellation. A successful `bwrap --version` or user-manager query is insufficient. |
| `OM-A21-05` / A21/A22 | On a disposable target, create project/approval/config/backup sentinels, install, upgrade and `sudo pacman -R archy`; compare contents/permissions and package inventory. Exercise separate A22 uninstall including conflicting user edits. |
| `OM-A22-01` / A22 | Validate the standalone plugin root with `omarchy plugin validate <root>`; perform explicit local install, rescan, enable/disable, hot reload, theme and Lua-binding checks. Archive before/after hashes and visual evidence. None of these mutation commands belongs in doctor. |
| `OM-A23-01` / A23/A19/A20 | Execute clean golden journey, CPU renders, independent IFC reader and visual review with immutable manifests. GPU is a separate optional tested profile, not an inferred capability. |
| `OM-MIG-45` / A21/A22/A23 | Resolve a specific 4.5 tag and installed package set, diff APIs/config, repeat compatibility/desktop/worker gates in a separate profile. No automatic widening of the 4.0.6 baseline. |

Gate records must include machine identity, timestamp, exact versions/source revision, command, exit status, output/artifact references and result. Missing infrastructure stays unavailable/deferred; an executed failing test stays failed. Closing documentation work is not closing target acceptance.

## Review focus and research handoff

The most important implementation review cases are lazy-installing PATH entries (slice 2), ambiguous/development package versions (slice 1), mise overrides or mixed interpreters (slice 3), accidental uninstall of user-owned files (slice 4), and reporting passive containment observations as proven enforcement (slice 2 plus OM-A21-04).

Completed here: read repository guidance/specification/backlog and existing research; independently checked the linked primary documentation; defined A21 contracts, file boundaries, validation steps and deferred gates. Remaining: all implementation/tests/package builds, complete immutable upstream/artifact pins, license inventory, independent implementation review and actual Omarchy execution. No product test or target gate is claimed passing.
