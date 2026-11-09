# Omarchy development tasks and installation boundary

This page covers the A21 slice 3 development tasks. It does not install Archy on Omarchy. Package installation and target acceptance remain separate A21 work; the current development host is not Omarchy evidence.

## Local Rust tasks

The repository pins Rust **1.99.1** in both `.mise.toml` and `rust-toolchain.toml`. The project config sets `auto_install = false`, but an inherited `MISE_AUTO_INSTALL=true` takes precedence over that file. Rustup has a separate automatic-install switch, which can be inherited as `RUSTUP_AUTO_INSTALL=1`. Set **both** `MISE_AUTO_INSTALL=false` and `RUSTUP_AUTO_INSTALL=0` on the command line before mise starts, as shown below. The project also exports `RUSTUP_AUTO_INSTALL=0` to all mise tasks, and the version-check script sets it before invoking either Rust proxy. These controls stop automatic toolchain installation; an absent selected toolchain must fail instead of being provisioned. Explicit install commands remain a separate user action. A deliberately conflicting invocation is outside this supported boundary.

Each core task runs `scripts/check-rust-toolchain.sh` as its first own step. The script checks the effective `rustc` and `cargo` version strings and stops unless both report 1.99.1. `mise run --skip-deps` and `MISE_TASK_SKIP_DEPENDS` skip task dependencies but do not remove this first step. Matching version text does not establish executable provenance; inspect selected paths separately when recording release evidence.

From the repository root, review and explicitly trust `.mise.toml` if mise asks for trust. If Rust 1.99.1 is absent, a user may explicitly install it with `mise install rust@1.99.1`; this is a deliberate development setup step, not part of a task, doctor, or package hook. No project task changes global mise defaults or shell profiles. The project does not select a Python version yet.

| Explicit command | Action |
| --- | --- |
| `MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0 mise run fmt` | `cargo fmt --all -- --check` |
| `MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0 mise run lint` | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| `MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0 mise run test:core` | `cargo test --workspace --locked` |
| `MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0 mise run build` | `cargo build -p archy-cli --release --locked` |

`python3 tests/doctor/mise_contract.py` checks the local config with Python 3.11+ standard-library `tomllib` and exercises a disposable rustup home when rustup is available. It does not install tools. `mise config ls` and `mise ls --current` can be used to inspect active configuration and selected tools before running a task. An inherited `MISE_RUST_VERSION`, `RUSTUP_TOOLCHAIN`, or another path/toolchain override must not be taken as proof of the project pin: the task checks the versions actually reported by its commands. The [rustup environment reference](https://rust-lang.github.io/rustup/environment-variables.html) defines `RUSTUP_AUTO_INSTALL=0` as disabling automatic installation.

## Backend interpreter separation

Core formatting, linting, tests, and builds do not set up or call workers. A future IFC development environment needs A11's reviewed Python interpreter, platform artifacts, and complete lock before an explicit `setup:ifc` or `test:ifc` task can exist. **Both tasks are unavailable now.** No placeholder lock, executable setup command, automatic virtual environment, system Python package install, or fallback interpreter is supplied. The IFC worker must run with its own locked interpreter and existing environment after that review.

Blender uses the Python bundled with the reviewed Blender binary. It must not be run through the IFC environment, project mise Python, or rolling system Blender as an automatic fallback. The A06 Blender artifact and A11 IFC runtime need separate target smoke evidence before either backend is reported ready.

`archy doctor` is intended to inventory installed capabilities without running mise, trusting this config, creating environments, or installing dependencies. The installed `/usr/bin/archy` must work without mise, Cargo, or an activated development environment. Backend installation and tests are explicit user or release-gate actions, distinct from doctor and these core tasks.

See [the A21 research plan](../plans/omarchy-packaging-plan.md) for package ownership and deferred Omarchy acceptance gates, and [the slice handoff](../plans/mise-handoff.md) for current validation evidence.
