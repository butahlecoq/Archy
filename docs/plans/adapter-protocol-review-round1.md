# Independent adapter protocol fix rereview, round 1

Date: 2026-10-29. Scope: the P1 live IFC diagnostics mismatch in `docs/plans/adapter-protocol-review-final.md` and Rawls's frozen fix handoff. Reviewer made no product, fixture, or schema changes.

## Disposition

**P1 resolved on this pinned WSL runtime.** The current IFC worker emits `validation.reopen_process` and `validation.express_validation.failure`. The Rust decoder now requires both, models successful failure markers as explicit JSON null, requires zero reopen and EXPRESS exit codes, empty EXPRESS findings, and caps each diagnostic `stderr` at 4,096 UTF-8 bytes. Nested diagnostic structs retain `deny_unknown_fields`; the whole response still has its 1 MiB cap. The focused mutations reject nonzero codes, nonnull or missing failure markers, oversized stderr, and unknown diagnostic fields. The committed response fixture includes the current shape and passes strict Rust decoding. No new blocking finding in this scoped review.

## Evidence

| Check | Observed result |
| --- | --- |
| `cargo test -p archy-adapters --test protocol --locked --offline` | Exit 0; 8 passed, 0 failed, 1 intentional native test ignored. |
| `ARCHY_IFC_PYTHON=/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python cargo test -p archy-adapters --test protocol rust_request_to_native_ifc_worker_to_rust_decoder --locked --offline -- --ignored --exact` | Exit 0; 1 passed. Rust request bytes matched the committed request fixture; the frozen Python worker returned success; Rust decoded the actual response. No runtime rebuild. |
| Source-derived schema equality and fixture validation | `cargo run --locked --offline --quiet -p archy-adapters --example backend_contracts` output matched `schemas/backend.schema.json`; the committed IFC response passed the generated Draft 2020-12 IFC response schema. |
| Schema diagnostic definitions | `IfcValidation` requires `reopen_process`; `ReopenProcess` and `ExpressValidation` require `failure` of JSON null type; both diagnostic objects set `additionalProperties: false`. |
| Fixture identity | `ifc-response.json`: 1,897 bytes, SHA-256 `9d18932401ab00f5cddb23670294ab70eeb6a96dbd42c61f715ce426b2cc5ce8`. Schema SHA-256 `95efc31a76d9566da04d5402fbc90c10c7c713731e7db40fd138e8483301fc75`. |

The roundtrip confirms wire compatibility on the existing CPython 3.12.16/IfcOpenShell 0.9.0.post1 environment. The response fixture and decoder accept worker *claims* about artifacts; this review does not replace A09 artifact byte reading, hash recomputation, or independent file validation. A13 and actual Omarchy acceptance remain separate gates.
