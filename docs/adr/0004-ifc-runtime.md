# ADR 0004: Pinned IFC runtime and bounded schematic worker

Accepted for WSL development on 2026-10-29, pending independent review and
target Omarchy acceptance. The development runtime is CPython 3.12.16 from
the official source archive SHA-256
`b5796915dc45ff745cfb4e990a7e7c93d6f546328a102ce31448d69ebc37575c`
and IfcOpenShell 0.9.0.post1 Linux CPython 3.12 x86-64 wheel SHA-256
`39fb69217fca787264ada575e10b46b2c038c96f85810bc5275b6b505fa91f7e`.
The interpreter, venv and fetched wheels stay in workspace `.tools`; the full
15-package tested runtime graph is in `workers/ifc/requirements.lock` with hashes.
This avoids Blender Python and Arch system Python. It is WSL evidence only.

The worker accepts one exact, bounded `ifc-worker-v1` request and writes a
schematic IFC4 Project→Site→Building→Storeys hierarchy. Each storey contains
one `USERDEFINED` `IfcBuildingElementProxy` with one Body/SweptSolid extrusion
of the canonical local footprint. Site placement is identity, building XY
placement is the variant origin, storey Z is `index × floor_to_floor_m`, and
the proxy is identity relative to its storey. North is provenance metadata.
Metre SI units are explicit. All IfcRoot GUIDs use UUIDv5 from the documented
`uuid5(NAMESPACE_URL, "urn:archy:ifc:v1")` namespace and typed semantic roles,
including relationship endpoints and property-set/quantity names,
excluding run IDs and timestamps. Export timestamps are recorded separately.

`Archy_SchematicMassing` records canonical source digest/references, method,
versions, coordinate convention, schematic flag and `DRAFT`. Custom quantities
are analytic footprint, approximate GFA and mass volume with named methods and
explicit units. Worker-supplied actor/approval fields are rejected. A14-backed
human authority remains a future Rust concern; the worker cannot approve.

The worker reopens the actual written IFC in a fresh process, checks hierarchy,
GUIDs, the project's active SI unit assignment, placements, profile vertices,
quantities, draft state and native
tessellation. It also invokes IfcOpenShell EXPRESS validation. These checks
share the exporter engine and do not close A13. A09 must supervise the process,
validate artifacts independently and enforce OS isolation; A05 alone publishes
accepted immutable artifacts. Neither the MIT worker nor Rust relicenses the
LGPL library or bundled native components. Distribution notice/source review
and clean Omarchy execution remain required.

The worker runtime dependencies and build frontend/backend dependencies have
separate full hash locks. The build backend/frontend are pinned in the
workspace build venv; the resulting wheel is installed into the distinct IFC
runtime venv without changing its locked library versions.
