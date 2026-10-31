# IFC adapter response reconciliation handoff

Date: 2026-10-29. Scope: `crates/archy-adapters/src/response/ifc.rs`, `crates/archy-adapters/tests/protocol.rs`, `tests/fixtures/backend/ifc-response.json`, `schemas/backend.schema.json`, and this handoff. No Python worker, CLI, report crate, workflow, commit, or push was changed. Independent Hume rereview is pending.

## Root cause and plan

`docs/plans/adapter-protocol-review-final.md` reported a P1 mismatch: the frozen Python IFC worker adds `validation.reopen_process` and `validation.express_validation.failure`, but the Rust strict decoder and committed fixture omitted them. The reviewed fixture predated Herschel's worker fixes. The current installed worker file has SHA-256 `bb1cf33347ad9b46429c4a136697938401164a81238e5e60870637fae7b59f3c`, identical to `workers/ifc/archy_ifc_worker/worker.py` in this checkout.

Plan executed: capture a real success response, reproduce the Rust rejection, model the diagnostic fields with strict types, reject unsuccessful or oversized diagnostics, add fixture mutations and an opt-in real roundtrip test, then regenerate the source-derived backend schema. No new dependency was needed; this uses existing `serde`/`schemars` types and the pinned optional Python worker only in the opt-in integration test.

## Changes

- A fresh native run used pinned CPython 3.12.16 and IfcOpenShell 0.9.0.post1 with `tests/fixtures/backend/ifc-request.json` in a temporary project. Its 1,897-byte `status=ok` response was captured verbatim as `ifc-response.json` (SHA-256 `9d18932401ab00f5cddb23670294ab70eeb6a96dbd42c61f715ce426b2cc5ce8`). The old 1,684-byte fixture lacked both diagnostics. The temporary model and validation files were removed with their temporary directory; the fixture's artifact paths, sizes and digests remain worker claims, not independent artifact authority.
- `IfcValidation` now requires a typed `reopen_process` with integer `returncode`, bounded string `stderr`, and explicit null `failure`. `ExpressValidation` requires explicit null `failure`. Success validation requires reopen return code zero, existing EXPRESS exit code zero and empty findings, and at most 4,096 bytes in each diagnostic stderr. Both nested structs retain `deny_unknown_fields`; the 1 MiB whole-response cap and existing identity/geometry checks remain in force. Diagnostics do not supply geometry or approval authority.
- The fixture regression now rejects nonzero reopen status, nonnull failure markers, oversized stderr, missing failure marker, and unknown fields in either diagnostic object. An opt-in Rust test serializes the canonical IFC request, checks it against the request fixture, launches the pinned worker, reads its actual response, and decodes it through Rust. It is ignored in the default core suite because it requires the separately installed heavy backend; invoke it with `ARCHY_IFC_PYTHON` as below.
- `backend.schema.json` was regenerated from `backend_contracts`. The schema now requires `reopen_process` and both `failure` fields; each `failure` has JSON `null` type and both nested objects disallow additional properties. The existing schema checker needed no source change: it already compares the backend schema to source output and validates the IFC fixture.

## Exact local evidence

Before the decoder change, `cargo test -p archy-adapters --test protocol strict_ifc_response_fixture_and_mutations --locked --offline` exited 101: the freshly captured valid response failed strict deserialization (`data did not match any variant of untagged enum IfcResponse`).

After the change, all commands below exited 0 from the main repository:

| Command | Result |
| --- | --- |
| `cargo fmt -p archy-adapters --check` | No formatting diff |
| `cargo clippy -p archy-adapters --all-targets --locked --offline -- -D warnings` | No warnings |
| `cargo test -p archy-adapters --test protocol --locked --offline` | 8 passed, 0 failed, 1 intentional backend integration ignore |
| `ARCHY_IFC_PYTHON=/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python cargo test -p archy-adapters --test protocol rust_request_to_native_ifc_worker_to_rust_decoder --locked --offline -- --ignored --exact` | 1 passed, 0 failed; real Rust request → frozen Python worker → Rust decode |
| Source exporter equality check against `schemas/backend.schema.json` | Equal; schema SHA-256 `95efc31a76d9566da04d5402fbc90c10c7c713731e7db40fd138e8483301fc75` |
| `python3 scripts/check-schemas.py` | All 12 schemas valid; registered source-derived schemas, including backend, matched; backend fixture and request contracts conformed; 26 CLI negative fixtures passed; doctor report conformed with deferred target acceptance |

The real roundtrip confirms protocol compatibility on this pinned WSL environment. A09 artifact reopening/hash authority, A13 second-engine IFC verification, and Omarchy acceptance remain separate gates. The independent Hume reviewer should inspect this bounded change before parent publication.
