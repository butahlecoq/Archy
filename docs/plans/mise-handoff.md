# A21 slice 3: project-local mise handoff

**Scope:** `.mise.toml`, `scripts/check-rust-toolchain.sh`, `tests/doctor/mise_contract.py`, `docs/omarchy/install.md`, and this handoff. The script was added under the explicit round 1 scope grant. Implementation follows [the A21 research plan](omarchy-packaging-plan.md) and `AGENTS.md`. No agent delegation, commit, push, global config edit, or target execution occurred in this slice.

**Live validation update:** [mise-live-validation.md](mise-live-validation.md) records bounded v2026.10.23 execution in isolated mise directories. During an earlier override test, rustup briefly provisioned a new numeric toolchain despite mise auto-install being off; the process was stopped and the newly created toolchain and files were removed. The follow-up correction and disposable rustup evidence are appended there without erasing the incident.

## Decisions and boundaries

- Rust is exactly 1.99.1, matching `rust-toolchain.toml`. Project-local `auto_install = false` is a default, which `MISE_AUTO_INSTALL=true` can override. The supported invocation explicitly prefixes each command with `MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0` before mise starts. Project `[env]` carries the rustup value through every task step; the guard also exports it before probing Rust. A deliberate conflicting invocation is outside that guarantee.
- Each core task runs the version script as its first own step, then the Cargo command. The script checks the **effective** `rustc` and `cargo` version strings so inherited toolchain overrides fail visibly. `--skip-deps` cannot remove this own step. Matching version strings alone do not prove executable provenance.
- `fmt`, `lint`, `test:core`, and `build` use the exact locked Cargo commands in the research plan. Core tasks have no worker dependency or environment hook.
- Python is deliberately absent from `.mise.toml`. A11 has not supplied the reviewed lock/interpreter, so `setup:ifc` and `test:ifc` are not defined. IFC will use a separate explicit locked environment; Blender will use its bundled Python. Neither interpreter is chosen by doctor or a core task.
- Trusting the project config, installing a missing pinned Rust toolchain, creating a future IFC environment, running backend tests, and package installation are separate explicit user or release-gate actions. Doctor must remain read-only. The packaged CLI must not require mise.

## Validation on this development host

| Check | Result |
| --- | --- |
| Initial `python3 tests/doctor/mise_contract.py` | Passed: 6 tests before round 1 review. |
| Round 1 `python3 tests/doctor/mise_contract.py` | Passed: 8 tests. Includes task-owned guard structure, fake effective-version mismatches, and inherited `MISE_AUTO_INSTALL=true` overridden at stub mise process startup by each documented invocation. See [round 1 evidence](mise-round1-evidence.md). |
| Initial `MISE_AUTO_INSTALL=false mise run fmt`, `lint`, `test:core`, `build` | Unavailable before the coordinator supplied mise. Superseded by the bounded live results below. |
| Live mise v2026.10.23 tasks | `fmt` exit 1 (concurrent formatting diffs); `lint` and `test:core` exit 101 (five concurrent doctor compile errors); later `build` exit 0. See [exact evidence](mise-live-validation.md). |
| Live setting and override checks | Project `auto_install=false` was read by real mise; `MISE_AUTO_INSTALL=true` overrode it. `--skip-deps` did not skip the task-owned guard. A selected stable override failed in that guard before Cargo. Missing mise store entry still allowed rustup fallback; a numeric override triggered rustup provisioning and was cleaned up. |
| Post-fix contract and disposable live checks | 9 contract tests passed. In an empty isolated rustup home, real mise ran the guard for all four `--skip-deps` tasks and each exited 1 for missing 1.99.1 before Cargo. No runtime files or downloads appeared. Real `mise env` exported `RUSTUP_AUTO_INSTALL=0`, including when an outer shell inherited `1` but the supported command prefixed `0`. See the [follow-up section](mise-live-validation.md#follow-up-correction-and-isolated-rereview-2026-10-29-1403-utc). |
| IFC setup/test | Unavailable pending A11 reviewed lock and interpreter. No command was run. |
| Blender/IFC interpreter identities and unchanged global mise defaults | Backend identities unavailable because reviewed runtimes were not exercised. Mise config/data/cache/state were isolated; no global mise settings command was run, but no before/after global-default snapshot was taken. |
| Omarchy target acceptance | Deferred; this host provides development evidence only. |

## Independent review focus

Round 1 review found that an environment setting can override the local auto-install setting and that `--skip-deps` can skip a dependency guard. Live mise confirms the setting precedence and task-owned guard ordering. The follow-up adds rustup's separate no-install switch and proves missing-tool refusal for all four task paths in a disposable home. For rereview, assess the supported startup invocation, project environment persistence, and direct-script guard, while preserving the earlier rustup incident as historical evidence. Matching version strings do not prove binary provenance. Review A11's interpreter and lock before adding IFC tasks. The package and doctor agents own their separate files; this slice does not claim their results.
