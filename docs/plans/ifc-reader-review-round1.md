# A13 independent IFC reader rereview, round 1

Date: 2026-10-29. **Scoped verdict: PASS.** The three findings in `ifc-reader-review-final.md` are addressed in the frozen Node reader slice. This review changed only this document; no implementation, install, commit, push or agent action was performed. Carson's separate Rust process module and A09 supervisor authority remain outside this review.

## Finding adjudication

1. **Canonical outline — resolved.** `measureSolid` now extracts the complete exterior top-cap boundary from edges used by one cap triangle, rejects a boundary that is not a single closed loop, and returns measured XY vertices. `sameOutline` compares that loop to the expected closed footprint with a 1e-6 m per-coordinate limit, allowing only a cyclic start or reversed winding. The existing area, bounds, closed-winding, signed-volume and L-notch checks remain. The regression changes the L inner outline from x=18 to x=19 while rebinding the source and IFC digest, and now rejects on `transformed footprint outline mismatch`. Direct real IFC parses report rectangle and L outlines matching the canonical values.
2. **Source binding — resolved for the retained fixture bytes.** `verify.cjs` reads a third, bounded regular source file with `O_NOFOLLOW`. `verifySource` requires canonical JSON bytes, compares each declared geometric/source field with the expected case, hashes the actual source bytes, and requires that digest in both the expected record and the IFC `CanonicalInputSha256`/`SourceArtifacts` declarations. Tests reject modified source bytes and matching false IFC/expected digest claims. These compact retained source fixtures are curated analytic inputs; their binding does not authenticate the historical exporter invocation or immutable A05 storage by itself.
3. **Installed package staging — resolved.** Package-local `.gitignore` contains `/node_modules/`. `git check-ignore -v` identifies that rule for the MPL license and Node WASM; `git status --short --untracked-files=all -- tests/backend/ifc-reader` lists source, fixtures, lock and tests without installed package files. The npm lock retains `web-ifc 0.0.78` and its registry SHA-512 integrity; runtime loader/WASM SHA-256 checks remain in place.

## Independent verification

| Command | Observed result |
| --- | --- |
| `npm test` in `tests/backend/ifc-reader` | Exit 0; 23 tests passed, zero failed/skipped. The three new negative tests and prior real-parse, graph, geometry, malformed-input and nonmutation cases all passed. |
| `node verify.cjs fixtures/rectangle.ifc fixtures/expected.json rectangle.ifc fixtures/source/rectangle.json` | Exit 0; one JSON `ok` report, three rectangle proxies with 200 m² caps and approximately 640 m³ signed volume each; measured outline matches the four expected corners. |
| `node verify.cjs fixtures/l_shape.ifc fixtures/expected.json l_shape.ifc fixtures/source/l_shape.json` | Exit 0; one JSON `ok` report, three L proxies with 75 m² caps and approximately 240 m³ signed volume each; measured outline includes the concave notch. |
| `sha256sum` for changed reader/verifier, lock, expected, three IFCs, two source fixtures, Node loader and WASM | Values matched the updated `ifc-reader-handoff.md` identities. |
| `git check-ignore -v` and `git status --short --untracked-files=all` for the reader directory | Installed `node_modules` assets ignored; curated files visible for staging. |

The reader still supports only the documented schematic IFC4 subset on tested Node 22.22.2 in WSL. The changed IFC fixture provenance properties are tied to the newly retained source bytes; backend exporter reproducibility, whole-process resource limits, actual Omarchy import, visual review and publication are separate gates. This scoped pass makes no claim about those gates.
