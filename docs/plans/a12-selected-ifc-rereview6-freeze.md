# A12 selected IFC context correction: review freeze

30 October 2026. Candidate worktree: `/home/butahlecoq/work/archy/.worktrees/a12-ifc-selected-20261030`. Base HEAD: `160787d1f6c65a85d0e022a111d56ee7b0eb2b86`. The 35-path implementation patch before adding this record has SHA-256 `77f945ea30072bf77a65b91a8527a36c35620aafd8c2ab62ee51aa88fcf51a4f` (`git diff --cached --binary -- . ':(exclude)docs/plans/a12-selected-ifc-rereview6-freeze.md' | sha256sum`). This record is staged as the 36th path. No commit, push, install, other-worktree write, or child agent was used.

## Finding and correction

The rereview-five reachable `TrueNorth` direction, model precision `1.`, and context identifier `'Other'` each returned `status=ok` through the actual three-part framed reader with unchanged accepted source bytes. The reader checked the context count/type/dimension but did not check those fields. A direct framed regression was red on the reachable `TrueNorth` case before the change. The reader now requires the exporter model context's null identifier and `TrueNorth`, precision `1e-5`, identity world origin/axis/ref direction, and the body subcontext's null target scale, user target label, precision and north. It retains coordinate tolerance for numerically equivalent identity vectors.

An audit of the same reachable schematic graph found other unverified claims: canonical footprint profile name/self-intersection, project/site/building identity, optional spatial descriptions/geolocation/elevations/address, proxy description, and quantity descriptions/measurement method/formulas. The reader now checks these against actual exporter output and accepted project/variant identity. A second red direct test showed that a reachable `FootprintArea` description claiming certification was accepted before the final null-field check. Mesh outline, area, volume and placement checks remain in force. The source mapping still supports the real rectangle and L fixtures and accepted project JSON; no YAML acceptance is inferred.

The Rust native false-success fixture now appends a reachable `IFCDIRECTION` as `TrueNorth`, recomputes artifact hash and size claims, and verifies rejection before publication, empty staging and unchanged revision, followed by genuine publication and replay. Existing first/last duplicate provenance/quantity and duplicate-context framed cases remain in the suite.

## Frozen files and pins

- `reader.cjs`: `330421009582effe2fecdc23e264a1538db5074a405707601d2e1c7a4522feb5` (matches `READER_SHA` in `ifc_verifier.rs`).
- `verify-stdin.cjs`: `77ccf960952d413267147ec8cce98e71a4e7143ac0283d782855f77e5fb00403`.
- `step-scope.cjs`: `046e1377dc647c6da42ac4a2894c2413ddd7eb4d428e58e7c7cf697e762f6762`.
- `reader.test.cjs`: `a1e04860a8b822508c88faa7d8cbf474e0f08651ade3844b59c4f4664d73a6eb`.
- `ifc_verifier.rs`: `c41853ddb9542b0b1822b47e4c8ead38a24a78e75f18d31597b2fa9e8a1735c3`.
- `export_ifc.rs`: `e4ecd1a329414ab84823e1f60fce0540b5d1e0a7b0b40a23ef2c10fd1f70b9fb`.

## Verification evidence

- Direct framed regression: both rectangle and L fixtures passed unchanged; reachable north, precision, context identifier, world origin/orientation, body options, profile semantics, identity/geolocation, proxy wording and quantity claims returned `ARTIFACT_INVALID`; equivalent near-identity orientation passed. Focused test passed after the red run.
- Full pinned Node 22.22.2 reader suite on final reader: 29 passed, 0 failed, 66.83 seconds. This includes real fixture parsing, duplicate member and context cases, STEP scope, transformed meshes and source binding.
- Native selected export/replay on final reader: `CARGO_BUILD_JOBS=1`, `TMPDIR=/var/tmp` on ext4, pinned IfcOpenShell Python and Node, 1 passed, 0 failed, 127.12 seconds. It exercised matching false-success claims, the new reachable north payload, source preservation, retry, genuine rectangle/L publication and replay. This run had no timeout. Prior review runs did report `BACKEND_TIMEOUT` under build contention; those are recorded in the earlier handoff and are not reclassified as passes.
- `cargo clippy --locked --offline -p archy-cli -p archy-adapters -p archy-service-gate --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --cached --check`: exit 0.

## Independent review handoff

Rereview the semantic profile and attempt another reachable, supported STEP mutation that changes a model-context or placement/orientation claim while preserving real geometry and accepted source. Check that the new native `TrueNorth` mutation actually changes the generated IFC, is rejected before immutable publication, and that genuine replay still reads the pinned script. Validate the full staged diff, hashes and index state before grafting. The installed fixed runtime/service, Omarchy acceptance, YAML selected-source export, visual review and human approval remain separate gates; this development-host test does not establish them.
