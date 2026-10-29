# Contributor source quickstart

This walkthrough exercises the current source CLI on a disposable project. It is a development check, including on Ubuntu under WSL. It does not install Archy on Omarchy or validate a package, desktop plugin, Blender, or IFC worker. See the [installation boundary](omarchy/install.md) and [package status](omarchy/package.md) before describing target support.

## Record the environment first

From the repository root, record `git rev-parse HEAD`, `git status --short`, `uname -a`, `rustc --version`, `cargo --version`, and `python3 --version`. Record whether the shell is Omarchy, another Arch system, native Linux, or WSL. The source toolchain is pinned to Rust **1.99.1** by `rust-toolchain.toml` and `.mise.toml`; the demo needs Python 3's standard library. `packaging/arch/compatibility.json` is the source for the platform/backend proposals:

| Component | Version or status at this source revision | What it proves |
| --- | --- | --- |
| Rust source toolchain | 1.99.1, with rustfmt and clippy | Reproducible development selection; check the effective executable versions. |
| Omarchy | 4.0.6 documented baseline | A policy target, not a completed install or target-machine acceptance. |
| Blender | 4.5.15 proposed, `runtime_supported: false` | No supported worker runtime established by this matrix. |
| IFC Python / IfcOpenShell | 3.12.16 / 0.9.0.post1 proposed, `runtime_supported: false` | No complete IFC lock or supported worker runtime established. |

The development tasks and automatic-install controls are in [Omarchy development tasks](omarchy/install.md). An installed `/usr/bin/archy` must work without Cargo, mise, or an activated virtual environment; this walkthrough builds a source binary and does not test that installed path. `archy doctor --json` is a passive prerequisite report, not proof of backend smoke tests or release readiness. Its nonzero exit on WSL is expected when the target platform is unavailable.

## Run the supported source demo

Choose a **new, empty** Cargo target on local storage. These commands avoid network resolution and automatic Rust toolchain installation; missing cached dependencies or toolchain components fail explicitly. Do not run them inside an existing project directory.

```sh
export CARGO_TARGET_DIR=/absolute/path/to/new-archy-cargo-target
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo build --offline --locked -p archy-cli
ARCHY_BIN="$CARGO_TARGET_DIR/debug/archy" python3 -m unittest discover -s tests/demo -v
python3 scripts/demo.py --archy "$CARGO_TARGET_DIR/debug/archy"
```

The last command prints a fresh `Project:` path. It calls real `init`, `validate`, seeded `generate`, `evaluate --all`, and JSON/Markdown `compare`, then exits **2** with `UNAVAILABLE: archy render is absent`. That stop is the current product boundary, not a successful full release demo. The project retains `project.yaml`, `runs/<run-id>/` history, `comparison.json`, and `comparison.md`. Record the path and inspect the files. Do not expect GLB, PNG, or IFC output or an approval event. To use a chosen parent, pass `--project-dir /absolute/path/to/absent-project-directory`; the parent must exist and the project path must not.

For a quick read of the retained result, replace `PROJECT` with the printed path:

```sh
find "$PROJECT/runs" -maxdepth 3 -type f -print | sort
python3 -m json.tool "$PROJECT/comparison.json" >/dev/null
sha256sum "$PROJECT/project.yaml" "$PROJECT/comparison.json" "$PROJECT/comparison.md"
```

Use the [CLI boundary](cli.md) for command details and the [MCP facade](mcp/README.md) for its separate typed interface. The comparison covers geometric area, repeated-footprint approximate GFA, height, and declared setbacks. The 900 m² example GFA target is soft. Daylight, cost, structure, access, and legal compliance are uncomputed. Constraint authority is a user declaration until separately verified. A selection or agent proposal is not human approval; any eventual preapproval IFC must be marked draft under the [approval policy](adr/0005-approval-policy.md).

## Leave a reviewable handoff

Copy [the session template](templates/session-handoff.md) into your own review note. Include the exact source revision, environment, commands and exit codes, real project path, artifact hashes, scope, blockers, and next action. Preserve failed variants and accepted history. Request independent review of the changed diff and evidence before treating the slice as accepted. WSL and CI checks cannot close clean Omarchy installation, backend, visual, containment, or independent IFC acceptance gates.

If using the golden example or an analytic fixture as benchmark evidence, also record hardware, elapsed time for the exact command, artifact byte sizes, and the input/seed. Do not infer a throughput target from one development machine.
