# Archy 0.1 proof of concept

The proof of concept asks whether an architectural brief can drive a reproducible massing study with inspectable outputs. The repository implements the deterministic evaluator, MCP tools for evaluation and persistent project management, temporary previews, the Blender workflow, and IFC4 export and verification.

A real end-to-end agent workflow completed with Codex CLI v0.165.0 and
`gpt-6-luna` at medium reasoning. The headless run used on-request approval
through `codex exec --approve-for-me`. It used Archy's guidance tool and schema
and operation resources, then evaluated and previewed three alternatives. The
agent created and read a temporary project, ran and retrieved a verified study,
then created, evaluated, and promoted a revision. These results apply to the
tested CLI, model, and setup. Other runtimes and versions remain unverified.

## Implemented local workflow

Run `python -m archy.workflow examples/massing-brief.yaml output/massing-study` from the repository root. The workflow:

1. Validates the versioned YAML brief and evaluates its area and setback constraints, plus the optional maximum building-height limit.
2. Records measurements, formulas, inputs, and the source brief SHA-256 digest.
3. Generates a metric Blender scene with one rectangular solid per alternative, honoring its optional counter-clockwise plan rotation around the local lower-left footprint corner (omitted rotation means zero).
4. Renders an isolated PNG preview for each alternative.
5. Reopens the saved scene and checks each mesh's world-space bounds against the brief.
6. Exports one IFC4 building per alternative, adds one storey level per floor, and places one full-height mass proxy in each building. The verifier checks the IFC structure, proxy geometry, and properties against the evaluation.
7. Writes JSON and HTML comparison reports with measurements, constraint results, geometry checks, previews, and an IFC download link.
8. Writes a version 2 artifact manifest with hashes for every deliverable.

Install the optional IFC dependencies with `python -m pip install '.[ifc]'`. For a complete run, use `python -m archy.workflow examples/massing-brief.yaml output/massing-study`. To export an existing evaluation separately, use `python -m archy.ifc evaluation.json massing.ifc` and verify it with `python -m archy.ifc_verify evaluation.json massing.ifc`.

This workflow has been run on Omarchy with Python 3.14.8, PyYAML 6.0.3, Blender 5.2.2 LTS, and IfcOpenShell 0.9.0post1. The geometry is limited to rectangular footprints (optionally rotated in plan) extruded to uniform floor heights. Storey levels record elevations but have no floor geometry. Archy does not model courtyards, detailed floor plates, or building-code requirements.

## Remaining proof-of-concept work

- Evaluate the same end-to-end flow with other agent runtimes and versions.
- Extend IFC semantics beyond alternative-specific buildings, storey levels, and massing proxies, then validate the additional data independently.
- Evaluate additional MCP clients and reviewed Blender or IFC modeling integrations.
- Record versions, launch setup, and security boundaries for each additional setup on Omarchy.

## Success criteria

The full milestone succeeds when one agent-authored brief drives three inspectable alternatives through the modeling integrations, each alternative has recorded parameters, a render, evaluation results, and a BIM model whose reported measurements can be independently reproduced. The local workflow produces and verifies a separate IFC building, storey levels, and a full-height mass proxy for each alternative. This criterion is met for the tested Codex CLI v0.165.0, `gpt-6-luna` with medium reasoning, and local Blender and IfcOpenShell setup. Other agent runtimes and detailed BIM semantics remain incomplete.

The proof of concept does not establish building-code compliance or validate circulation, detailed floor plans, structural feasibility, or construction readiness.
