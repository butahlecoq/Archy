# Historical normalization integration

The historical v1 normalization candidate was copied from base `f506ffb`
onto `7b53b10`. Before copying, all 12 hashes in the author's freeze matched,
and the integration copies of each modified tracked file matched that base.
The copied files were verified byte-for-byte. Independent Sol review in
`a02-versioning-review-sol.md` covers the frozen candidate, legacy source
binding, rehashed forgeries, golden history and disposable mutation probes.

Integration checks use a new Cargo target directory at
`.tools/targets/a02-integration-7b53-fresh`, with logs under
`.tools/a02-integration-7b53-logs` in the workspace wrapper. Formatting,
strict workspace Clippy, locked workspace tests, and schema checks all passed.
The 12 frozen hashes were rechecked after the tests.

This is a historical v1 compatibility boundary. It does not implement v2
normalization or a new preference policy, and does not complete A02.
