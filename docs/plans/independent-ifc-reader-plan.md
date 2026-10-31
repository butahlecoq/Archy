# A13 independent IFC reader plan

Research date: 29 October 2026. Bounded WSL development/release-verification plan; no engine parse or target acceptance has run in this research. A11/A12's IfcOpenShell exporter/reopener is a separate implementation and cannot provide second-engine evidence by itself.

## Verified reusable package

The official [npm registry version metadata](https://registry.npmjs.org/web-ifc/0.0.78) identifies **web-ifc 0.0.78**, MPL-2.0, with Node CommonJS entry `web-ifc-api-node.js`. No `engines` or runtime dependency list is declared in this package. Its README's Node >=16 requirement is a source-build requirement, not a tested runtime-support matrix. The available development Node is **v22.22.2**; actual compatibility must be demonstrated.

The [published tarball](https://registry.npmjs.org/web-ifc/-/web-ifc-0.0.78.tgz) was downloaded to workspace `.tools/web-ifc-research`, and its actual SHA-512 matched registry integrity:

`sha512-LGKMNYFVNyz9vVERN0iz+zvPQJqT7MEGBxslzcuuhwDDOA116LmJLKkg8sZ5SEvQJSc5NRVMWgpRNkDmuoQoJg==`

The archive contains matching Node JS/WASM, browser builds, declarations and MPL license. After checking its member paths and extracting it, actual SHA-256 values were:

- `web-ifc-node.wasm`: `36b10f4f9fada12bb89ed664f9e61eae1c82b2ce5f04ee4175205149516498f5`
- `web-ifc-api-node.js`: `592491e48f22ba5dd2f1d33a6485469bed63ac1b5f48547a80b146536556ff6f`

Use these matching files from the exact package; never mix the moving upstream build ZIP, browser WASM and Node loader. A separate development verifier package should pin `web-ifc` and commit its npm integrity lock. Install without package scripts; no Three.js/viewer/Node build stack is required for the shipped Node loader. Preserve MPL notices and inspect actual distribution obligations before bundling; the Rust MIT license does not relicense this engine.

## Inspected APIs and limits

The pinned archive's `web-ifc-api-node.d.ts` and JS were inspected alongside the [official API reference](https://thatopen.github.io/engine_web-ifc/docs/classes/web-ifc.IfcAPI.html) and [upstream license](https://github.com/ThatOpen/engine_web-ifc/blob/main/LICENSE.md). Use the pinned declarations when they differ from moving documentation.

- `IfcAPI`, `SetWasmPath`, async `Init`, `OpenModel(Uint8Array, settings)`, `GetModelSchema`, and `CloseModel` provide the bounded parser lifecycle.
- Set `ALLOW_INCOMPATIBLE_SCHEMA_ALIASES=false` and `COORDINATE_TO_ORIGIN=false` explicitly. Set a documented `MEMORY_LIMIT`; external process wall/memory/output limits remain necessary.
- `GetLineIDsWithType` and `GetLine` inspect actual projects, units, spatial aggregation, containment, storeys, proxies and property relationships. Check the project's active `UnitsInContext`, not merely the existence of unused unit entities.
- `StreamAllMeshesWithTypes`/`StreamAllMeshes` return proxy IDs and placed geometries. Each geometry has `flatTransformation`; `GetGeometry`, `GetVertexArray` and `GetIndexArray` expose mesh arrays. Copy required arrays before disposal/reuse, validate counts/indices/finite values, and free model/geometry resources according to the pinned API.
- `GetCoordinationMatrix` and `GetWorldTransformMatrix` are available. Determine the actual mesh layout, matrix order, coordinate convention and unit-scaling behavior with translated unequal-sided fixtures; do not assume glTF's inverse Y-up conversion applies to web-ifc. Do not apply a placement twice. Streaming defaults to applying linear unit scaling: explicitly select/record that setting and test wrong-unit fixtures.

The declarations expose Float32 mesh arrays. Preserve the reviewed Task02 coordinate/parity budgets; measure errors and fail explicitly when precision is insufficient. Do not widen tolerance merely to make the independent parser agree. API presence and package integrity are mechanism evidence, not a passing IFC import.

## Implementation and evidence

Implement an optional Node verifier under `tests/backend/ifc-reader/` with its own package manifest/lock, script, small curated IFC/expected fixtures and README. Keep it outside core-test dependencies. Inputs are explicit bounded artifact/expected-fixture files, not arbitrary source code, executable names or IFC editing operations. It emits one bounded JSON observation/error report; it cannot publish, approve or change accepted runs.

1. Verify the package/JS/WASM identities, Node version and parser version, then open actual A11/A12 outputs with the independent engine. Do not call IfcOpenShell from the verifier or trust its validation report/worker summary as expected geometry.
2. Assert IFC4, one active metre-unit project, exact Project/Site/Building/Storey/Proxy counts and relationships, unique stable root GUIDs, correct schematic proxy type, DRAFT/source properties and expected storey placements. Reject missing active units even if geometry looks metre-scaled.
3. Apply actual per-instance transforms once, resolve axis/scaling convention from independent asymmetric probes, and compare transformed vertices/bounds/heights with canonical SI fixtures. Check winding/closed edges and signed per-solid volumes; compare cap/projected footprint area and concave L preservation, not only bounding boxes. State the supported schematic subset and reject unsupported geometry rather than silently replacing it with a hull.
4. Run analytic rectangle (200 m², three storeys, 600 m² GFA, 9.6 m height) and L (75 m², 225 m² GFA), translated `[10,-5]`, nonzero north metadata and repeated exports. Derive expected values from hand-checked/core fixtures, not from the exported IFC's custom quantity claims.
5. Exercise missing/wrong active units, wrong placement/height, filled L notch, missing/extra proxy, corrupt/truncated model, malformed indices, duplicate GUID and modified DRAFT/quantity properties. Failed fixtures must produce nonzero status with actionable evidence and no output mutation.
6. Record exact source/IFC/expected/package hashes, command/exit, graph and mesh observations, transform policy and comparison tolerances. Run synthetic math/parser-shape tests plus real engine fixtures; an import without measured observations is insufficient. Obtain independent Sol review.

Current prerequisites are verified package bytes plus existing WSL exporter outputs. Exact parser coordinate layout/behavior, Node initialization/import, memory/disposal behavior and real fixture results are unverified. A09 sandbox authority, A05 immutable publication, actual Omarchy interoperability and human visual review remain separate open gates; second-engine WSL measurements do not close target acceptance.
