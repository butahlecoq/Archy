# A10 independent rereview: portability and slot 7

**Verdict: ISSUES for the requested caller-safety boundary; PASS for the scoped WSL fixture evidence and ordinary/linked checkout path correction.** A caller that controls the documented executable override can cause arbitrary code to run with the harness user's permissions, including writes outside the evidence directory. The harness is a developer fixture and the finding does not establish a production vulnerability. The stricter requested property that a caller cannot overwrite or delete arbitrary data is not satisfied when those overrides are untrusted.

## Finding: executable override has unrestricted filesystem effects

`ARCHY_A10_CLI` accepts any nonempty absolute path, checks only `is_file()`, and then executes that file as the CLI with the current user's permissions. `ARCHY_A10_TOOLS_DIR` similarly selects an executable named `blender-4.5.15-linux-x64/blender`. The occupied-slot check protects existing slot paths from the harness's own setup; it cannot constrain an override executable. Path overrides are documented for unusual **test** layouts, so this matters if a caller or untrusted environment may set them. They must be treated as trusted code selection unless that boundary is changed. It is not a claim that the default pinned CLI is malicious or that this fixture is the production supervisor.

Exact controlled reproduction, from the review worktree, writes only inside a disposable directory:

```sh
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python - <<'PY'
from pathlib import Path
import json, os, subprocess, tempfile
script = Path('tests/backend/blender/three_variant_proof.py').resolve()
with tempfile.TemporaryDirectory(prefix='a10-override-review-') as tmp:
    root = Path(tmp)
    sentinel = root / 'sentinel'
    sentinel.write_text('original\n')
    fake = root / 'fake-cli'
    fake.write_text('#!/bin/sh\nprintf altered > "$A10_TEST_SENTINEL"\nexit 1\n')
    fake.chmod(0o700)
    env = os.environ.copy()
    env.update(ARCHY_A10_EVIDENCE_DIR=str(root / 'evidence'),
               ARCHY_A10_CLI=str(fake), A10_TEST_SENTINEL=str(sentinel),
               PYTHONDONTWRITEBYTECODE='1')
    result = subprocess.run(['/home/butahlecoq/work/archy/.tools/reader-venv/bin/python',
                             str(script), '--slot', '0'], env=env,
                            capture_output=True, text=True, timeout=30)
    print('exit', result.returncode, 'sentinel', repr(sentinel.read_text()),
          'expected', repr('original\n'))
    print('workflow_error', json.loads((root / 'evidence/slot-0/proof.json').read_text()).get('workflow_error'))
PY
```

Actual: `exit 1 sentinel 'altered' expected 'original\n'`; `workflow_error AssertionError: CLI generation failed`. Expected under the requested caller-safety property: the outside sentinel remains `original\n`. The exit status reports generation failure only after the selected executable has already modified the file. A trusted-operator-only fixture can document that trust boundary; meeting the stronger property requires constraining executable selection and its filesystem authority, with a regression check. This review makes no product edits.

## Source and evidence identity

Worktree `/home/butahlecoq/work/archy/.worktrees/a10-three-variant-proof`, branch `codex/a10-three-variant-proof`, HEAD `8e76c4af76b5c4cc834bbfe16e1140f4a91695db`. The updated untracked proof script hashes to `569b2e091e15360752002c599105003f5f6b09af440d65a507fb408c591eec1d`, the path tests to `cca977806012c9c522e2cdea904fd350266fca9d797eb8b9ecb1f1d73f4073cb`, and the handoff to `65473f07cf4cbbc2717e5870866656b723faf0ffafc567b37d38bb91534a7a16`.

All seven slot 7 source entries match both the current worktree and `source-snapshot`; all 64 artifact entries match their sizes and hashes. The source manifest SHA-256 is `10b8ae37c4c327041c87f35ec02653d1da049bda04ee6ca29b671145e99c6112`; artifact manifest `5175d28ffc719e04835780dafa9d8c86f3e6e4e2f3df9fdb74a1d8322b8786c3`; `proof.json` `da1b2ca850e67e64ba1a411881b36e3d38abbec506bbb6ccc097be7d2ceeab70`. The artifact manifest precedes and does not include the source snapshots or source manifest; the latter covers the seven source snapshots separately. The manifest is an inventory, not an attestation of a complete Rust build: `source_revision` is HEAD and excludes untracked changes, and the CLI binary is not bound into the source manifest. These are the same provenance limits noted in the first review.

Slots 3–5 were checked against their **own** earlier source snapshots and artifact inventories, not against the corrected harness. All entries matched and their artifact manifest hashes remained respectively `2d88e9566f0e8fad3feed37e6bfe8e88586070481769262f279df32a7f1217e4`, `e516b1a08beff6604aa6f20af7c6dcd94b6ffca128171c89775cd9ea631bf7ee`, and `2b7b60cf775f7bae04f5ec3aa7e26e51c45271e30456314f8e40ef63f76f7304`.

