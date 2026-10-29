# A21 slice 3 mise round 2 independent review

**Scope:** Review the `RUSTUP_AUTO_INSTALL=0` correction in `.mise.toml`,
`scripts/check-rust-toolchain.sh`, `tests/doctor/mise_contract.py`, and
`docs/omarchy/install.md` against `AGENTS.md`, the A21 packaging plan slice 3,
`mise-handoff.md`, `mise-live-validation.md`, and `mise-review-round1.md`.
Read-only review plus isolated tests; this document is the only repository
write. No product edit, commit, push, agent, runtime download, or Omarchy
execution.

## Finding and disposition

**No blocking finding in the supported no-install invocation.** The earlier
rustup auto-provisioning incident in `mise-live-validation.md` was real and is
preserved there; mise's own `auto_install=false` did not control rustup. The
correction covers the three relevant phases:

1. `docs/omarchy/install.md` prefixes each supported task invocation with
   `MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0`, so inherited `true`/`1`
   values are replaced before mise startup. The real pinned mise process
   reported effective `auto_install=false` under that prefix.
2. Project `[env] RUSTUP_AUTO_INSTALL="0"` is emitted by real `mise env` and
   applies to task steps. Each task's own first `run` array entry invokes the
   guard; no `depends` edge can be skipped instead. The [mise task reference](https://mise.jdx.dev/tasks/task-configuration)
   says array entries run in order and stop on failure; the
   [run reference](https://mise.jdx.dev/tasks/running-tasks.html) defines
   `--skip-deps` as omitting dependency tasks. The real refusal runs below
   confirm the guard remains the first step for all four tasks.
3. `scripts/check-rust-toolchain.sh` exports `RUSTUP_AUTO_INSTALL=0` before
   either `rustc --version` or `cargo --version`. The direct-script contract
   test starts with inherited `RUSTUP_AUTO_INSTALL=1` and missing isolated
   toolchains; both proxy paths refuse without creating runtime files. The
   [rustup environment reference](https://rust-lang.github.io/rustup/environment-variables.html)
   documents `0` as disabling automatic installation.

These controls apply to the documented command form. Deliberately conflicting
CLI flags or later task-specific environment overrides are outside the
reviewed form. The guard checks reported versions, not executable hashes or
provenance; the handoff and install page state that limit. No Python, IFC,
Blender, global mise setting, or worker action is added to the four core tasks.

## Exact review evidence

- `python3 tests/doctor/mise_contract.py -v`: exit **0**, **9 tests passed**, 0
  failures/skips. Tests include exact TOML task shape, no-install prefixes,
  inherited startup overrides through a stub mise process, both effective
  version mismatches, and isolated rustup refusal for both probes.
- `/home/butahlecoq/work/archy/.tools/mise-2026.10.23/mise --version`:
  `2026.10.23 linux-x64 (2026-10-29)`, exit **0**. Local binary SHA-256
  `a6ac620e73fb58cefbdfef8fb154856fbd8ae08d75ae88103e4b81daef3a836e`
  matches the handoff's recorded value. This review did not independently
  retrieve the upstream release digest.
- Real mise was invoked by Python `subprocess.run` with an explicit environment
  (equivalent to `env -i`), `cwd=/home/butahlecoq/work/archy/repository`,
  captured output and a 15-second timeout. The retained fixture is
  `/tmp/archy-mise-review2.r_fowqja`. It had separate `home`, mise
  `config/data/cache/state/tmp/system`, empty `cargo`, and `rustup` containing
  only a pre-seeded minimal `settings.toml`. The environment set
  `MISE_AUTO_INSTALL=false`, `RUSTUP_AUTO_INSTALL=0`, `MISE_OFFLINE=true`,
  `MISE_SELF_UPDATE_AUTO=false`, `CARGO_NET_OFFLINE=true`, isolated
  `RUSTUP_HOME`/`CARGO_HOME`, and rustup distribution/update URLs to
  nonexistent local `file://` paths. `PATH` was
  `/usr/bin:/bin:/home/butahlecoq/.cargo/bin`; mise was called by its absolute
  verified path. No shared rustup home or global mise directory was selected.
- `mise trust .mise.toml`: exit **0**, trust written only to isolated mise
  state. `mise settings get auto_install`: exit **0**, stdout `false`.
  `mise env`: exit **0**, included `export RUSTUP_AUTO_INSTALL=0`.
- `mise run --skip-deps fmt`, `lint`, `test:core`, and `build`: **each exit 1**.
  Each stderr began with `[task] $ sh scripts/check-rust-toolchain.sh`, then
  `toolchain '1.99.1-x86_64-unknown-linux-gnu' is not installed`. No Cargo
  command started. The isolated rustup/Cargo file inventory was exactly
  `rustup/settings.toml` before and after; mise `data/installs` was empty.
  The rustup help line suggested a manual install; it was not executed.
- In the same fixture, an outer environment set `MISE_AUTO_INSTALL=true` and
  `RUSTUP_AUTO_INSTALL=1`. The actual shell command
  `MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0 /home/butahlecoq/work/archy/.tools/mise-2026.10.23/mise settings get auto_install`
  exited **0** with `false`; the corresponding `mise env` command exited
  **0** and exported `RUSTUP_AUTO_INSTALL=0`. The corresponding
  `mise run --skip-deps fmt` exited **1** at the missing-tool guard. Runtime
  inventory remained only `rustup/settings.toml`, with no mise install entry.

## Acceptance limits

The exit-1 task runs prove **missing-runtime refusal** for this isolated WSL
scenario. They are not successful `fmt`, `lint`, `test:core`, or `build` runs.
The earlier live results in `mise-live-validation.md` remain historical and
were not rerun here. The isolated test did not execute a successful second
Cargo task step; project `[env]`, real `mise env`, and the task contract support
the all-step environment claim, while direct observation of a successful
second step remains a separate validation. No before/after snapshot of global
defaults was taken; the selected environment and path inventory demonstrate
isolation of this review run. Omarchy target acceptance, executable provenance,
backend interpreter separation, and package installation remain external
gates. This review approves only the bounded no-auto-install correction, not
A21 or core task success as a whole.
