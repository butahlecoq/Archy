# Backend research and proposed integration pins

Research date: 29 October 2026. Scope: specification sections 3, 6.2–6.3, 7, 9, and 10. This is a design recommendation based on official documentation, not evidence that either backend has run on the target Omarchy machine. No worker implementation or target-machine installation was performed for this note.

## Recommended baseline

| Component | Proposed exact pin | Rationale and boundary |
| --- | --- | --- |
| Blender | **4.5.15 LTS**, official Linux x64 archive | Supported 4.5 line through July 2027; conservative baseline for the 4.5 API. The currently newer 5.2.3 LTS is an upgrade candidate requiring the same smoke tests. |
| Blender Python | The interpreter bundled in the exact Blender archive | Run `bpy` inside Blender. Record `sys.version` and `bpy.app.version_string`; do not assume it equals the IFC environment. |
| IFC interpreter | **CPython 3.12.16**, separate virtual environment | Exact source release available 25 October 2026. A distro interpreter may be substituted only by an explicit tested pin update. |
| IFC library | **ifcopenshell==0.9.0.post1** | Matches the documentation inspected; use its Linux Python 3.12 wheel, independently of Blender. |
| IFC schema | **IFC4 / IFC4 ADD2 TC1 semantics** | Broad schematic handoff; no IFC4.3 infrastructure semantics required. |