## Path and occupied-slot checks

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/backend/blender -p test_a10_paths.py -v
```

Exit 0, four tests passed. Direct read-only `discover_fixture_paths` on `/home/butahlecoq/work/archy/repository` and this linked worktree selected common Git directory `/home/butahlecoq/work/archy/repository/.git`, Blender `/home/butahlecoq/work/archy/.tools/blender-4.5.15-linux-x64/blender`, and evidence `/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof`; each selected its own `target/debug/archy`. Both selected executables exist. No workflow was run in the ordinary checkout. Tests cover local/sibling tools precedence, explicit absolute paths and rejection of empty/relative overrides; they do not establish sandboxing of the selected executables.

The following actual occupied-slot probe exited 1 with `FileExistsError`; hashes of every file under slots 3, 4, 5 and 7 were identical before and after. A separate owned-temporary-directory probe placed `slot-0` as a symlink to a directory containing a sentinel; the same refusal left the sentinel and target directory unchanged.

```sh
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python - <<'PY'
from pathlib import Path
import hashlib, os, subprocess, tempfile
base = Path('/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof')
script = Path('tests/backend/blender/three_variant_proof.py').resolve()
py = '/home/butahlecoq/work/archy/.tools/reader-venv/bin/python'
def snapshot(d):
    return {str(p.relative_to(d)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in d.rglob('*') if p.is_file()}
env = os.environ.copy()
for key in ('ARCHY_A10_TOOLS_DIR', 'ARCHY_A10_CLI', 'ARCHY_A10_EVIDENCE_DIR'):
    env.pop(key, None)
before = {n: snapshot(base / f'slot-{n}') for n in (3, 4, 5, 7)}
r = subprocess.run([py, str(script), '--slot', '7', '--failure-index', '1'],
                   env=env, capture_output=True, text=True, timeout=30)
print('occupied slot exit', r.returncode, 'FileExistsError', 'FileExistsError' in r.stderr,
      'unchanged', all(before[n] == snapshot(base / f'slot-{n}') for n in before))
with tempfile.TemporaryDirectory(prefix='a10-slot-review-') as tmp:
    root = Path(tmp)
    (root / 'outside').mkdir()
    sentinel = root / 'outside/sentinel'
    sentinel.write_text('original')
    (root / 'evidence').mkdir()
    (root / 'evidence/slot-0').symlink_to(root / 'outside', target_is_directory=True)
    env['ARCHY_A10_EVIDENCE_DIR'] = str(root / 'evidence')
    r = subprocess.run([py, str(script), '--slot', '0'], env=env,
                       capture_output=True, text=True, timeout=30)
    print('symlink slot exit', r.returncode, 'FileExistsError', 'FileExistsError' in r.stderr,
          'sentinel', repr(sentinel.read_text()),
          'outside_files', sorted(p.name for p in (root / 'outside').iterdir()))
PY
```

Actual: `occupied slot exit 1 FileExistsError True unchanged True`; `symlink slot exit 1 FileExistsError True sentinel 'original' outside_files ['sentinel']`. This tests existing entries, not concurrent filesystem races or an untrusted executable override.

## Slot 7 actual evidence and rerun

Slot 7 contains fresh generation `d1c4b5e1-752c-44b2-bb94-6a78ae53ec75` and evaluation `eb971add-6f8d-426c-8b5a-3fa1043559b0`; both recorded CLI exits are 0 and generation has three distinct L variants. Each retained worker request equals its saved variant and hashes that variant; each proof design-check list equals the associated saved evaluation. Two workers exited 0 with five response artifacts each, and their artifact hashes and lengths match disk. Blender version output records 4.5.15 LTS.

I independently loaded the **two successful** slot 7 GLBs with trimesh, applied each of the three node transforms, converted axes, derived bounds from vertices and summed volumes. The results match the retained proof and rerunning `glb_geometry` against saved evaluations. Variant `3902dc3b` bounds are `[5.052881876, 6.268478967, 0, 13.472765446, 31.309940815, 9.600000143]` m and volume `1842.245417331` m³; `efd505ff` bounds are `[4.281063549, 5.620833773, 0, 11.657241344, 30.072162151, 9.600000143]` m and volume `1307.895079699` m³. All six corresponding PNGs decode at 512 × 512 and their foreground counts match the proof. The middle variant `8594779d` deliberately has no GLB/PNGs in this slot: its worker exited 20 on a preplaced sentinel, left the sentinel bytes unchanged and made no response. All three evaluations and both other results remain readable. Slot 3 supplies three successful GLBs and nine PNGs under the earlier frozen source, as reviewed in round 1.

This command repeats the slot 7 manifest, snapshot, request, result, independent vertex and image checks:

```sh
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python - <<'PY'
from pathlib import Path
import hashlib, importlib.util, json
import numpy as np, trimesh
from PIL import Image
root = Path.cwd()
slot = Path('/home/butahlecoq/work/archy/.tools/targets/a10-three-variant-proof/slot-7')
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
for kind in ('source', 'artifact'):
    manifest = slot / f'{kind}-manifest.tsv'
    rows = manifest.read_text().splitlines()[1:]
    for row in rows:
        name, size, digest = row.split('\t')
        parents = [root, slot / 'source-snapshot'] if kind == 'source' else [slot]
        for parent in parents:
            path = parent / name
            assert path.stat().st_size == int(size) and sha(path) == digest, path
    print(kind, len(rows), sha(manifest), 'OK')
proof = json.loads((slot / 'proof.json').read_text())
gen = slot / 'project/runs' / proof['generation_run']
eva = slot / 'project/runs' / proof['evaluation_run']
variants = json.loads((gen / 'generation.json').read_text())['variants']
assert len(variants) == len({v['id'] for v in variants}) == 3
assert (slot / 'generate.exit').read_text().strip() == '0'
assert (slot / 'evaluate.exit').read_text().strip() == '0'
spec = importlib.util.spec_from_file_location('proof', root / 'tests/backend/blender/three_variant_proof.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
for i, (v, item) in enumerate(zip(variants, proof['variants'])):
    job = slot / f'variant-{i}'
    saved = gen / 'variants' / (v['id'] + '.json')
    report = json.loads((eva / 'evaluations' / (v['id'] + '.json')).read_text())
    req = json.loads((job / 'request.json').read_text())
    assert req['params'] == v and req['input_digest'] == sha(saved)
    assert report['variant_id'] == v['id'] and report['checks'] == item['design_checks']
    if i == 1:
        assert (job / 'worker.exit').read_text().strip() == '20'
        assert b'worker output collision' in (job / 'worker.stderr').read_bytes()
        assert (job / 'model.glb').read_bytes() == b'a10-existing-output-sentinel\n'
        assert not (job / 'response.json').exists()
        print(i, 'collision preserved; evaluation readable')
        continue
    response = json.loads((job / 'response.json').read_text())
    assert response['status'] == 'ok' and len(response['artifacts']) == 5
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
    assert module.glb_geometry(job / 'model.glb', v, report) == item['geometry']
    for view in ('top', 'front', 'iso'):
        with Image.open(job / f'view-{view}.png') as image:
            image.load()
            assert image.format == 'PNG' and image.size == (512, 512)
            pixels = np.asarray(image.convert('RGB'), dtype=np.int16)
            count = int((np.max(np.abs(pixels - pixels[0, 0]), axis=2) > 8).sum())
            assert count == item['png_foreground_pixels'][view]
    print(i, 'instances', len(vertices), 'bounds', bounds.tolist(), 'volume', volume, 'PNGs OK')
PY
```

I also ran the corrected harness in a new slot under an external fixed timeout:

```sh
timeout 240s env PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/reader-venv/bin/python tests/backend/blender/three_variant_proof.py --slot 8 --failure-index 1
```

Exit 0; `all_fixture_checks_passed: true`, statuses `fixture_verified`, `failed_collision_preserved`, `fixture_verified`, with three readable evaluations. Slot 8 generation/evaluation IDs are `60f0dafe-4f51-44c2-8fef-b1b8b725beb6` and `2cd7ec90-0a44-4d98-8d8e-7d30f3e02f20`. Its source manifest hashes to `10b8ae37c4c327041c87f35ec02653d1da049bda04ee6ca29b671145e99c6112`, artifact manifest to `4725a31ea6dbc45377317d7afcf5f0c44fc45d803945b521abc287309c3d3995`, and proof to `247a3dacee760f4753deb173469c2792b2c1b04241d30bcb0c1ef9381e07a95b`; all listed slot 8 source and artifact files matched. Slots 3–5 and 7 were preserved.

This is WSL CPU fixture evidence. A synthetic collision before response creation does not establish preservation of published artifacts, timeout/cancellation in a production supervisor, atomic render publication, GPU behavior, or Omarchy acceptance.

## Disposition after scope clarification

The caller-safety verdict at the beginning of this review is superseded. The executable-override reproduction remains valid, but a developer deliberately selecting trusted CLI/Blender code is within the intended fixture boundary; it is not a fixture vulnerability under that boundary. Selected executables run with the invoking developer's permissions, and this harness does not sandbox them. A09/A16's production untrusted-agent boundary remains separate.

**Documentation issue:** the handoff's portability section identifies the three variables as test-only overrides, but does not explicitly tell the operator that `ARCHY_A10_CLI` and the Blender executable selected by `ARCHY_A10_TOOLS_DIR` must be trusted and execute with the operator's permissions. The review above states that fact; the fixture handoff should state it alongside the override instructions. No source or handoff edit was made in this rereview.

**Destination scope:** the earlier occupied-directory and final-slot-symlink probes still pass. An additional owned-temporary-directory probe set `ARCHY_A10_EVIDENCE_DIR` to a symlink pointing at another temporary directory, with `/bin/false` as the trusted CLI. The harness followed the evidence-root symlink and created `slot-0` in its target before the CLI failed (`symlink evidence root accepted: True`, exit 1). Therefore refusal applies to an existing final `slot-N` entry; it does not reject symlinks in the evidence-root path. If refusal of symlinked destination ancestors is required, that remains a separate destination-handling issue. The probe did not overwrite an existing slot or file.
