# Roadmap

## 0.1: Proof of concept

The local massing workflow is implemented. Run `python -m archy.workflow examples/massing-brief.yaml output/massing-study` to evaluate the brief, generate and inspect a Blender scene, render each alternative, export and independently verify an IFC4 massing model, and write JSON and HTML comparison reports with a version 2 artifact manifest. Each IFC alternative has its own building and elevation-only storeys. The geometry is a rectangular solid with optional counter-clockwise plan rotation around its local lower-left corner and constant floor-to-floor height; floor plates and other detailed BIM elements are not modeled.

The workflow has been verified on Omarchy with Python 3.14.8, PyYAML 6.0.3, Blender 5.2.2 LTS, and IfcOpenShell 0.9.0post1. The project CLI and local stdio MCP server store immutable revisions, full study bundles, and a compare-and-swap current-revision pointer. Richer BIM semantics and building-code evaluation remain incomplete.

An end-to-end agent workflow has also completed with Codex CLI v0.165.0 and
`gpt-6-luna` at medium reasoning. The headless run used on-request approval
through `codex exec --approve-for-me`. The agent used Archy's guidance tool and
schema and operation resources, then evaluated and previewed three alternatives.
It created and read a temporary project, ran and retrieved a verified study,
then created, evaluated, and promoted a revision. This confirms the tested CLI,
model, and setup only. Other runtimes and versions still need evaluation.

## After the proof of concept

Use the results to decide whether to keep extending the Python CLI or build a Rust CLI, and whether to add backend-independent geometry interfaces and richer validation.
Archy should remain a deterministic decision and handoff layer for early massing, not a general CAD or BIM authoring tool. Before building overlapping capabilities, investigate upstream contributions and integrations with existing BIM, CAD, and analysis tools. Richer features such as rooms, facades, cost, energy, or collaboration require separately approved bounded designs and independent validation.

## Open decisions

- Choose an open-source license and confirm the project name is usable.
- Decide how to version future richer design geometry and measurement rules while retaining the optional v1 maximum-height constraint.
- Define how structured specifications and IFC data stay consistent.
- Choose and evaluate integrations, their versions, and their security boundaries.
- Decide which project files and generated artifacts belong in Git or external storage.
- Evaluate other agent runtimes and versions, and compare their behavior with the tested Codex setup.
