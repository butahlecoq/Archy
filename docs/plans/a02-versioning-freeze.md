# A02 versioning re-review freeze

Base: `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`; branch `codex/a02-versioning`. This is an uncommitted working-tree candidate. The freeze file itself is excluded from its hash list.

The only changes since the independent review are the golden test correction, handoff, and this freeze. Run the focused golden test and inspect the fixture comparison before re-reviewing the unchanged verifier.

| Path | SHA-256 |
| --- | --- |
| `Cargo.lock` | `13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40` |
| `crates/archy-core/src/lib.rs` | `c92f273c2d9cbbb173a6d7b392c29513f90a1da4fc72492b177511f03d3b61c1` |
| `crates/archy-core/src/project.rs` | `fca7c7db21d9eadda788934bd84332bcaeb81443182d141ce8d9b5477e0b77de` |
| `crates/archy-core/src/project/v1.rs` | `89eb57e2518e282389057b08c0791ad4813c474c58112f9a530c29c70f56fa6c` |
| `crates/archy-core/src/validation.rs` | `2354fb5fa868372d4ca793e616f3b6c3faff412bd38655a204778429bb121597` |
| `crates/archy-core/src/validation/v1.rs` | `10312f50576c34515db32cd1e6025bc19ee5531c147074b66982721746d391ca` |
| `crates/archy-cli/src/run_integrity.rs` | `b9ea3fafdd202f1f3c0c5f74c6390f1e1fa5641cd27e7abbd02d2da0d127517c` |
| `crates/archy-cli/tests/core_workflow.rs` | `ebd46ebe7ef1f06dd37e143130285b382cc497605ba082f3166eeff69166b56f` |
| `crates/archy-cli/tests/golden_artifacts.rs` | `8d2adcd3b4965174f286abb369d56db939cd9fe1deb4fa762aacd165c3bc1180` |
| `tests/fixtures/briefs/legacy-adversarial-preferences.json` | `d3b51c16a980a422b60b972de9e47ca176f9ed2d937706743efac191950fdd75` |
| `docs/plans/a02-versioning-handoff.md` | `c8a3f6caa25919f073602f80b669d6e87b18a77d04da1d9ef96d6d5cd212c541` |
| `docs/plans/a02-versioning-review.md` | `ffdfdfe9426d935ec3ce0ec400d9581887b49b39f56bdbd9c9c0ebcd5b93b3ad` |

Accepted-project fixture: 59 files; aggregate SHA-256 `dc78571a1468af8b789adb7021d5080c56b24eb82c08cd114764d9284bd4c909`. Compute it over sorted worktree-relative paths, each encoded as UTF-8 path, NUL, lowercase file SHA-256 hex, newline.

The workspace test log is `/home/butahlecoq/work/archy/.tools/targets/a02-versioning/a02-review-fix-tests.log`; the handoff records commands, exit codes, ignored gates, and the withdrawn earlier claim. Independent Sol-medium re-review remains outstanding.
