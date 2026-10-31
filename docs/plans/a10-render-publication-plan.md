# A09/A10 render publication slice

Base `ff7f6bc` plus independently reviewed A09 current-head patch. Scope: CLI render selection, accepted evaluated source binding, retained project lock, idempotent immutable failure publication, typed render record/receipt and replay, with no installed-runtime bypass. A successful capture must pass the pinned, bounded Rust PNG/GLB verifier before accepted success publication. The service/capture layer checks bytes and worker claims; the independent reader checks decoded previews and transformed mesh geometry against the evaluated source.

1. Add render request and typed records. Resolve the latest evaluated run or explicit run under one lock, verify its source and recomputed evaluations, then call `begin` before launching. Replay returns the stored receipt.
2. Build fixed Blender requests from the verified source and execute under the same lock. Record each failure independently of design checks. Store no worker output as a successful render without independent parsing.
3. Publish the complete bounded record and source/evaluation snapshots through `PendingRun`, and extend semantic history verification for render failures. Add scoped CLI tests for missing gate, revision conflict, replay, partial failure and preserved earlier runs.
4. Run focused Rust tests, format, clippy and diff checks with an isolated SSD Cargo target; freeze the patch and write a handoff for separate Sol review. No commit or host installation.

Deferred production gate: real installed static gate and root-owned complete runtime inventory, target policy/teardown execution, positive Omarchy integration, and visual acceptance. Fixture-only parser success cannot close this gate.

## Camera and preview follow-up, 30 October 2026

Premise under test: a fixed 512-pixel overview camera can give every protocol-valid positive footprint enough foreground pixels for one fixed nonblank verifier. Direct pinned Blender renders at 200 × 2, 0.1, 0.05, 0.02, 0.005 and 0.0001 m test that premise before modifying the gate. The source verifier keeps its border and noise rejection profile.

The worker frames each view to the massing, which is the only rendered subject. When a footprint dimension projects below two pixels at a camera scale, it temporarily renders screen-width massing edges so a thin, real feature remains legible; the GLB and saved scene remain unsymbolized. The response warns which views used the symbol. A Blender tessellation failure for a positive but unrepresentable footprint reports `BACKEND_UNAVAILABLE` at the geometry field. Test normal rectangle/L and both orientations of thin rectangles with the real worker, then run the exact Rust preview gate, captured publication and consistently rehashed replay tests, protocol tests, artifact caps, format/diff checks, and freeze all changed paths for independent review. These checks do not establish pixel-to-model proof or installed Omarchy acceptance.
