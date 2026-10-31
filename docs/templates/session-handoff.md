# Session handoff template

Date and contributor:

## Scope and source

- Issue / bounded slice:
- Base commit and worktree:
- Changed files and reason:
- Explicit exclusions / dependencies:
- New dependencies and rationale (or none):

## Environment and exact checks

- OS and whether this is actual Omarchy, native Linux, CI, or WSL:
- Hardware (CPU, RAM, GPU when relevant), storage type, and whether elapsed time was measured:
- `git rev-parse HEAD` and `git status --short`:
- Effective Rust, Cargo, Python, Omarchy, Blender, and IFC versions; mark unavailable or unverified rather than guessing:
- Commands, exit codes, and meaningful output (including expected nonzero stops):
- Tests/fixtures and result:

## Evidence and review

- Demo project path and run IDs, if used:
- Project-relative report and artifact paths with SHA-256 hashes; distinguish generated files from reviewed release assets:
- Artifact sizes in bytes and measured elapsed time, if this session is used as benchmark evidence (no implied throughput target):
- Any failed variants, recovery state, or retained logs:
- Independent reviewer, review revision, findings, and disposition; write **pending** until reviewed:

## Open gates and next action

- Omarchy install/package, backend smoke, render, IFC, visual, containment, and approval gates: tested / deferred, with evidence for each tested gate:
- Known limits and blockers:
- Next owner and one concrete next action:
- Decisions requiring an actual human, including selection or approval:

An agent's text never grants approval. A source build, WSL run, or simulated backend result never establishes target Omarchy acceptance. Do not call an issue complete while its required gates remain open.
