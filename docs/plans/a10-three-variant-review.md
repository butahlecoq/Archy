# A10 three-variant independent review

**Scoped verdict: PASS for the retained WSL CPU fixture. Full A10 acceptance remains open.** No run-proven defect was found in the stated fixture evidence. The production render supervisor, artifact publication and target Omarchy run have not been demonstrated.

## Source and evidence identity

Reviewed worktree `/home/butahlecoq/work/archy/.worktrees/a10-three-variant-proof`, branch `codex/a10-three-variant-proof`, HEAD `8e76c4af76b5c4cc834bbfe16e1140f4a91695db`; the proof script and handoff were untracked additions. `tests/backend/blender/three_variant_proof.py` SHA-256 `98efeb1a325a46001ebbe4254dbccb513983c01fd30e5dc1c4bbfb60ee7f57f5`; handoff SHA-256 `9a1fa033208b69858542aa6a16840200903a47fca63d995dbddf2d1b1b878559`. The current CLI executable hash is `a84886871501ef034aa34ebe78c92739cf6a1ca6909a8ea2025fc6eed3b298f4`, matching the handoff.

The evidence root is `/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof`. Both slots' source manifests hash to `7a05db81a8c8c8a82d443aa4a0b27711386044f273886a90819fae27d7c2b759`. Every one of their six entries matches both the current worktree and its retained source snapshot. The manifest covers the harness, three Blender worker files, input brief and Cargo lock; it does not inventory the whole Rust source tree. The `source_revision` field is the harness's `git rev-parse HEAD` value, not a hash of untracked files or an attestation that the executable was built from that revision. Snapshots are taken after the work; the CLI hash is reported in the handoff rather than bound by the slot manifests. These identities support review of this development evidence, not a complete reproducible build provenance claim.

| Evidence | Artifact manifest SHA-256 | Listed files | `proof.json` SHA-256 |
| --- | --- | ---: | --- |
| slot 3, three successful variants | `2d88e9566f0e8fad3feed37e6bfe8e88586070481769262f279df32a7f1217e4` | 69 | `b94641cd4453bda49dcf93ca547d856b83da52f0e9ee7aae21abddc795d3ff98` |
| slot 4, injected failure | `e516b1a08beff6604aa6f20af7c6dcd94b6ffca128171c89775cd9ea631bf7ee` | 64 | `a4bda4ff82e082b6e6fb37ff3dabde37adcb88775b64246bda34e6ceaf1bc48b` |
| slot 5, independent rerun | `2b7b60cf775f7bae04f5ec3aa7e26e51c45271e30456314f8e40ef63f76f7304` | 64 | `201c4e4c7cef6929f8c84fd51f00b60cd274a3443ebb65fd480d44bed77c4d9c` |

Every listed file matched its size and hash. The artifact inventories are written before the source snapshots and source manifest: those seven files are independently covered by the source manifest, not by the artifact manifest. Neither manifest lists itself. Manifest hashes identify inventories; they are not signed provenance.

## Independent checks

Slot 3 has distinct generation/evaluation run IDs `99051459-1d62-4237-a8f3-64ec56ac07bc` and `0e6e42f9-deb9-48a5-8b41-aa540bff873f`. Slot 4 used separate runs `dbc726b5-29b8-4259-9be9-c674f24216b2` and `dc6b6de6-c5c5-45b2-b17b-b0ec0095773e`. In both slots I checked all three worker request parameters against the actual generation snapshot, the request input digest against its variant file, and proof design checks against the corresponding saved evaluation. Seeded IDs agree across runs, as expected for the same deterministic inputs; the separate projects, run IDs, logs and outputs demonstrate separate executions, not reused slot 3 files.

The reader applies each trimesh scene instance's transform to its mesh vertices, then converts glTF axes to Blender world axes. Its measured bounds come from those vertices; it compares them with core evaluation metrics and the source footprint, checks each storey elevation and corner, welded solid closure/winding/positive volume, roof area and triangle centroids, and total GFA. The 1e-6 m coordinate budget is read from each evaluation and checked exactly; area and GFA use the recorded dimensioned budgets. This is a fixture-specific geometry check, not a general glTF conformance validator. All three seed-42 alternatives are L shapes; the pre-existing rectangle test was not rerun here.

Independently loading the slot 3 GLBs with trimesh and transforming every instance reproduced the recorded bounds and summed volumes:

| Variant suffix | Instances | World bounds `[xmin,ymin,zmin,xmax,ymax,zmax]` m | Volume m³ |
| --- | ---: | --- | ---: |
| `3902dc3b` | 3 | `[5.052881876, 6.268478967, 0, 13.472765446, 31.309940815, 9.600000143]` | 1842.245417331 |
| `8594779d` | 3 | `[5.519766686, 4.592456390, 0, 15.282759666, 31.666032314, 9.600000143]` | 2216.629128770 |
| `efd505ff` | 3 | `[4.281063549, 5.620833773, 0, 11.657241344, 30.072162151, 9.600000143]` | 1307.895079699 |

For variant 0 I created a disposable GLB copy with all three position accessor `min` and `max` values replaced by `[0,0,0]`. The proof reader still returned identical geometry and nonzero measured bounds. In a second disposable copy, shifting the first storey node 1 m along X caused `AssertionError: world vertex outside core footprint`. This tests the trust boundary in the actual reader; the original GLB and retained evidence were untouched.

