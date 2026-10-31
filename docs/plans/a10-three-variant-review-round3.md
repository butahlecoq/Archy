# A10 final scoped fixture rereview

**PASS for the WSL developer fixture and the stated final-component evidence-root guard.** No verified defect in this scope. Full A10 production rendering, publication and Omarchy acceptance remain open. The selected CLI and Blender executables must be trusted and run with the invoking developer's permissions; this fixture does not sandbox them.

## Frozen identities

Worktree `/home/butahlecoq/work/archy/.worktrees/a10-three-variant-proof`, branch `codex/a10-three-variant-proof`, HEAD `8e76c4af76b5c4cc834bbfe16e1140f4a91695db`. The additions below are untracked; HEAD alone does not identify them.

| File or evidence | SHA-256 |
| --- | --- |
| `docs/plans/a10-three-variant-handoff.md` | `85ab678f3195226b1cdf539ee61134f2ca761ab17e3bccc116b8a15372938d01` |
| `tests/backend/blender/three_variant_proof.py` | `27d7d7532bad59ecf58e9e472525d338cf27cb0d5f24d4bb3b0ba580c61c616a` |
| `tests/backend/blender/test_a10_paths.py` | `8830942165578aa8d3e508bfd4ca792feaafdf24cb3aa81c53b599639a1b9fb2` |
| Slot 9 source manifest | `3826dd2b93dea2bd20a5eb09bc63451c126bfcff25945fbe30b2f063eec1068c` |
| Slot 9 artifact manifest | `378f8d59c0ba4de8be05b0d78bc36d3d07b7c78e5edbe6e3aa9dbb69df581f8a` |
| Slot 9 `proof.json` | `41c07f1d747007825dddc0332d48fc22ecb0aac60765f58cbe35e667184ba488` |
| Current checkout CLI executable | `a84886871501ef034aa34ebe78c92739cf6a1ca6909a8ea2025fc6eed3b298f4` |

Slot 9 is `/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof/slot-9`. Every one of its seven source entries matched the current worktree and retained source snapshot; all 69 artifact entries matched size and hash. Source snapshots are covered by the separate source manifest, which is created after the artifact inventory. These manifests are development inventories, not signed provenance or a complete Rust source/build identity. The author reports the CLI hash in the handoff; the manifest does not bind the executable to the complete build source.

I verified each retained slot 3–8 artifact inventory and its own source snapshot, preserving their historical source identities. All matched; their artifact manifest hashes were, in order: `2d88e9566f0e8fad3feed37e6bfe8e88586070481769262f279df32a7f1217e4`, `e516b1a08beff6604aa6f20af7c6dcd94b6ffca128171c89775cd9ea631bf7ee`, `2b7b60cf775f7bae04f5ec3aa7e26e51c45271e30456314f8e40ef63f76f7304`, `afaeb244d44a0738934867a119767f7d0403023b30045ae7308c65bbee86a63f`, `5175d28ffc719e04835780dafa9d8c86f3e6e4e2f3df9fdb74a1d8322b8786c3`, and `4725a31ea6dbc45377317d7afcf5f0c44fc45d803945b521abc287309c3d3995`.

## Destination and documentation