Blender versions/support are verified on the [official release listing](https://www.blender.org/releases/) and [4.5 LTS page](https://www.blender.org/releases/4-5/). The Linux archive is listed in the [official 4.5 download index](https://download.blender.org/release/Blender4.5/). [Python 3.12.16](https://www.python.org/downloads/release/python-31216/) is an official release.

The [IfcOpenShell package metadata](https://pypi.org/project/ifcopenshell/0.9.0.post1/) declares Python >=3.10,<3.16. Its Python 3.12 Linux x86-64 wheel is `ifcopenshell-0.9.0.post1-py312-none-manylinux_2_31_x86_64.whl` (glibc >=2.31), SHA-256 `39fb69217fca787264ada575e10b46b2c038c96f85810bc5275b6b505fa91f7e`. Architecture and wheel tags must match the machine. An aarch64 wheel is separate; Blender x64 is not an ARM installation strategy.

Before committing dependency locks, download exact archives/wheels from official hosts, verify published hashes, and generate a complete requirements lock including transitive dependency versions and hashes. `ifcopenshell==...` alone is not a complete lock. Keep archive identity, worker source revision, dependency lock digest, CPU/GPU/driver, OS/glibc, and interpreter versions in every backend run manifest. Check security updates before release; a patch upgrade changes the tested baseline and must be recorded.

## Fixed subprocess entry points

Illustrative absolute deployment layout; replace only trusted installation/project roots. The following is the exact proposed Blender argument order:

```sh
/opt/archy/backends/blender-4.5.15-linux-x64/blender \
  --background --factory-startup --disable-autoexec --offline-mode \
  --python-exit-code 20 --threads 1 \
  --python /opt/archy/workers/blender/worker.py \
  -- --request /srv/archy/project/runs/RUN_ID/blender-request.json \
     --response /srv/archy/project/runs/RUN_ID/blender-response.json
```

The Rust process must construct an argument vector without a shell. The worker path and interpreter are administrator-selected absolute paths outside writable project data, not request fields. The agent supplies bounded typed geometry and preset identifiers only. Never accept Python text, expressions, module names, arbitrary executable options, imports, or `.blend` startup files in the request.

Blender documents background execution, Python script execution, an explicit exception exit code, user environment opt-in, and autoexecution controls in the [4.5 command-line reference](https://docs.blender.org/manual/nb/4.5/advanced/command_line/arguments.html); [scripting security](https://docs.blender.org/manual/id/4.5/advanced/scripting/security.html) explains automatic script execution. `--disable-autoexec` controls automatic scripts; it does not sandbox the explicitly invoked trusted worker. `--offline-mode` is an application preference override, not an OS network security boundary. Confirm all flags with the pinned binary's `--help` as a deployment gate.

For IFC:

```sh
/opt/archy/backends/ifc-venv/bin/python -I \
  /opt/archy/workers/ifc/worker.py \
  --request /srv/archy/project/runs/RUN_ID/ifc-request.json \
  --response /srv/archy/project/runs/RUN_ID/ifc-response.json
```

Use an environment allowlist, no inherited `PYTHONPATH`, isolated user/config/cache directories, no network permission, and process-group timeout/cancellation. Python `-I` and Blender startup flags do not supply filesystem/process isolation. Apply OS containment separately. Limit input bytes, vertices, storeys, image dimensions, samples, memory, CPU, and output bytes. Validate both the typed request and worker response, reject unknown fields and nonfinite numbers, resolve paths beneath the selected run directory, and reject symlink escapes. Write into a fresh staging directory and atomically publish only after verification; do not overwrite prior artifacts.

For a deterministic initial preview choose fixed Cycles CPU, seed, sample count, resolution, color-management settings, materials, lights, and camera presets in trusted worker code. Produce three named PNG views and one GLB with explicit unit scale. Prefer direct mesh/data construction over UI-context-dependent operators. Render pixel equivalence across different hardware remains a tolerance test, not a byte-identity promise. Record all settings. GLB's axis conversion must be accounted for by independent world-space bounds checks.

## Schematic IFC mapping

The following is the proposed Archy semantic contract, not an assertion of detailed BIM authoring or construction readiness.

| Archy concept | IFC mapping |
| --- | --- |
| Project/site/building | One `IfcProject`, one `IfcSite`, one `IfcBuilding` |
| Storeys | `N` `IfcBuildingStorey` spatial containers, index `i=0..N-1`, elevation `i * floor_to_floor_m` |
| Storey mass | One `IfcBuildingElementProxy` per storey, `PredefinedType=USERDEFINED`, `ObjectType=ArchySchematicStoreyMass` |
| Spatial relationships | Project → site → building → storeys via `IfcRelAggregates`; each proxy → exactly one storey via `IfcRelContainedInSpatialStructure` |
| Geometry | Closed simple rectangle/L polygon as `IfcArbitraryClosedProfileDef`, extruded along local +Z by floor-to-floor height using `IfcExtrudedAreaSolid` in `Body`/`SweptSolid` representation |
| Provenance | Custom `Archy_SchematicMassing` property set: schema/worker versions, variant/run IDs, source digest, schematic flag, approval state |
| Units/origin | Explicit metre SI length units, square/cubic metre quantities; right-handed local Z-up, no invented georeferencing |

IFC's [proxy definition](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/FINAL/HTML/schema/ifcsharedbldgelements/lexical/ifcbuildingelementproxy.htm) supports placeholders without a predefined detailed building-element meaning. For USERDEFINED, [the specification requires ObjectType](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD1/HTML/schema/ifcsharedbldgelements/lexical/ifcbuildingelementproxy.htm). Do not label these volumes as walls, slabs, apartments, spaces, or a quantity-surveying model.

Create the model with `project.create_file(version="IFC4")`; create roots with `root.create_entity`; explicitly assign SI units and a Model/Body/MODEL_VIEW context. Use documented [aggregate.assign_object](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/aggregate/assign_object/index.html) and [spatial.assign_container](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/spatial/assign_container/index.html) for relationships.

Use [profile.add_arbitrary_profile](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/profile/add_arbitrary_profile/index.html), which requires identical first/last coordinates for closure. Then [geometry.add_profile_representation](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/geometry/add_profile_representation/index.html) with metre depth and an explicit cardinal-point policy. The helper defaults to centering; test `cardinal_point=None` against the pinned code to preserve the supplied local footprint coordinates, or construct the simple swept solid explicitly if necessary. Do not silently accept a shifted L footprint. Assign the resulting representation to the proxy.

Storey placement is relative to the building; proxy placement is identity relative to its storey. If using [geometry.edit_object_placement](https://docs.ifcopenshell.org/autoapi/ifcopenshell/api/geometry/edit_object_placement/index.html), pass an explicit world-space 4×4 matrix in SI with `is_si=True` and verify resulting relative placements. Do not apply elevation in both the profile and the product transform. Test transform composition rather than relying on the `Elevation` attribute alone.

Attach custom quantities with explicit method names: footprint area, approximate gross floor area, and mass volume. GFA is `area * N`; total height is `N * floor_to_floor_m`. Avoid implying standardized measured floor-area or legal planning definitions. Deterministic project-derived GUIDs and stable ordering are useful; file timestamps and library serialization may still prevent byte-identical IFC output. Compare a canonical semantic digest separately from the recorded artifact SHA-256.

## Independent validation plan

1. Fixed rectangle and concave L fixtures supply analytic area, bounds, volume, storey count, height, and setbacks. Include translated footprints, nonzero north angle, touching boundaries, reversed rings, invalid rings, and maximum allowed sizes. Reject malformed input before either backend.
2. After IFC write, a fresh process reopens the actual file, verifies IFC4, explicit SI units, unique valid GUIDs, exact root/container/proxy counts, relation cardinality, property flags, and accumulated placements. Assert that no detailed wall/slab/space entities were emitted.
3. Run schema validation with the documented command below. Parse its findings; do not infer semantic or geometric correctness from process exit status alone. The [validation API](https://docs.ifcopenshell.org/autoapi/ifcopenshell/validate/index.html) documents limitations, including syntax/errors that need parser logging and incomplete rules coverage.

```sh
/opt/archy/backends/ifc-venv/bin/python -I -m ifcopenshell.validate \
  /srv/archy/project/runs/RUN_ID/model.ifc --rules --json
```

4. Independently tessellate reopened IFC proxies with IfcOpenShell geometry processing and compare world-space bounds and closed-volume measurements to the Rust analytic evaluator, using recorded tolerances (initial analytic coordinates 1e-6 m; geometry tessellation tolerances must be justified separately). This is independent of the exporter code path, but uses the same library and is not an independent parser.
5. Release gate: use a second engine, proposed [ThatOpen web-ifc](https://github.com/ThatOpen/engine_web-ifc), to parse and tessellate the IFC, verify proxy/hierarchy/units and world bounds, and visually inspect the exported L concavity and stacked storeys. Pin its actual npm artifact and integrity plus matching WASM at implementation time; no independently validated web-ifc version is claimed here. IfcConvert/Bonsai alone do not satisfy parser independence because they share IfcOpenShell.
6. Decode each PNG and check dimensions/nonempty foreground; independently parse GLB, validate buffers/accessors, and compare transformed bounds against the same analytic fixtures. Check expected files, media types, sizes, hashes, and run-root containment before returning success.

## License boundaries

Blender states that distributed binaries use GPL-3.0-or-later and published scripts using its Python API must be GPL compliant. Proposed policy: put `workers/blender` under **GPL-3.0-or-later** with its own SPDX headers/license; keep protocol schemas dependency-neutral. [Blender license policy](https://www.blender.org/about/license/?roistat_visit=2540962) and [FAQ](https://www.blender.org/support/faq/) are the primary references. Blender outputs normally remain user-owned; embedded scripts and third-party assets require separate treatment.

IfcOpenShell Python is **LGPL-3.0-or-later**; Bonsai is GPL, so using the Python library is a distinct dependency choice. Preserve notices, dependency source/offers and replacement/relinking rights required by actual distributed packages, and audit native dependencies. See the [official package license inventory](https://pypi.org/project/ifcopenshell/0.9.0.post1/) and [upstream COPYING.LESSER](https://github.com/IfcOpenShell/IfcOpenShell/blob/v0.8.0/COPYING.LESSER). Do not assume every package in the ecosystem shares that license.

A subprocess/data protocol is a deliberate architectural boundary; it is not a blanket legal conclusion that all combined distribution obligations disappear. Keep the Rust core's MIT/Apache proposal pending a distribution review, license the bpy worker separately, and document component licenses and source availability before shipping bundled binaries. User-installed dependencies reduce bundling work but do not remove obligations for published worker source. No legal compatibility certification is made here.

## Unverified gates before backend readiness

- Target Linux architecture/glibc, exact archive hashes, loader/native libraries, Python extension import, dependency lock, and worker license inventory.
- Pinned Blender `--version`/`--help`, bundled Python, startup isolation, offline behavior, headless Cycles CPU render, and GLB exporter availability. No GPU performance/driver compatibility claim yet.
- Exact IfcOpenShell helper signatures/cardinal-point behavior, metre handling, concave profile extrusion, containment, and transform composition on the pinned wheel.
- Real rectangle/L backend fixtures, three demo variants, all output verification, timeout/cancellation/crash recovery, and malformed-response/path traversal tests.
- Independent parser version/integrity and real IFC interoperability evidence; approval/draft properties agree with recorded human decision.
- Clean Omarchy install and golden workflow with archived environment/run manifests. Until these pass, report backend availability as unverified; documentation research is not acceptance evidence.
