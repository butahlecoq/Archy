# Archy

Archy is a prototype for agent-centered architectural design on Omarchy. It turns a structured massing brief into inspectable alternatives, BIM deliverables, and reproducible comparisons.

The repository includes a deterministic massing evaluator, a local MCP server, a Blender workflow, and a project CLI. It validates rectangular alternatives, including optional plan rotation and an optional maximum building-height limit, records brief provenance, generates inspectable geometry, renders isolated previews, checks saved geometry, exports and verifies an IFC4 model, and writes JSON and HTML comparison reports. Rotation is counter-clockwise in degrees around the footprint's local lower-left corner; an omitted angle means zero. Each IFC alternative has its own building and storey levels, with one full-height mass proxy. The project CLI and MCP tools store immutable brief revisions and study history.

Omarchy is the initial target environment for development and the proof of concept. The project should document any Omarchy-specific setup while keeping its architectural specifications independent of the desktop environment.

## Direction

Archy owns the project specification, constraints, design decisions, and evaluation history. Existing tools provide specialized modeling and BIM capabilities. The intended source of truth is a structured design specification together with IFC data; a Blender scene is a derived view, not the sole record of a design.

The first milestone is a proof of concept that takes one brief, generates three building masses without manual modeling, checks their stated geometric constraints, renders them, and produces a comparison whose measurements can be independently reproduced.

Archy is intentionally not a general CAD or BIM authoring tool. It is a deterministic decision and handoff layer for early massing: it owns the brief, evaluation, comparison, verification, and project history, while reusing existing tools such as Blender and IfcOpenShell for geometry, rendering, and exchange. Overlapping capabilities should be investigated as upstream contributions or integrations before being built into Archy.

## Documents

- [Architecture](docs/architecture.md): proposed responsibilities, data, and boundaries.
- [Proof of concept](docs/proof-of-concept.md): workflow, deliverables, and success criteria.
- [Massing brief schema](docs/massing-brief.schema.json): version 1 machine-readable brief shape.
- [Agent tools](docs/agent-tools.md): local MCP access to deterministic massing evaluation.
- [Integrations](docs/integrations.md): candidate projects and questions to investigate.
- [Project structure](docs/project-structure.md): project revisions, study runs, and history.
- [Roadmap](docs/roadmap.md): milestones and decisions still to make.

## Run the massing proof of concept

Run the complete local workflow from the repository root with `python -m archy.workflow examples/massing-brief.yaml output/massing-study`. It evaluates the brief, builds and inspects the Blender scene, renders one isolated PNG per alternative, exports an IFC4 file, verifies its structure, geometry, and properties, and writes JSON and HTML comparison reports. Each option has a separate building, one level per floor, and one full-height proxy. Constraint failures remain in the report so alternatives can still be compared. Before Blender starts, the workflow validates the IFC input, checks that IfcOpenShell is available, and requires each building's total extrusion height to be at least 0.0001 m (0.1 mm). The output bundle includes `massing.ifc` and `manifest.json` with artifact hashes, sizes, and Blender provenance. Verify a copied or archived bundle with `python -m archy.verify output/massing-study`. Blender must be on `PATH`. Install IFC dependencies with `python -m pip install '.[ifc]'` in the Archy environment.

For standalone exchange, create an evaluation with `python -m archy examples/massing-brief.yaml evaluation.json`, then run `python -m archy.ifc evaluation.json massing.ifc`. Verify it with `python -m archy.ifc_verify evaluation.json massing.ifc`. These commands require the `[ifc]` extra.

## Check the local toolchain

Run `.venv/bin/python -m archy doctor` for a readable diagnostic, or add `--json` for machine-readable output. The Blender check launches Blender with a bounded factory-startup probe that imports `bpy`, `math`, `hashlib`, and `subprocess`, then reports Blender's embedded Python prefix. This confirms that Blender starts and those imports are available in its embedded Python; it does not establish compatibility with Archy's Python environment or certify that the desktop environment is contained or safe. Each check has a name, status (`available`, `unavailable`, or `deferred`), detail, and an optional remedy. The command exits 1 when a required Blender or IFC dependency is unavailable, and 0 otherwise. Compatibility checks, desktop containment checks, and packaging readiness remain deferred; this command makes no full readiness claim.

## Keep project revisions and studies

Create a project from a brief with `python -m archy project init output/courtyard --brief examples/massing-brief.yaml`. Create a proposed revision with `python -m archy project revise output/courtyard --brief revised-brief.yaml --message "Adjust the east wing"`. Each revision keeps its exact brief bytes and parent revision. A revision does not become current until you promote it.

Run the current revision with `python -m archy project run output/courtyard`. To run a proposal, pass `--revision REVISION_ID`. List revisions and study results with `python -m archy project history output/courtyard`. Promote a proposal only if the current revision still matches the value you reviewed: `python -m archy project promote output/courtyard REVISION_ID --expected-current CURRENT_ID`. Project studies retain the existing workflow artifacts and manifest. Verify a run with `python -m archy.verify output/courtyard/runs/RUN_ID`.

To let an MCP host help draft and inspect studies, install the `[mcp,ifc]` extra, put Blender on `PATH`, and launch `archy-mcp`. The local stdio server exposes the evaluator, a temporary full-workflow preview, eight persistent project tools, the brief schema, operation contract, and a study-drafting prompt. Project tools use UUIDs and a server-configured data directory; they do not accept filesystem paths. Preview artifacts are returned to the agent and removed after the call. See [Agent tools](docs/agent-tools.md) for storage configuration and tool details.

Archy is a working name. Trademark, package, and repository-name availability need further investigation. A license has not yet been chosen.
