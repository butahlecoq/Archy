# A13 optional independent IFC reader handoff

Round1 source frozen for Hume rereview on 29 October 2026 after the three findings in [ifc-reader-review-final.md](ifc-reader-review-final.md). Scope is `tests/backend/ifc-reader/**` and this handoff. The independent review file is preserved. No exporter, Rust, Cargo, checker, or production path was changed for this slice. No commit or push was made.

## Implemented

- A separate Node package pins web-ifc `0.0.78` as its sole dependency with registry SHA-512 integrity `sha512-LGKMNYFVNyz9vVERN0iz+zvPQJqT7MEGBxslzcuuhwDDOA116LmJLKkg8sZ5SEvQJSc5NRVMWgpRNkDmuoQoJg==`. Install uses `npm ci --ignore-scripts --no-audit --no-fund`; the runtime checks the Node loader and WASM SHA-256 before parsing. Tested Node is `22.22.2` only.
- `reader.cjs` independently parses IFC4 graph, active SI units, hierarchy, containment, unique root GUIDs, DRAFT/source properties, quantity values and units, placements, and per-proxy transformed meshes. It measures closed winding, complete exterior top-cap outline, cap areas, bounds and positive signed volume. `verify.cjs` reads explicit bounded regular IFC, expected and source inputs, rejects symlinks, emits one JSON report, and returns nonzero on failure. It performs no model or config writes.
- The checked subset is three schematic extruded storeys for the analytic rectangle and concave L. A repeated L export checks GUID stability. Unsupported structure and geometry fail explicitly. Expected areas/GFA/volumes come from the analytic fixture, not the IFC quantity fields or exporter validation summaries. Two compact canonical source files retain the translated analytic values. The reader checks source/expected field agreement, hashes actual bounded source bytes, and compares that digest with both the expected record and IFC source properties. The curated IFC fixtures have their source properties rebound to those retained bytes; the geometry and GUIDs remain from the original exporter outputs.
- The web-ifc 0.0.78 probe established six float32 values per vertex (XYZ then normal), uint32 indices, and column-major `flatTransformation` in the engine's Y-up frame. Applying that transform once and mapping `[x,y,z]` to IFC `[x,-z,y]` yielded the translated IFC coordinates. `GetWorldTransformMatrix` was separately checked for IFC placements and is not applied to the mesh a second time. Mesh streaming requests linear-unit scaling explicitly.

## Exact development evidence

`npm test` exited 0: **23 tests passed, 0 failed, 0 skipped**. For round1, three new regressions first failed with “Missing expected rejection”: changed L inner outline despite unchanged area/bounds, changed source bytes, and matching false IFC/expected digest claims. They now reject. Positive tests imported all three real IFC fixtures with web-ifc. Other negatives cover absent/wrong active units, wrong origin and height, missing/extra proxy, duplicate GUID, changed DRAFT, quantity value and unit, filled notch, corrupt/truncated IFC, invalid index/open mesh, symlink/directory/missing input, oversized or symlinked source, and command input nonmutation. Test subprocesses have 10 s timeouts. No target Omarchy import was run.

Both direct commands exited 0: `node verify.cjs fixtures/rectangle.ifc fixtures/expected.json rectangle.ifc fixtures/source/rectangle.json` and `node verify.cjs fixtures/l_shape.ifc fixtures/expected.json l_shape.ifc fixtures/source/l_shape.json`. Each observed one IFC4 project/site/building, three storeys/proxies, metre/area/volume active units, and 19 unique root GUIDs. Rectangle proxy bounds were approximately `[13,-2,0,23,18,3.2]`, cap area 200 m², signed volume 640.000009536743 m³ per storey, and outline `[[13,18],[13,-2],[23,-2],[23,18]]`. L bounds were approximately `[13,-2,0,23,8,3.2]`, cap area 75 m², signed volume 240.000003576279 m³ per storey, and outline `[[13,-2],[23,-2],[23,3],[18,3],[18,8],[13,8]]`; each L mesh had 20 triangles. The outline comparison accepts a cyclic starting-point shift or reverse winding and requires the same vertices within 1e-6 m. The observed float32 Z drift was below the retained 1e-6 m coordinate budget. Perimeter-derived area budgets were 0.000060000004 m² for rectangle and 0.000040000006 m² for L. No tolerance was widened for the parser.

SHA-256 identities:

| File | SHA-256 |
| --- | --- |
| `reader.cjs` | `333d82dd3df2d7c0659b84192edc814d8f097827e4827d16aebfa5c1a2885039` |
| `verify.cjs` | `b8862aca2cf8ad186ace4f5a80538c9e3174b460440af35989b6a12e4903b043` |
| `package-lock.json` | `70d0bc1419cd2c12d1a30cb6eada91e19640ba291841711ac706914630d84a46` |
| `fixtures/expected.json` | `31d656478293355ef92514bb6b0b2a08e3c474993174b7013142c706c2489dde` |
| `fixtures/rectangle.ifc` | `582291741174224aaecc06359d5a78bc9a21eb8448a037ce5736f46f179f39ef` |
| `fixtures/l_shape.ifc` | `f9152a4b20f95d24a915dbbc5a3e7fc1d736674601ab900cf378d3eee585b9ea` |
| `fixtures/l_shape_repeat.ifc` | `e048ac473d4ea5342e6620cdbf7d458223ad95fa8a772fc64edb5123765ae4d1` |
| `fixtures/source/rectangle.json` | `d7fb108848e058fc1a7ffd2510ac1474fd8a114aeb37c38676e525a0d87ce7e9` |
| `fixtures/source/l_shape.json` | `a82d2d65d379f13219a5b15385385f07f52ab9276f3ea1a1e9cb83882a56690e` |
| `web-ifc-api-node.js` | `592491e48f22ba5dd2f1d33a6485469bed63ac1b5f48547a80b146536556ff6f` |
| `web-ifc-node.wasm` | `36b10f4f9fada12bb89ed664f9e61eae1c82b2ce5f04ee4175205149516498f5` |

## Limits and review boundary

`git check-ignore -v` identified the package-local `.gitignore:1:/node_modules/` for both `web-ifc-node.wasm` and `LICENSE.md`; `git status --short --untracked-files=all -- tests/backend/ifc-reader` listed curated source, fixtures, lock and tests without installed package files. The npm lock remains separate, and the loader/WASM hashes above are still checked at runtime.

The CLI caps input size and the web-ifc model memory setting, but a parent process must enforce wall time, whole-process memory and output bounds for hostile inputs. The tests do not claim an adversarial OS sandbox or absence of every indirect runtime syscall. MPL-2.0 notices and obligations remain attached to the npm package; this repository does not bundle or relicense its loader/WASM. Hume independent rereview is pending at this freeze. A09 sandbox authority, A05 immutable publication, actual Omarchy interoperability and human visual review remain outside this slice.