I independently verified and decoded all nine slot 3 PNGs: each was 512 × 512 RGBA, and per-view changed-pixel counts matched `proof.json`. I visually inspected variant 0 top, variant 1 iso and variant 2 front: they show a visible mass; the L notch is visible in the top and iso examples, and the front is a solid silhouette. Pixel variation is a smoke check, not a proof of geometry or a cross-GPU tolerance. The site is not drawn.

Slot 4's middle worker actually exited 20 with `worker output collision` in stderr, left `model.glb` containing the preplaced `a10-existing-output-sentinel\n`, and created no response. Variant 0 succeeded before it and variant 2 succeeded after it; their artifacts and all three independently saved evaluations remain readable. This exercises a worker boundary collision, with a synthetic sentinel, not preservation of an already published GLB or a mid-render failure. Production failure isolation and atomic acceptance still require A09.

## Commands and results

From the worktree, the pinned Blender path and existing CPython reader environment were used. The independent rerun wrote a new slot and preserved slots 3 and 4:

```sh
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python tests/backend/blender/three_variant_proof.py --slot 5 --failure-index 1
```

Exit 0; `all_fixture_checks_passed: true`, with render statuses `fixture_verified`, `failed_collision_preserved`, `fixture_verified` and three readable passing design evaluations. Slot 5 generation/evaluation IDs are `9a8ad40f-2583-4dce-bb59-696408147403` and `ba55cfb7-94ab-40af-a5e9-a78025be3609`.

The manifest and source snapshot check can be repeated from the worktree as follows:

```sh
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python - <<'PY'
from pathlib import Path
import hashlib
root = Path.cwd()
base = Path('/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof')
for n in (3, 4, 5):
    slot = base / f'slot-{n}'
    for kind in ('artifact', 'source'):
        manifest = slot / f'{kind}-manifest.tsv'
        rows = manifest.read_text().splitlines()[1:]
        bad = []
        for row in rows:
            name, size, digest = row.split('\t')
            roots = [slot] if kind == 'artifact' else [root, slot / 'source-snapshot']
            for parent in roots:
                path = parent / name
                if not path.is_file() or path.stat().st_size != int(size) or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                    bad.append(str(path))
        print(n, kind, len(rows), hashlib.sha256(manifest.read_bytes()).hexdigest(), bad)
PY
```

All `bad` lists were empty. The following command independently reproduces the all-instance bounds and the accessor-metadata and shifted-transform probes:

```sh
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python - <<'PY'
from pathlib import Path
import importlib.util, json, struct, tempfile
import numpy as np
import trimesh
slot = Path('/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof/slot-3')
proof = json.loads((slot / 'proof.json').read_text())
spec = importlib.util.spec_from_file_location('proof', 'tests/backend/blender/three_variant_proof.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
for i, item in enumerate(proof['variants']):
    variant = json.loads((slot / 'project/runs' / proof['generation_run'] / 'variants' / (item['variant_id'] + '.json')).read_text())
    report = json.loads((slot / 'project/runs' / proof['evaluation_run'] / 'evaluations' / (item['variant_id'] + '.json')).read_text())
    path = slot / f'variant-{i}/model.glb'
    scene = trimesh.load(path, force='scene', process=False, skip_materials=True)
    vertices, volumes = [], []
    for node in scene.graph.nodes_geometry:
        transform, name = scene.graph[node]
        mesh = scene.geometry[name].copy()
        mesh.apply_transform(transform)
        a = np.asarray(mesh.vertices)
        vertices.append(np.column_stack((a[:, 0], -a[:, 2], a[:, 1])))
        volumes.append(float(mesh.volume))
    all_vertices = np.concatenate(vertices)
    bounds = np.r_[all_vertices.min(axis=0), all_vertices.max(axis=0)]
    measured = module.glb_geometry(path, variant, report)
    assert np.allclose(bounds, item['geometry']['bounds_m'], rtol=0, atol=1e-12)
    assert measured == item['geometry']
    print(i, 'instances', len(vertices), 'bounds', bounds.tolist(), 'volume', sum(volumes))
    if i == 0:
        raw = path.read_bytes()
        n = struct.unpack_from('<I', raw, 12)[0]
        doc = json.loads(raw[20:20+n])
        for accessor in doc['accessors']:
            if 'min' in accessor and 'max' in accessor and len(accessor['min']) == 3:
                accessor['min'] = accessor['max'] = [0, 0, 0]
        def write_copy(target):
            data = json.dumps(doc, separators=(',', ':')).encode()
            assert len(data) <= n
            target.write_bytes(raw[:20] + data + b' ' * (n-len(data)) + raw[20+n:])
        with tempfile.TemporaryDirectory(prefix='a10-review-') as tmp:
            altered = Path(tmp) / 'metadata.glb'
            write_copy(altered)
            assert module.glb_geometry(altered, variant, report) == measured
            print('false accessor bounds: geometry unchanged')
            doc['nodes'][0]['translation'][0] += 1.0
            write_copy(altered)
            try:
                module.glb_geometry(altered, variant, report)
            except AssertionError as error:
                print('shifted storey rejected:', error)
            else:
                raise AssertionError('shifted storey unexpectedly passed')
PY
```

Actual probe output included `false accessor bounds: geometry unchanged` and `shifted storey rejected: world vertex outside core footprint`. The GLB copies were temporary. The retained artifacts and the fixture rerun do not establish target GPU, Omarchy, production supervisor, accepted render manifest, cancellation or atomic publication behavior.
