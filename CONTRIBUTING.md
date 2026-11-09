# Contributing to Archy

Archy is a deterministic architectural massing decision and handoff layer. It is intentionally not a general CAD or BIM authoring tool.

## Before contributing

Read:

- `README.md`
- `docs/roadmap.md`
- `docs/architecture.md`
- `AGENTS.md`

## Scope policy

Contributions must support Archy's narrow core:

- structured briefs
- deterministic evaluation
- comparison and reporting
- IFC export and verification
- project history and revisions
- agent-facing access

Do not add overlapping CAD, BIM, rendering, simulation, compliance, or collaboration features without an approved bounded design and integration analysis.

## Development

Use the repository's virtual environment and installed dependencies. Keep changes scoped to one issue.

Run relevant verification before submitting:

```bash
.venv/bin/python -m archy.workflow examples/massing-brief.yaml /tmp/archy-study --blender /usr/bin/blender
.venv/bin/python -m archy.verify /tmp/archy-study
```

## Pull requests

Every pull request must include:

- the issue it addresses
- scope and non-goals
- verification commands and results
- known limitations
- documentation updates when behavior changes

Do not hide failing checks. Record them as evidence.

## Release contributor workflow

# Contributing

Read [AGENTS.md](AGENTS.md), the [release specification](docs/specs/release-0.1.txt), and the [backlog](docs/plans/backlog.md) before changing a workflow. Work in an isolated branch/worktree. Use the locked Rust toolchain and a unique Cargo target on local SSD. Do not install Blender, IFC packages or Python libraries into the host for core checks.

Start with the [source quickstart and version matrix](docs/contributor-quickstart.md). Use the [session handoff template](docs/templates/session-handoff.md) to leave exact commands, environment, artifacts, open gates, and the independent review state.

Run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace` for Rust changes. For the demo, build `archy-cli` and run `ARCHY_BIN=/absolute/path/to/archy python3 -m unittest discover -s tests/demo -v`. The demo uses only Python's standard library. Record commands, exit codes, source and artifact hashes, environment and any deferred gate in a handoff. Independent review is required before declaring a slice accepted.

Preserve accepted runs, failed variants, and user files. Never infer approval from an agent's text or infer legal compliance from geometric checks. Actual Omarchy install, containment, backend artifacts, visuals and independent IFC inspection remain separate acceptance gates.
