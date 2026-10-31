# A07/A12 typed adapter protocol handoff

Date: 2026-10-29. Bounded Rust protocol and schema slice, with no commits or
pushes. The workspace has concurrent owners; this document covers the adapter
crate, backend fixtures/schema, authorized root Cargo and schema-checker
integration. It does not claim A07/A12, A09, A13 or Omarchy acceptance.

## Plan and implementation

1. Build immutable typed requests from normalized spec plus canonical Variant.
   Validate IDs/project association, recompute evaluation with `archy-eval`,
   enforce worker capacity, fixed preset and safe logical paths before bytes
   are serialized. Neither request carries an executable, argv or environment.
2. Parse at most 1 MiB of worker response as strict JSON with recursive
   duplicate-key rejection, then decode fixed Blender/IFC success or error
   shapes. Reject unknown nested fields, malformed or nonfinite geometry,
   incorrect identity/version/preset/validation claims, partial-error artifacts
   and unexpected artifact names, media or producers. Artifact hashes and sizes
   are explicitly *claims* for later independent A09 verification.
3. Export the Rust-derived backend schema and exact request serializers. Compare
   committed request bytes with Rust output, feed those bytes through each
   import-free Python `parse_request`, and decode response fixtures captured
   from actual pinned WSL worker runs. Mutate critical nested fields as
   regressions. Register backend and report source-derived contracts in the
   existing schema checker without removing its ten prior schema checks.

The implementation reused the workspace's `archy-core`, `archy-eval`, `serde`,
`serde_json` and `schemars`; it added no new external runtime package. Root
`Cargo.toml` now lists `archy-adapters` and Darwin's `archy-report` as members.
After the adapter/report/CLI manifests were loadable, `cargo generate-lockfile
--offline` updated `Cargo.lock` with both packages and existing dependencies.
The resulting lockfile SHA-256 at this handoff is
`01de4d6615a78d672c9969e5d478d3675cd6feda6fc052a0486653dba9f571ce`.

The Blender fixture came from pinned Blender 4.5.15 LTS, build
`5afeabba6097`, bundled Python 3.11.15, under untracked
`/home/butahlecoq/work/archy/.tools/adapter-blender-fixture`. The IFC fixture
came from the isolated pinned CPython 3.12.16/IfcOpenShell 0.9.0.post1 WSL
worker under untracked `.tools/adapter-ifc-fixture`. The IFC response's wire
`backend_version` is `0.9.0.post1`; the native wrapper separately reports
`0.9.0post1+b5e509b`. The four committed fixture file SHA-256 values are:

| Fixture | SHA-256 |
| --- | --- |
| `blender-request.json` | `81f1d4b9d8239da29089880f19db61f4a698db8daddbac0f113a002a72eba14e` |
| `blender-response.json` | `40b73b4a2e6059c61e06b4128c99f78b69c9cf70c7fa07986d66d3b288f71092` |
| `ifc-request.json` | `2c19e2a24de4e657807e5e504ead3cbebf85cb356c5e581734b31c4f6d45aaf4` |
| `ifc-response.json` | `1d4468b0062205aeff09a0b1ad2915dfe353e98b3375ec2b3bd4f080311c495f` |

The source-derived `backend.schema.json` SHA-256 is
`368485f06b7face8bccded2a0986dba0945c90fd7e44f4ccc74373841b4fad0e`.
The IFC worker was being corrected during fixture authoring; recheck this
fixture against its final frozen response before integration. These JSON files
are protocol evidence, not independent checks of the files named in responses.

## Checker review repair

The independent storage-schema review found two missing relationships in
`scripts/check-schemas.py`. The checker now recomputes
`SHA256(b"archy-run-request-fingerprint-v1\0" + raw_serialized_request_utf8)`
and compares it with both manifest and revision `request_sha256`. It also
binds `request.context.destination` to `receipt.path`. Both the live Rust
transaction exporter and saved fixture have separate probes that alter
`request_bytes` and destination; the destination probe recomputes the affected
fingerprint and manifest digest so it tests the intended binding. No storage
runtime code was changed. This is ready for Popper's scoped independent recheck;
no reviewer action is claimed here.

## Exact final checks and observed failures

Run from the repository root on this source snapshot:

```sh
cargo generate-lockfile --offline
cargo fmt -p archy-adapters --check
cargo clippy -p archy-adapters --all-targets --locked --offline -- -D warnings
cargo test -p archy-adapters --locked --offline
python3 scripts/check-schemas.py
```

All five final commands exited 0. The adapter test command ran **8 tests,
8 passed, 0 failed** (plus zero unit/doc tests). It exercises canonical
request preparation, wrong identity/capacity/path rejection, duplicate and
oversized response bytes, real Blender and IFC success fixtures with field
mutations, valid error envelopes with partial-artifact rejection, and exact
Rust request bytes accepted by both Python validators.
The checker validated all **12** schema files, source-derived backend and
report serializer data, both backend response fixtures, actual CLI normalized
output and all **26** negative brief fixtures. The live doctor JSON conformed
with exit 1 and `target_acceptance=deferred`.

Retained red evidence: before response fixtures existed, the two strict
response fixture tests failed on missing files. A first IFC fixture/decoder
iteration exposed the wire-version versus native-wrapper version mismatch; the
wire constant was corrected to `0.9.0.post1`. Removing an unused adapter dev
dependency made `--locked` exit 101 until the lockfile was regenerated offline.
The first formatting check after the minimum-height edit exited 1 on a missing
brace and test line wrapping; the brace was fixed and `cargo fmt -p
archy-adapters` applied. The final locked commands above then passed. Earlier
coordinator `--locked` build failure during concurrent manifest additions was
the same stale-lock condition, not a new external dependency requirement.

## Integration boundary

A09 still must own trusted runtime paths and fixed argv, bounded supervised
execution, isolation, artifact byte reading, independent PNG/GLB/IFC checks,
hash recomputation and immutable publication through A05. This crate does not
use worker hashes as authority, implement a production render CLI, or approve
designs. IFC second-engine A13 verification and actual Omarchy execution remain
deferred. The current response fixtures and strict decoder pass on this source
snapshot; an IFC worker response-shape change requires fixture and decoder
reconciliation before any later integration claim.
