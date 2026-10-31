# Independent artifact readers for Blender development tests

Research: 2026-10-29. Recommendation: **Pillow==13.0.0 + trimesh==5.2.0**, with NumPy resolved and hash-locked in a separate development-test environment, outside Blender's bundled Python. These are established OSS projects; trimesh explicitly does not guarantee API stability, so pin and test. No product code, installation, agents, commits or pushes in this task.

## Exact published artifacts and licenses

Published availability was verified through primary PyPI pages; successful local installation is **not** claimed. The shell's PyPI JSON requests failed DNS; web access succeeded. Binary downloads were not supported by the web reader, so hashes below are publisher/PyPI metadata, not independently computed download hashes.

| Package | Current exact version / requirements / license | Selected artifact provenance |
| --- | --- | --- |
| Pillow | **13.0.0**, Python >=3.11, **MIT-CMU**; [metadata/files](https://pypi.org/project/pillow/13.0.0/) | `pillow-13.0.0-cp312-cp312-manylinux_2_27_x86_64.manylinux_2_28_x86_64.whl`; uploaded 2026-10-16; CPython 3.12, Linux x86-64, glibc-compatible wheel; PyPI reports Trusted Publishing and attestation. |
| trimesh | **5.2.0**, Python >=3.10, **MIT**; NumPy is its only hard dependency; [metadata/files](https://pypi.org/project/trimesh/5.2.0/) | `trimesh-5.2.0-py3-none-any.whl`; uploaded 2026-10-24; PyPI reports Trusted Publishing and attestation. No `easy`/`all` extras needed for the proposed plain mesh tests. |
| pygltflib (alternative, not recommended dependency) | **1.16.5**, metadata Python >=3.6, **MIT**; [metadata/files](https://pypi.org/project/pygltflib/1.16.5/) | `pygltflib-1.16.5-py3-none-any.whl`; uploaded 2025-07-24; no Trusted Publishing/provenance reported. Treat the Python floor as metadata, not tested transitive compatibility. |

- Pillow wheel SHA-256: `42d63be3af4f9b951b199b0a9d23365d37ab14687e6d77b74940432c8515c4ca`; [exact download](https://files.pythonhosted.org/packages/e6/99/fdf65cc539e266ab93546c261bdf98411fe165c6e4e13f9dbf597097192b/pillow-13.0.0-cp312-cp312-manylinux_2_27_x86_64.manylinux_2_28_x86_64.whl).
- Pillow attested publishing revision: [293218ad059d994baf1777d882911bed6a49704a / wheels.yml](https://github.com/python-pillow/Pillow/blob/293218ad059d994baf1777d882911bed6a49704a/.github/workflows/wheels.yml).
- trimesh wheel SHA-256: `04db2c7ec62afdd7763fb04c82bb5f257ddd984993a19ee83697f475fc58fd29`; [exact download](https://files.pythonhosted.org/packages/51/14/78624d9c89bab76c2a2e1e839c7641fa686828c2c1c8fb64aedf3b371a5d/trimesh-5.2.0-py3-none-any.whl).
- trimesh attested publishing revision: [6c158cce226500fc64b12acd265af0cf01d1c212 / release.yml](https://github.com/mikedh/trimesh/blob/6c158cce226500fc64b12acd265af0cf01d1c212/.github/workflows/release.yml).
- pygltflib wheel SHA-256: `41d3349c59dcf1586faeaee29c967be07ac2bf7cecdb8ae2b527da1f25afdaac`; exact artifact and digest are on its versioned files page above.
- Before use: download matching wheels, recompute hashes, inspect wheel METADATA/license notices and lock NumPy plus all resolved dependencies/artifacts. NumPy's exact version/hash and bundled-library licenses were not researched here; this is not a complete install lock. Use a different verified wheel for another interpreter/architecture; never reuse the CPython 3.12 wheel hash indiscriminately. Preserve component notices if redistributing wheels.

## PNG: decode pixels, then test the image

[Pillow Image API](https://pillow.readthedocs.io/en/stable/reference/Image.html) distinguishes lazy `Image.open`, structural `verify`, and pixel-decoding `load`. A header/signature or `verify()` alone is insufficient.

1. Bound input bytes and dimensions; require PNG, exact expected resolution and one frame. Keep truncated-image acceptance disabled and turn decompression-bomb warnings into failures. Reject dimensions over the test budget before allocating the decoded image.
2. Run `verify()`, close, reopen and call `load()` to force full decoding; convert explicitly to RGB/RGBA for comparisons. Record original mode; constrain initial render fixtures to 8-bit RGB/RGBA rather than silently reducing precision.
3. For transparent-background fixtures, require a substantial count of alpha-covered pixels and a bounded foreground box. For opaque output, compare RGB against a separately rendered empty-scene baseline from identical settings; alpha alone proves nothing. Use declared pixel-difference/coverage thresholds, not merely “nonzero pixels” or file size.
4. Proposed initial fixtures: 256×256, at least 1% foreground with alpha >16 or maximum channel difference >8/255 against the baseline; calibrate against the fixed camera/preset and record changes. Reject all-transparent, empty-background, truncated and wrong-resolution samples. A solid-color corrupt image must not pass: also check expected foreground extent/variation or a coarse reference silhouette.
5. These checks establish decoded, nonempty render evidence, not correct geometry or photorealism. Avoid byte-identical render expectations across hardware; record render/color-management settings.

## GLB: scene instances, world transforms and geometry

[trimesh GLB loader](https://trimesh.org/trimesh.exchange.gltf.html) supports `ignore_broken=False`, `merge_primitives=False`, and `skip_materials=True`; low-level `load_glb` returns Scene-constructor kwargs. Instantiate a Scene from those kwargs with mesh processing disabled (`process=False`) to avoid silently repairing exporter defects. Use a file-like object and deny external resource resolution; preflight the JSON chunk to reject external buffer/image URIs and unsupported required extensions.

[Scene API](https://trimesh.org/trimesh.scene.scene.html) provides `dump()` copies with instance transforms baked, `to_mesh()` for transformed mesh concatenation, and transformed `triangles`. Iterate transformed instances, not unique `scene.geometry.values()`: the latter loses parent transforms and repeated instances. Before concatenating, assert every expected primitive loaded and reject unexpected non-mesh geometry; `to_mesh()` can discard it. Check the selected/default scene explicitly rather than including unrelated scenes.

[Khronos glTF 2.0 specification](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/Specification.adoc) defines parent×local composition; local TRS is T×R×S, matrices are column-major, quaternions xyzw, and coordinates are right-handed Y-up in metres. Do not transpose or apply the node transform twice. Reject skins, animation, morph targets and compressed geometry in the first test profile; ordinary node transforms are not skinning evaluation.

For the intended Blender Z-up export mapping `(x,y,z) -> (x,z,-y)`, apply inverse rotation **once after world transforms**: `(xg,yg,zg) -> (xg,-zg,yg)` (Rx(+90°)). This is a proposed exporter contract to verify with a translated asymmetric fixture, not something guaranteed merely by the GLB extension. The Blender 4.5 manual fetch was unavailable in this session; inspect the actual export_yup setting and emitted artifact before accepting this mapping. Compute bounds from all transformed vertices after conversion, not from accessor min/max or only two opposite transformed AABB corners.

[Trimesh mesh API](https://trimesh.org/trimesh.base.html) exposes watertightness, winding, volume and transforms. Check finite positions, index ranges, nondegenerate triangles, winding and watertightness before trusting volume. Sum per-solid world-space signed volumes only after checking consistent outward orientation; do not use abs(volume) to conceal flipped faces. Compare scale effects against abs(det(world linear transform)); negative-scale fixtures must explicitly test orientation handling.

GLB TRIANGLES already encodes triangulation; do not add an n-gon triangulator or convex-hull approximation. Require mode 4 (or its default) initially; reject strips/fans/lines/points unless explicitly tested later. Concave L roofs need real concave triangulation: bounds alone cannot catch a filled notch. Material/UV seams can duplicate vertices; if needed, validate a separate topology copy with exact-position welding only, recording that operation, never hole filling or tolerance-based repair of the original evidence. Summed solid volumes are not boolean-union volume; reject overlaps or use known nonoverlapping storey fixtures.

## Minimal test handoff

- Use a fresh Python process outside Blender to decode each actual output; retain file hashes, reader versions, instance/triangle counts, per-solid bounds/volumes and failed findings. This is independent of bpy, not a second full glTF conformance validator.
- Analytic fixtures: off-origin unequal-sided box, concave L prism, stacked storeys, nested parent rotation/translation, nonuniform scale and repeated mesh instances; add mirrored scale, invalid indices, corrupt/truncated buffers and open/inverted meshes as negative cases.
- Compare with core SI geometry: proposed tolerance 1e-5 m for small fixture bounds and max(1e-6 m³, 1e-5×expected volume) for volume; justify using actual float32 exports, never widen silently. Test the L notch by volume and cross-section/silhouette, not just overall bounds.
- pygltflib is useful for structured glTF/accessor inspection, but its documented validator is incomplete; it is not a world-mesh/volume engine. Choosing it instead would require custom accessor decoding, stride/sparse handling, scene traversal and geometry calculations. Do not add it alongside trimesh without a demonstrated structural-check need.
- Next implementation gate: resolve/hash-lock the selected pair plus NumPy, verify imports and run these real-file fixtures on WSL. No installs/tests ran here. Actual Omarchy, backend rendering and visual acceptance remain separately deferred.
