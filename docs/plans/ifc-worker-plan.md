# A11/A12 IFC Python worker brief

Research: 29 October 2026; implementation proposal, not runtime acceptance. Only this document is changed.
Inputs: repository `AGENTS.md`, [release specification](../specs/release-0.1.txt), [backend research](backend-research.md), [A11–A13 backlog](backlog.md), [Task02 contracts](task-02-handoff.md), and [A05 persistence plan](task-03.md).
Goal: export one canonical rectangle/L variant as schematic IFC4 masses, with reproducible identity, explicit draft/provenance, strict protocol and verifiable geometry. No arbitrary imported IFC editing, mesh-to-wall conversion or new BIM kernel.
A11 owns runtime/hierarchy; A12 owns selected-variant export and design manifest. A13 owns reopened measurement and independent-engine acceptance; A09 owns bounded execution/isolation and artifact acceptance. Do not duplicate those components.

## Verified runtime candidates

Primary sources were retrieved successfully through web/curl; no wheel, interpreter archive or heavy asset was downloaded, installed or executed.

| Component | Evidence and decision |
| --- | --- |
| IfcOpenShell | [Exact PyPI JSON](https://pypi.org/pypi/ifcopenshell/0.9.0.post1/json) reports `0.9.0.post1`, Python `>=3.10,<3.16`, `LGPL-3.0-or-later`. Adopt this exact candidate pending import/native smoke. |
| Linux wheel | `ifcopenshell-0.9.0.post1-py312-none-manylinux_2_31_x86_64.whl`; 54947812 bytes; uploaded `2026-10-15T11:38:03.717273Z`; not yanked. Requires matching Python 3.12/x86-64 and glibc baseline implied by manylinux_2_31; not an ARM/musl package. |
| Published wheel SHA-256 | `39fb69217fca787264ada575e10b46b2c038c96f85810bc5275b6b505fa91f7e`, freshly read from PyPI, not inherited unverified from old research. Actual bytes have not been hashed. |
| CPython | [Official 3.12.16 release](https://www.python.org/downloads/release/python-31216/) dated 25 October 2026: available as source only, no official binary installers. Exact candidate is CPython 3.12.16 in a separate project-local environment, built from verified source or an independently verified managed build. |
| CPython source checksum | Release page lists XZ source SHA-256 `b5796915dc45ff745cfb4e990a7e7c93d6f546328a102ce31448d69ebc37575c`. This authenticates the proposed source artifact only after download/check, not any third-party interpreter binary. No managed-build hash is claimed. |
| Dependencies | PyPI requires shapely, numpy, isodate, python-dateutil, lark, pyparsing, typing-extensions; advanced/dev extras are unnecessary. Resolve and hash-lock the full tested graph during implementation; one IfcOpenShell pin is not a complete lock. |

Use a dedicated venv with the exact tested interpreter; never reuse Blender Python or modify Arch system Python. If 3.12.16 cannot be provisioned, report that blocker rather than silently substituting a distro patch version.
The wheel URL is supplied by the exact PyPI JSON entry above; select by complete filename and digest, not by a mutable “latest” endpoint. Record source URL, artifact hash, interpreter build provenance, native versions, OS/architecture/glibc and lock digest.

## Reusable APIs verified in official documentation

Docs identify themselves as IfcOpenShell 0.9.0post1. Signatures below were inspected; runtime behavior must still be checked against the pinned wheel. Import named helpers, not a request-supplied API dispatcher.

| Helper/source | Bounded use |
| --- | --- |
| [project.create_file](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/project/create_file/index.html) | `create_file(version="IFC4")`; creates header defaults including time and an MVD label. Do not mistake that default label for certified MVD compliance. |
| [root.create_entity](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/root/create_entity/index.html) | `create_entity(file, ifc_class=..., predefined_type=..., name=...)`; assigns initial GUID/ownership defaults. Replace random GUIDs using the deterministic policy below. |
| [aggregate.assign_object](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/aggregate/assign_object/index.html) | `assign_object(file, products=[child], relating_object=parent)` for spatial decomposition. |
| [spatial.assign_container](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/spatial/assign_container/index.html) | `assign_container(file, products=[proxy], relating_structure=storey)`; containment is distinct from spatial aggregation. |
| [unit.add_si_unit](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/unit/add_si_unit/index.html), [assign_unit](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/unit/assign_unit/index.html) | Create LENGTHUNIT/AREAUNIT/VOLUMEUNIT with `prefix=None`; `assign_unit(file, units=[...])`. Avoid convenience defaults that might select millimetres. |
| [context.add_context](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/context/add_context/index.html) | Create Model context and Body subcontext with `target_view="MODEL_VIEW"`, `parent=model_context`. |
| [profile.add_arbitrary_profile](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/profile/add_arbitrary_profile/index.html) | `add_arbitrary_profile(file, profile=closed_xy_points, name=None)` expects SI metres and identical first/last point; returns `IfcArbitraryClosedProfileDef`. |
| [geometry.add_profile_representation](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/geometry/add_profile_representation/index.html) | `(file, context, profile, depth=1.0, cardinal_point=5, clippings=None, placement_zx_axes=(None,None))`; depth is metres. Use `cardinal_point=None` as the candidate no-recentering policy; verify translated L geometry before accepting it. |
| [geometry.edit_object_placement](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/geometry/edit_object_placement/index.html) | `(file, product, matrix=None, is_si=True, should_transform_children=False)` supports local placements and 4x4 matrices. Set hierarchy first and place parent-to-child; test accumulated transforms. |
| [guid.compress](https://docs.ifcopenshell.org/autoapi/ifcopenshell/guid/index.html) | `compress(uuid_hex)` converts UUID hex to IFC GUID. Avoid random `guid.new()` for stable exported identities. |

## IFC4 representation and identity policy

Use IFC4 / ADD2 TC1 semantics supported by package metadata: one IfcProject, IfcSite and IfcBuilding; N IfcBuildingStorey and N IfcBuildingElementProxy. No detailed walls, slabs, spaces or construction semantics.
Aggregate Project→Site→Building→Storeys; contain each proxy in exactly its corresponding storey. Set proxy `PredefinedType=USERDEFINED`, `ObjectType="ArchySchematicStoreyMass"`, deterministic names and a schematic description.
Create one closed rectangle/L profile and per-storey +Z extrusion of `floor_to_floor_m`; attach a Body/SweptSolid representation to each proxy. Reuse profiles safely; no overlapping-box approximation of L geometry.
Normalize profile coordinates relative to the variant origin; put XY translation in the building placement, storey Z at `i * floor_to_floor_m`, proxy placement identity relative to its storey. Site placement is identity; north angle is metadata, not an implicit rotation.
When using placement helper matrices, supply the desired world transforms and verify resulting relative chains: building=(ox,oy,0), storey/proxy=(ox,oy,i*h). Never add translation/elevation in both profile vertices and placement.
If `cardinal_point=None` recenters unexpectedly, fail the fixture and use explicit `IfcExtrudedAreaSolid`/axis placement construction; no silent compensating geometry shifts. This fallback needs the same parity tests.
Stable IDs: use stdlib UUIDv5 with a fixed documented Archy namespace derived from `NAMESPACE_URL` and `urn:archy:ifc:v1`; compress UUID hex. Names include project ID plus typed role, variant ID for variant-specific entities, and zero-based storey index.
Give relationship/property-set IfcRoot entities deterministic IDs too, using endpoint roles and stable property-set names; inspect all IfcRoot instances for uniqueness and stable ordering. Run IDs/timestamps never enter entity identity.
Record execution timestamps honestly in provenance; compare a normalized semantic inventory for reproducibility rather than promising byte-identical STEP headers. Hash the actual IFC bytes separately.
Attach `Archy_SchematicMassing` provenance: project/variant/run IDs, canonical-input digest, worker/library/schema versions, metre coordinate convention, method/tolerance versions, schematic=true, draft status and source artifact references.
Always emit DRAFT in this slice. A future A14-backed approval record may change labeling only after Rust verifies human authority; actor strings or a worker request's boolean never establish approval.
Custom quantities state footprint area, approximate GFA=area×storeys, and mass volume=area×height with units/methods. Recompute from canonical input; do not trust deserialized metric summaries or imply net area, cost, legal compliance or professional certification.

## Strict worker boundary and errors

Request schema: version, fixed operation `export_schematic_ifc`, request/project/variant/run IDs, canonical Variant, normalized site, source digest, and project-relative logical output destination. No paths to imported IFC, executable options, code, module/helper names or expressions.
Rust revalidates canonical identity/geometry and recomputes evaluation before dispatch; Python independently checks finite parameters, footprint consistency and bounded structure. Preserve legitimate failed design checks separately from export success.
Initial capacity proposal: <=1 MiB request, one variant, exactly 5/7 closed footprint vertices, 1..100 storeys, absolute coordinates/heights <=10000 m; unsupported capacity is an explicit failure, not a new core design rule or silent clamp.
Reject duplicate/unknown JSON fields, unsupported versions, nonfinite values and conflicting IDs. Package Python modules in the venv so `python -I -m archy_ifc_worker` works without project/PYTHONPATH imports; the trusted module has no runtime plugin discovery.
Use fixed argv, read-only input, fresh private output staging and fixed basenames `model.ifc`, `validation.json`, `response.json`. The response is written last; stdout/stderr are bounded diagnostics, not the JSON channel.
Response echoes IDs/digest and includes worker/backend/interpreter versions, status, warnings, validation results and `{path,sha256,media_type,producer,size_bytes}` for complete outputs. Rust produces the final design manifest linking source, metrics, hashes, draft state and provenance.
Errors reuse code/message/field/retriable: INVALID_SCHEMA/INVALID_GEOMETRY for requests; BACKEND_UNAVAILABLE for missing/incompatible native runtime; BACKEND_TIMEOUT from the supervisor; ARTIFACT_INVALID for invalid outputs; PERMISSION_DENIED for unsafe destinations; INTERNAL_ERROR for unexpected failures. No partial success or invented artifacts.
A09 must enforce timeout/cancellation, network/filesystem isolation, resource/log/output quotas, no-follow checks and independent hashes; `-I` is not an OS sandbox. A05 alone publishes accepted immutable runs; worker cannot modify revisions, prior exports or approval records.

## Validation, A13 independence and smoke evidence

The [official validation reference](https://docs.ifcopenshell.org/autoapi/ifcopenshell/validate/index.html) documents `validate(f, logger, express_rules=False) -> None` and `json_logger`; inspect logger statements and parser logs, not the function return as a pass flag.
Run `/trusted/ifc-venv/bin/python -I -m ifcopenshell.validate /job/output/model.ifc --rules --json`; verify installed CLI help and capture exit code, JSON findings and stderr. Validation has documented coverage limits and is not a proof of geometry or regulatory validity.
Fresh-process reopen must verify schema, metre units, exact hierarchy/proxy counts, unique GUIDs, containment, world placements, draft properties and artifact hash. Deliberate corrupt/truncated/wrong-unit/wrong-placement fixtures must fail.
Tessellate reopened proxies with IfcOpenShell and compare world bounds, closed volume, footprint/GFA and storey heights to canonical geometry using Task02's versioned coordinate and propagated area/GFA budgets. An exporter reopen is not an independent engine.
A13/A19 require a second engine: [ThatOpen web-ifc](https://github.com/ThatOpen/engine_web-ifc) is a candidate only; its current package/version/WASM hash/license/runtime were not verified in this bounded task. Do not invent or claim an installed pin.
Before independent acceptance, pin its actual npm artifact plus matching WASM integrity/license, prove IFC4 proxy parsing/tessellation, apply accumulated transforms, and compare selected masses/hierarchy/units with the same fixtures; archive parser versions, hashes, measurements and visual notes. If unavailable, retain an explicit gate failure/deferred status.
IfcConvert and Bonsai share IfcOpenShell and do not satisfy engine independence. No second-engine implementation is required to begin A11 hierarchy work, but absence prevents later independent acceptance.

## Implementation sequence and licensing

- [ ] A11: create `workers/ifc/{requirements.lock,README.md,protocol.py,hierarchy.py,worker.py}`, packaging metadata, tests and ADR0004. Provision exact interpreter, hash-check wheel/transitives, inspect native versions, import, create/write/reopen a minimal IFC4 hierarchy.
- [ ] A12: add `geometry.py`, `properties.py`, Rust adapter/CLI glue and design-manifest schema; reuse A09 supervisor/A05 publication rather than adding a second process/storage path. Keep core unit tests backend-optional.
- [ ] Smoke: Task02 rectangle area 200/GFA 600 m² and L area 75/GFA 225 m², three storeys at 3.2 m, translated [10,-5], north metadata, and two identical repeated exports. Assert 9.6 m height, notch preservation, transforms, stable GUIDs and DRAFT state.
- [ ] Negative tests: tampered variants/summary authority, malformed profiles, unsupported capacities, occupied/symlink/traversal output, fake approval, broken files and wrong quantities. Prior artifacts and outside sentinels remain unchanged.
- [ ] Attach exact commands/exits, schemas, unit/native integration results, package hashes, semantic comparisons, validation findings and independent review. No agents are dispatched by this research task.

License evidence: current PyPI metadata declares LGPL-3.0-or-later; upstream [COPYING.LESSER at v0.8.0](https://github.com/IfcOpenShell/IfcOpenShell/blob/v0.8.0/COPYING.LESSER) was accessible and contains LGPLv3, but that historical file is not an audit of the 0.9.0.post1 wheel.
Retain MIT for original Rust; original thin Python application glue may use MIT without copying upstream implementation, while IfcOpenShell/native dependencies retain their licenses. Inventory actual wheel and transitive notices/source/replacement obligations before distribution. Bonsai's GPL is separate; do not bundle it unnecessarily or claim subprocess separation removes obligations.

Remaining native checks: interpreter provisioning/build provenance, actual artifact hashes, wheel tag/loader/OpenCascade compatibility, NumPy/native imports, helper defaults/cardinal-point behavior, placement composition, EXPRESS validation output and geometry extraction. All remain unrun here.
WSL/Ubuntu real-worker smoke is development evidence only. Actual Omarchy clean installation, native export/reopen, second-engine interoperability and visual review remain deferred target gates; no release/target pass is inferred. Enough primary evidence exists to begin bounded A11/A12 work without further exploratory research.
