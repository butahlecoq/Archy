# A04 historical claimed marker evidence handoff

29 October 2026. Detached worktree `/home/butahlecoq/work/archy/.worktrees/a04-historical-marker-ff7-20261029` starts at `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1`, including the escaped pipeholder fixture fix. Applied the unchanged `/home/butahlecoq/work/archy/.tools/a04-authority-followon-8e36.patch` (SHA-256 `12363c82cb65aba845dbd3c2af9507e5055aaef362255fbab40b19e6c4332f07`). The earlier A04 handoff and Sol review are retained in this tree. The original A04 worktree was not edited. No commit, push, installation, or child agent.

## Historical production and provenance

The source was `git archive 7728da0d986821fd1b10245d7a56f40af3f9c37d`, a pre-A04 commit, extracted into `.tools/a04-historical-src-7728da0`. The archived `crates/archy-core/src/validation/v1.rs` hashes to `10312f50576c34515db32cd1e6025bc19ee5531c147074b66982721746d391ca`; its constraint parser requires only a nonempty `source` and copies the string into `NormalizedConstraint`. It sets `si-rectangle-v1`. The archived `crates/archy-eval/src/lib.rs` makes one measured geometric check per constraint, includes the source in an *unverified* explanation, and has no authority check branch. This establishes that the historical parser supported the marker, rather than inferring that from current code.

Built the archived CLI with `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a04-historical-ff7-20261029 cargo build --locked --offline --manifest-path /home/butahlecoq/work/archy/.tools/a04-historical-src-7728da0/Cargo.toml -p archy-cli --bin archy`. The copied executable `/home/butahlecoq/work/archy/.tools/a04-historical-archy-7728da0` hashes to `ff3fafbd690135a3105817202853232f52c2f24fa341e33ddee5887ed30c4e87`.

The input derives from `tests/fixtures/briefs/courtyard.json` by setting the height constraint's source to `claimed_regulation` and authority to `user claimed local rule (unverified)`. Its exact saved bytes hash to `0ad639435f1556039f2a71d0fdb3b026da404a2c83b38984ba4ea784efc82902`. The historical executable accepted these real CLI operations against that input:

```text
archy generate --project project.json --count 1 --seed 42 --idempotency-key historical-marker-generate-7728da0 --expected-project-revision 0 --json
  -> e80391ef-a7ec-4900-8d05-9409b250094e, revision 1
archy evaluate --project project.json --run e80391ef-a7ec-4900-8d05-9409b250094e --all --idempotency-key historical-marker-evaluate-7728da0 --expected-project-revision 1 --json
  -> a112afe0-ab7b-414d-8f35-11d5f97814c9, revision 2
```

The whole accepted project, including input snapshots, normalized snapshots, manifests, revision records, evaluations, reports, events, and the empty lock file, was copied byte for byte into `tests/fixtures/historical-claimed-v1`. `diff -qr` against the original capture returned no differences. Both accepted evaluations say `planar-massing-v1` and contain only `site_containment`, `height`, and `setback` checks. Both reports say `archy-report-v1`. This is a chain produced now by historical executable bytes, not evidence that this particular run was published before A04. Its passing required status is the old geometric result and grants no regulatory or human approval.

## Current verification

The focused `pre_a04_claimed_marker_chain_verifies_and_replays_exact_saved_bytes` test copies the fixture, verifies both accepted runs with the current A04 verifier, checks the recorded evaluation and report hashes and absent authority checks, then replays both historical CLI requests using their original keys. It compares the complete copied tree to the original fixture after replay and confirms revision 2. The focused run passed. With the separate current target, `cargo test --locked --offline -p archy-cli --test golden_artifacts -p archy-eval --test decision_contract` passed 2/2 golden and 7/7 evaluator tests. The separate `core_workflow` claimed-regulation test passed 1/1. `cargo fmt --all -- --check` and the source-only `git diff --cached --check` passed. A neighboring test first failed to compile because the historical and current builds shared a Cargo target and linked the old `archy-core`; this was an artifact collision, so current-tree checks use a separate unique target at `.tools/targets/a04-current-ff7-20261029`.

The archived `report.md` files contain the historical renderer's two-space Markdown hard breaks. An unscoped `git diff --cached --check` flags those trailing spaces; changing them would invalidate the accepted report bytes. Whitespace checks therefore exclude only the saved fixture directory. The existing A02 gate remains: the ff7 A04 slice does not prevent a fully rehashed fresh source from claiming a v1 profile. The local chain has no external trust anchor. Neither this fixture nor the passing historical geometry asserts non-downgrade, provenance of an old publication, or regulatory authority.

## Sol review inputs

Review `/home/butahlecoq/work/archy/.tools/a04-historical-marker-ff7-20261029.patch` against `ff7f6bc`, the all-path freeze `docs/plans/a04-historical-marker-freeze.sha256`, the historical source at `7728da0`, and the new behavior test. Check that every fixture file is the output of the archived CLI, that both old revisions and reports replay unchanged, and that the A02 profile gate remains explicitly outstanding. The prior follow-on review is copied unchanged as `docs/plans/a04-authority-followon-review-sol.md` (SHA-256 `4cb9131ca90cfed447299f00624229bbbd983eef05173d45985a710536c8fd45`).
