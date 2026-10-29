# Archy contributor and agent instructions

The release baseline is `docs/specs/release-0.1.txt`, extracted from the founder-supplied 29 October 2026 specification. It supersedes the older proposal documents where they conflict. The full release backlog is `docs/plans/backlog.md`.

- Target runtime: Omarchy/Arch Linux. Windows is a development host, not proof of Linux runtime acceptance.
- Rust owns CLI, normalized types, deterministic generation/evaluation and orchestration. Packaged Python workers own Blender and IfcOpenShell operations through bounded typed requests.
- Do not invent regulatory requirements or claim professional/code compliance. Preserve source and authority of every constraint.
- Do not silently discard failing designs. Preserve reproducible inputs, metrics, explanations and immutable run artifacts.
- No arbitrary Python execution, unscoped output paths, silent overwrites, or agent self-approval.
- Each bounded change needs a short plan, meaningful tests, dependency rationale, independent review and a handoff with exact validation evidence. Never call an unrun check passing.
- Use Sol at medium reasoning for implementation and review when delegating, per founder instruction. The coordinating agent owns integration and research. Implementers must not spawn additional agents.
- Keep backend dependencies optional for core unit tests; test real backends as explicit integration/release gates.
- License choices and third-party obligations must be recorded before distribution. Do not imply GPL/LGPL tools are relicensed by the core license.
