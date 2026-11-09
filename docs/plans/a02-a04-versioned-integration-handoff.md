# A02 plus A03/A04 versioned generation integration handoff

This is an uncommitted, detached worktree based on `7b53b1001ae94725177e12c6383debcd9b281f4d`. Root integration was running separately. No root or source worktree files were changed, and no commit, push, or install was made.

## Inputs and base

- A02: `.worktrees/a02-versioning/docs/plans/a02-versioning-freeze.md` and `a02-versioning-review-sol.md`, source commit `f506ffb7cb033d55d27b370b38cd0d94a99e6cb4`. All 12 freeze-list SHA-256 values matched before transfer.
- A03/A04: `.worktrees/a03-a04-gaps/docs/plans/a03-a04-gaps-followup-handoff.md` and `a03-a04-gaps-review-round2.md`, source commit `6ff5d47d4ea3ff9dc30200ac0283c3eb2ee2f3a8`. All 15 freeze-list SHA-256 values matched before transfer.
- Both source commits are ancestors of the requested base. Every frozen path present in its source commit had the same blob in the requested base, so the source working-tree changes apply to that base without an intervening touched-file change.

## Transfer and conflict resolution

The 12 A02 frozen files were copied first. Their exact applied snapshot is Git tree `93b5af0e67bcd4bff17706711b9885a8ed966a6e`. The 13 A03/A04 files outside the two shared paths were copied byte for byte. The A03 `core_workflow.rs` patch applied cleanly on A02 and retains both sets of tests.

The A03 `run_integrity.rs` patch had an import-context conflict. The merged verifier retains A02's recorded `schema_version=0.1.0` and `normalization_version=si-rectangle-v1` gate and `parse_project_v1` replay against stored input and normalized bytes. It adds A03's diagnostic context field, accepts either supported algorithm on a `generate` manifest, and dispatches `chacha8-u53-massing-v1` to `generate_variants_v1` versus `chacha8-u53-massing-diagnostics-v2` to `generate_variants`. `edit` remains restricted to v1 and `evaluate` to its existing version. The canonical generation result is compared byte for byte after recomputation.

The exact code, test, fixture, and schema patch relative to the applied A02 snapshot is [a02-a04-merged-over-a02.patch](a02-a04-merged-over-a02.patch), SHA-256 `282ed732ff503358e08444b5a031fdd43eed05606c599c2eaa5f67678512d429`. Applying it with `git apply --cached` to the A02 baseline tree reconstructed merged Git tree `b8e97d6f151e77a01740b8cad8b59a7b90fa6ec8` exactly. The [merged SHA-256 manifest](a02-a04-merged.sha256) contains all 25 unique frozen paths, including both resolved shared files; its own SHA-256 is `5abedae3ba2bc3d62d5a95ddfc5bd794e8bb273544d10b0f59f13acc9ed053c1`. The 10 A02-only and 13 A03-only transferred files still match their source hashes.

## Validation

All Cargo commands used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-a04-versioned-integration-20261029`; no dependency install was run.

- `cargo test --locked --offline -p archy-generate -p archy-eval -p archy-report -p archy-cli --test generation --test analytic --test report --test core_workflow --test golden_artifacts`: exit 0; 20 core workflow, 1 golden, 13 analytic, 6 generation, and 9 report tests passed. Core workflow includes unchanged historical exhausted-run bytes, legacy preference/source binding, v2 completed and exhausted diagnostics, and consistently rehashed forgery rejection. The golden test verifies five stored revisions and a fresh workflow.
- `cargo fmt --all -- --check`: exit 0.
- `git diff --check`: exit 0.
- `python3 scripts/check-schemas.py`: exit 0; all 14 source schemas matched, Rust serializer fixtures conformed, golden normalized output and 26 negative CLI fixtures conformed. Doctor returned its expected exit 1 with target acceptance deferred.

## Trust limits for independent root review

This is an independent integration candidate, not a root merge or release acceptance. The reviewed A02 slice supports only the historical v1 normalization profile; it does not add a v2 parser, normalizer, or migration policy. The v2 generation label denotes attempt diagnostics and the verifier recomputes it from the supported current generator. Existing successful variants retain v1 identity. The included tests and schema check cover deterministic core behavior, not Omarchy, optional backend parity, arbitrary concurrent filesystem attacks, or full workspace acceptance. Root review should inspect the two shared-file resolutions, independently apply the patch to the stated A02 tree, compare the merged hash manifest, and rerun the gates relevant to its current integration state.
