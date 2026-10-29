# A02+A03 merge integration review

**Verdict: scoped PASS.** I found no merge-specific break in historical v1 replay or generation-version dispatch. This review covers the A02+A03 integration in this worktree, not full A02, A03, or A04 acceptance. I made no implementation edit, commit, push, or host install.

## Patch and source identity

- The handoff's baseline tree `93b5af0e67bcd4bff17706711b9885a8ed966a6e` matches all 12 A02 freeze SHA-256 values. In a disposable Git index, `git apply --cached --check` and `git apply --cached` of `a02-a04-merged-over-a02.patch` produced tree `b8e97d6f151e77a01740b8cad8b59a7b90fa6ec8`, exactly the handoff value. The reconstructed shared-file blobs match the live files.
- The patch SHA-256 is `282ed732ff503358e08444b5a031fdd43eed05606c599c2eaa5f67678512d429`. The merged manifest SHA-256 is `5abedae3ba2bc3d62d5a95ddfc5bd794e8bb273544d10b0f59f13acc9ed053c1`. `sha256sum -c` passed for all 25 paths. Ten A02-only paths and 13 A03-only paths retain their source freeze hashes; only the expected shared `run_integrity.rs` and `core_workflow.rs` differ from each source.

## Merge behavior

`run_integrity::verify_one` checks the stored `schema_version=0.1.0` and `normalization_version=si-rectangle-v1`, then calls `parse_project_v1` on the stored input and compares the exact serialized normalized bytes. Separately, a `generate` manifest accepts only `chacha8-u53-massing-v1` or `chacha8-u53-massing-diagnostics-v2`. It calls `generate_variants_v1` for the former and `generate_variants` for the latter, then compares the exact canonical `generation.json` bytes. `edit` remains limited to the v1 algorithm; `evaluate` retains its evaluation version. The report context carries diagnostics only when the recomputed generation has them.

The merged tests retain A02's legacy preference and golden checks and add A03's captured exhausted-run and completed/exhausted diagnostic checks. The historical test restores each captured file, verifies the run, and compares each file's bytes afterward. The preference test pins the source and normalized hashes, checks stored input and normalized bytes, verifies after a live source edit through `verify_run`, checks same-key replay and changed-source conflict, and rejects a rehashed normalization-version forgery. The completed diagnostic test checks a nonempty rejection, report propagation, verifier acceptance, downstream evaluation, and a rehashed diagnostic-count forgery. The exhausted diagnostic test checks 100 rejected attempts and report propagation. These are meaningful assertions on stored artifacts and public behavior.

## Independent disposable CLI probes

I used the binary built from this worktree in the dedicated target below and temporary project directories. A restored exact-base exhausted fixture passed `archy desktop status --run`; all recorded file bytes stayed identical. Changing its manifest algorithm to v2 and consistently rehashing the revision then failed with `ARTIFACT_INVALID: generation result differs from supported algorithm`.

Fresh generation from `legacy-adversarial-preferences.json` stored the exact source bytes. Its source SHA-256 was `d3b51c16a980a422b60b972de9e47ca176f9ed2d937706743efac191950fdd75`; its `normalized.json` SHA-256 was the pinned v1 value `5665a318acf3abe884945905371fab72415405276bcb99783bc78fec2ea10bf3`. The accepted run passed desktop status.

A fresh completed narrow L-shape run had four attempts, one variant, three footprint rejections, and a v2 diagnostic algorithm label. `report.json` carried exactly the same diagnostic object as `generation.json`; desktop status accepted the run. Changing the stored normalization label to v2 and consistently updating the artifact, manifest, and revision hashes failed with `ARTIFACT_INVALID: unsupported recorded normalization profile`. In another fresh run, changing the v2 manifest label to v1 and rehashing the revision failed with the generation mismatch above. The two version decisions therefore reject crossed labels independently.

## Validation and limits

Cargo used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2` and fresh SSD target `/home/butahlecoq/work/archy/.tools/targets/a02-a04-independent-review-20261029`. `cargo test --locked --offline -p archy-cli --test core_workflow --test golden_artifacts` passed: 20 workflow tests and one golden test. `cargo test --locked --offline -p archy-generate -p archy-eval -p archy-report --test generation --test analytic --test report` passed: six generation, 13 analytic, and nine report tests. `git diff --check` passed.

The supported normalization profile is still v1. A v2 generation label denotes diagnostic output, not a new source normalizer. These probes and tests do not establish full workspace, Omarchy, optional backend, or product acceptance.