The handoff now states next to the overrides that selected CLI and Blender executables must be trusted, execute with developer permissions and are not sandboxed. It distinguishes this fixture from product runtime. Its earlier round-two routing request and pending-status wording are historical; this document records the subsequent rereview.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/backend/blender -p test_a10_paths.py -v
```

Exit 0: six tests passed, covering ordinary/linked checkout resolution, local tools preference, valid and invalid overrides, real/new evidence directories, occupied slots, final-slot symlinks, and evidence-root symlinks including dangling links. The positive and negative checks use disposable directories and sentinels.

I separately invoked the real harness with a temporary evidence-root symlink to a directory containing an `unchanged` sentinel: exit 1, stderr contained `evidence root must not be a symlink`, and the target still contained only the sentinel. A dangling final root symlink also exited 1 with the same guard and did not create its target. A repeat actual `--slot 9` invocation exited 1 with `FileExistsError`; hashes of every file under slots 3–9 before and after were identical. These checks protect the final root component and final slot entry in the observed single-process scenarios. Parent-component symlinks and concurrent replacement are not a race-free sandbox, as the handoff states.

## Slot 9 real artifacts

Slot 9 records real CLI generation `41b113a5-6caf-4293-a399-a3425601357a` and evaluation `2b6e46e3-6bf6-4b04-83b3-8c74ce2a52dc`, both exit 0, three distinct saved L variants and three passing fixture render results. The Blender version log reports 4.5.15 LTS build `5afeabba6097`. Each request matched its generated variant and digest; each saved evaluation's checks matched the proof; each worker exited 0 and its five response artifact hashes and lengths matched disk.

I independently loaded each GLB with trimesh, applied all three node transforms and converted the world vertices to Blender axes. Bounds and volumes matched the proof, and rerunning the frozen source's geometry reader against the saved evaluations matched its geometry results. All nine PNGs passed Pillow structural verification, decoded to 512 × 512, and had the foreground counts recorded in the proof. Direct image inspection of variant 0 top and variants 1 and 2 iso showed visible, framed L masses with notches. These are CPU smoke and geometry observations, not pixel-equivalence or target-GPU claims.

| Variant ID suffix | GLB SHA-256 | World bounds `[xmin,ymin,zmin,xmax,ymax,zmax]` m | Volume m³ | PNG foreground pixels top/front/iso |
| --- | --- | --- | ---: | --- |
| `3902dc3b` | `8711ea166746fceda41dfb4016bf1586bd2a6104df06262c39789aef82b96cde` | `[5.052881876, 6.268478967, 0, 13.472765446, 31.309940815, 9.600000143]` | 1842.245417331 | 34526 / 43904 / 41296 |
| `8594779d` | `0e80a334dbd3cfe11c44f50e83ff44e81cf5cce1aa509ffe565dabb49700de32` | `[5.519766686, 4.592456390, 0, 15.282759666, 31.666032314, 9.600000143]` | 2216.629128770 | 41444 / 50920 / 46450 |
| `efd505ff` | `4900ad5a02e41ea858a72ce8aa2008b274cd49fac22b3deec4d1666879a5d817` | `[4.281063549, 5.620833773, 0, 11.657241344, 30.072162151, 9.600000143]` | 1307.895079699 | 24819 / 38528 / 34417 |

Slot 9 is the three-success case; injected collision evidence is in earlier slot 7 and the independent slot 8 rerun, as recorded in round 2. No fresh full rendering run was needed for this review; all numbered slots were already occupied.

## Reproducible manifest and geometry checks

From the worktree with the existing pinned reader environment:

```sh
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python - <<'PY'
from pathlib import Path
import hashlib, importlib.util, json
import numpy as np, trimesh
from PIL import Image
root = Path.cwd()
base = Path('/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof')
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
for n in range(3, 10):
    slot = base / f'slot-{n}'
    for kind in ('source', 'artifact'):
        manifest = slot / f'{kind}-manifest.tsv'
        rows = manifest.read_text().splitlines()[1:]
        for row in rows:
            name, size, digest = row.split('\t')
            parents = ([slot / 'source-snapshot', root] if n == 9 else [slot / 'source-snapshot']) if kind == 'source' else [slot]
            for parent in parents:
                p = parent / name
                assert p.is_file() and p.stat().st_size == int(size) and sha(p) == digest, p
        print(n, kind, len(rows), sha(manifest), 'OK')
slot = base / 'slot-9'
proof = json.loads((slot / 'proof.json').read_text())
gen = slot / 'project/runs' / proof['generation_run']
eva = slot / 'project/runs' / proof['evaluation_run']
variants = json.loads((gen / 'generation.json').read_text())['variants']
assert len(variants) == len({v['id'] for v in variants}) == 3
assert (slot / 'generate.exit').read_text().strip() == (slot / 'evaluate.exit').read_text().strip() == '0'
spec = importlib.util.spec_from_file_location('proof', root / 'tests/backend/blender/three_variant_proof.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
for i, (variant, item) in enumerate(zip(variants, proof['variants'])):
    job = slot / f'variant-{i}'
    saved = gen / 'variants' / (variant['id'] + '.json')
    report = json.loads((eva / 'evaluations' / (variant['id'] + '.json')).read_text())
    request = json.loads((job / 'request.json').read_text())
    response = json.loads((job / 'response.json').read_text())
    assert request['params'] == variant == json.loads(saved.read_text()) and request['input_digest'] == sha(saved)
    assert report['variant_id'] == variant['id'] and report['checks'] == item['design_checks']
    assert (job / 'worker.exit').read_text().strip() == '0' and response['status'] == 'ok'
    assert len(response['artifacts']) == 5
    for artifact in response['artifacts']:
        path = job / artifact['path']
        assert path.stat().st_size == artifact['size_bytes'] and sha(path) == artifact['sha256']
    scene = trimesh.load(job / 'model.glb', force='scene', process=False, skip_materials=True)
    vertices, volume = [], 0.0
    for node in scene.graph.nodes_geometry:
        transform, name = scene.graph[node]
        mesh = scene.geometry[name].copy()
        mesh.apply_transform(transform)
        a = np.asarray(mesh.vertices)
        vertices.append(np.column_stack((a[:, 0], -a[:, 2], a[:, 1])))
        volume += float(mesh.volume)
    xyz = np.concatenate(vertices)
    bounds = np.r_[xyz.min(axis=0), xyz.max(axis=0)]
    assert len(vertices) == 3 and np.allclose(bounds, item['geometry']['bounds_m'], atol=1e-12, rtol=0)
    assert module.glb_geometry(job / 'model.glb', variant, report) == item['geometry']
    for view in ('top', 'front', 'iso'):
        path = job / f'view-{view}.png'
        with Image.open(path) as image: image.verify()
        with Image.open(path) as image:
            image.load()
            assert image.format == 'PNG' and image.size == (512, 512)
            pixels = np.asarray(image.convert('RGB'), dtype=np.int16)
            count = int((np.max(np.abs(pixels - pixels[0, 0]), axis=2) > 8).sum())
            assert count == item['png_foreground_pixels'][view]
    print(i, variant['id'][-8:], bounds.tolist(), volume, 'GLB and three PNGs OK')
PY
```
