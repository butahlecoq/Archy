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
