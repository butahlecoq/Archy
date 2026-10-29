# Contributing

Read [AGENTS.md](AGENTS.md), the [release specification](docs/specs/release-0.1.txt), and the [backlog](docs/plans/backlog.md) before changing a workflow. Work in an isolated branch/worktree. Use the locked Rust toolchain and a unique Cargo target on local SSD. Do not install Blender, IFC packages or Python libraries into the host for core checks.

Run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace` for Rust changes. For the demo, build `archy-cli` and run `ARCHY_BIN=/absolute/path/to/archy python3 -m unittest discover -s tests/demo -v`. The demo uses only Python's standard library. Record commands, exit codes, source and artifact hashes, environment and any deferred gate in a handoff. Independent review is required before declaring a slice accepted.

Preserve accepted runs, failed variants, and user files. Never infer approval from an agent's text or infer legal compliance from geometric checks. Actual Omarchy install, containment, backend artifacts, visuals and independent IFC inspection remain separate acceptance gates.
