# A05 courtyard golden evidence rereview, round 1

Date: 2026-10-29. **Scoped verdict: PASS.** The two findings in `a05-golden-review.md` are addressed in the frozen `golden_artifacts.rs` test. This review changed only this document; no source, fixture, commit, push or agent action was performed. The separate A05 final CLI review remains open.

## Finding adjudication

1. **Fresh-checkout directory modes — resolved.** `copy_tree` now creates a disposable project copy with `0755` directories and `0644` files regardless of author-worktree modes. The test asserts those modes, calls `verify_run` and requires `PERMISSION_DENIED`, then recursively sets only the disposable `.archy/` and `runs/` directory trees to `0700`. It confirms the source fixture's `.archy` mode is unchanged before verifying all five accepted runs. This directly exercises the mode state a normal Git checkout can produce without changing storage's private-directory policy or the tracked original.
2. **Report JSON byte comparison — resolved.** `assert_report` now reads the actual serialized `report.json`, replaces only pretty-printed `context.run_id` and `context.parent_run_id` values with the saved execution IDs, and compares all remaining bytes with the saved report. This retains whitespace, key order, hashes, algorithms, counts, metrics and other provenance in the comparison. Markdown still uses escaped execution-ID mapping and a byte comparison; variant, evaluation and generation JSON comparisons remain exact.

## Fresh verification

| Command | Observed result |
| --- | --- |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo test --locked -p archy-cli --test golden_artifacts` | Exit 0; Cargo discovered one golden test, one passed, zero failed/ignored. The test includes the public-mode rejection and private disposable-copy verification path. |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo clippy --locked -p archy-cli --all-targets -- -D warnings` | Exit 0. |
| `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo fmt --manifest-path crates/archy-cli/Cargo.toml -- --check` | Exit 0. |

The earlier independent audit in `a05-golden-review.md` verified the source hash, five revision links and manifests, report relationships, 46 artifact hashes, three design metrics/checks, restored canonical ID, and visibility of all 60 golden files through Git ignore rules. No saved fixture or evidence inventory was regenerated in this fix round. This pass covers the WSL golden fixture/test boundary; failed or unknown designs, backend outputs, full A05 integration, and native Omarchy acceptance remain separate gates.
