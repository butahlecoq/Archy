# A09 artifact capture: independent review

**Verdict: changes requested (one P2 provenance finding).** The success byte-capture boundary and nonpublication boundary are sound in this frozen slice. The declared Blender error path cannot establish that its response belongs to the supplied request.

## P2 — Bind declared Blender errors to the request, or label them unbound

`capture_blender(&File, &BlenderRequest)` passes the request to `decode_blender_response()`, but the `BlenderResponse::Error` branch checks only schema version and an empty artifact list (`crates/archy-adapters/src/response/blender.rs`). `BlenderFailure` has no request ID, project ID, variant ID, run ID, or digest fields. Consequently the same syntactically valid `response.json` error can become `CaptureOutcome::WorkerError` for two different Blender requests; its `response_bytes` and SHA-256 can then be retained as if they were job-specific evidence. IFC's error branch checks those request identities. This is a source-path finding; I did not execute a separate cross-request fixture.

Coordinate a versioned Blender error protocol/schema and worker change that carries and checks request identity before the error is treated as attributable to the current request. If that is outside this slice, make the API/handoff explicitly classify the Blender response as **unbound diagnostic bytes**, and forbid using its worker-declared code as authenticated job evidence. Keep the failure a failure; do not promote partial files or the response into accepted outputs. Add a focused cross-request negative test. This does not undermine successful artifact capture, because the success branch already binds request identity and exact claims.

## Checks and limits

The reviewed files match the handoff hashes: `artifacts.rs` `819b14087f7875f0ebd80602808ce5b302d0b20397893a8a50583e94414238c4`, `tests/artifacts.rs` `6076de9f6dc97cc770ace5fec934e9eeffaac7547362925d3c9e37b3951876a5`, `lib.rs` `c4167ff3d5dd56f2898f9804d70621287fdb08e77b40f635cadd700230c1543c`, adapter `Cargo.toml` `64ea0a6b5eae9a4eba86af21722c83f2a08ddb170c39725e8b3d2742db7c746e`, and `Cargo.lock` `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40`. I inspected the full five-file snapshot and the existing response decoders.

The public API accepts a caller-held directory `File` and typed request, with no arbitrary path or command argument. For success it requires the fixed Blender/IFC inventory, `openat2` beneath/no-symlink/no-magic-link/no-cross-device resolution, regular single-link metadata via `O_PATH` before `O_NONBLOCK` reads, the 1 MiB response and 16 MiB per-artifact bounds, checked 128 MiB aggregate accounting, one bounded read per file, byte-length/SHA-256 claim equality, and final name/metadata checks. It returns owned bytes and fixed metadata; it makes no semantic geometry or publication claim. Declared errors keep response bytes and typed failure, while known partial files remain unreturned and unsafe/extra partials are reported separately through `scratch_issue`.

Independent focused command `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo test --locked -p archy-adapters --test artifacts` exited 0 with 6 passed. The handoff reports 14 adapter unit and 8 protocol tests, package format, and strict package clippy passing; I did not rerun those or the whole workspace.

This API presumes the caller has proved whole-writer teardown and supplied a private quota-bound scratch capability. The tests do not establish those preconditions, resistance to a malicious same-user writer restoring metadata, independent file-format/geometry verification, or storage publication authority. No production source or target environment was changed by this review.
