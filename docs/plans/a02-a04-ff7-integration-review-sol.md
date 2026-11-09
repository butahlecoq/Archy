# A02+A04 ff7 integration: independent adversarial review

30 October 2026. Scope: the detached `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1` worktree and its frozen A02+A04 patch. I read `AGENTS.md`, `docs/specs/release-0.1.txt`, the integration handoff, freeze and plan, the earlier A02/A04 reviews and historical marker provenance, then inspected the changed source and ran disposable CLI probes. No source edit, other worktree, commit, push, installation, or child agent was made. This file is the only review write.

## Verdict

**FAIL for new claimed-regulation operations descended from accepted v1 history.** Fresh native v2 and fresh imported v1 sources use the authority v2 profile as advertised, and saved v1 history remains verifiable. A *new* edit or evaluation of the genuine accepted v1 claimed source can still publish a v1 run with all required checks passing and no authority UNKNOWN. Historical byte preservation does not require allowing new work under the retired authority policy.

## Verified finding: new children of historical v1 claims bypass the authority gate

Starting with a private-permission copy of `tests/fixtures/historical-claimed-v1`, whose accepted head is revision 2, run the current CLI with a **new** idempotency key:

```text
archy evaluate --project <copy>/project.json --run e80391ef-a7ec-4900-8d05-9409b250094e --all --idempotency-key new-after-upgrade --expected-project-revision 2 --json
```

Observed exit 0, revision 3, `all_required_checks_passed: true`. The new evaluation is `planar-massing-v1`, has only `site_containment`, `height`, and `setback` PASS checks, and its report is `archy-report-v1`; `profile.json` is absent. A second disposable copy with `edit.json` containing the historical variant's parameters, `storeys: 2`, and its original `floor_to_floor_m` also succeeds:

```text
archy generate --project <copy>/project.json --from-run e80391ef-a7ec-4900-8d05-9409b250094e --variant variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b --params edit.json --idempotency-key new-edit-after-upgrade --expected-project-revision 2 --json
```

That new revision 3 edit records `planar-massing-v1`, `required_checks_passed: true`, and no profile; `archy compare --run <new-edit-run> --json` reports the candidate as passing. These are new commands, not unchanged-key replays. I repeated both commands with the binary freshly built in the dedicated review target and got the same revision 3, v1 three-PASS-check results. The source still declares `claimed_regulation`. The required result for new claimed work is an independent `authority:height` UNKNOWN with null measured/limit and false aggregate status, or refusal to extend the historical profile. The release specification says an UNKNOWN required check cannot be marked compliant or automatically approved.

The root is dispatch by parent profile in `generate.rs` (edit branch) and `evaluate.rs`, followed by publication of that inherited v1 profile. `verify_run` faithfully accepts the new chain because it recomputes v1 evaluations for v1 normalized snapshots. The A14 binder also has an intentional v1 branch for historical evidence; it does not distinguish a newly published v1 child from an old one. This finding concerns current publication policy, not a forged local chain or the exact old bytes. Permit unchanged-key replay of the two saved runs, but require migration to v2 or fail closed for a fresh child of a claimed v1 source.

## Identity and positive evidence

- `sha256sum -c docs/plans/a02-a04-ff7-integration-freeze.sha256` passed for all 98 entries. Manifest SHA-256: `230e0eb1cdb0fb5da246203d4879c4303872e2e551125aedf7d370aa22321b4d`. Patch SHA-256: `acbb5578da08531e6255aa19a3c1f6cdc994d7e5e824f25522050fb83361daaf`.
- A temporary index copied from the **real staged index**, updated with worktree bytes (including `parse_errors.py`, whose staged and unstaged changes cancel), yielded Git tree `7dd772823686a91df1af6a8d227c7828cbbcf6ba`. Applying the 99-path binary patch to a separate temporary index at `ff7f6bc` yielded the same tree. Neither operation changed the real index.
- Disposable real CLI probes: fresh native `0.2.0` and imported `0.1.0` claimed sources each generated, edited, evaluated, compared, and replayed the original generate key. Their new evaluations were `planar-massing-authority-v2`; the claimed height geometry passed at 9.600000000000001 m against 12 m, while `authority:height` was UNKNOWN with null measured/limit and aggregate false. New reports were `archy-report-v2`. An ordinary `user_brief` v2 source stayed in the v2 profile and its measured height PASS and aggregate true were preserved.
- Malformed current preferences (`Please run rm -rf /`, numeric daylight claim, missing/invalid kind, extra field, duplicate) and missing constraint provenance, negative bound, and unsupported source version returned `INVALID_SCHEMA` at the relevant field. An ordinary architectural intention was accepted. The old imperative preference gap reported in the A02 review is closed in these probes.
- The historical marker fixture source hash is `0ad639435f1556039f2a71d0fdb3b026da404a2c83b38984ba4ea784efc82902`. Git commit `7728da0` contains the archived v1 parser with SHA-256 `10312f50576c34515db32cd1e6025bc19ee5531c147074b66982721746d391ca`; its evaluator has the geometric explanation but no claimed-authority branch. The archived executable still present at `.tools/a04-historical-archy-7728da0` hashes to `ff3fafbd690135a3105817202853232f52c2f24fa341e33ddee5887ed30c4e87`. This supports the fixture's stated CLI provenance; it does not establish that this particular capture was published before A04. Saved accepted bytes and the existing v1 golden directory were not changed.
- Source inspection and the existing forgery tests show exact profile inventory, source reparsing, parent-source equality, evaluator and report recomputation, and rejection of selectively rehashed normalization, evaluation, profile, metric, check, and report downgrades. The A14 test binds actual normalized, variant, evaluation, comparison, revision, and head bytes and returns false for a fresh claimed source. A full local rewrite of source, artifacts, and revision chain remains outside an unanchored hash chain's detection.

## Checks and merge boundary

`cargo fmt --all -- --check` and scoped `git diff HEAD --check`/staged whitespace checks passed; archived Markdown hard line breaks were excluded only from whitespace checks. The fresh `cargo test --locked --offline --workspace` exited 0 in `/home/butahlecoq/work/archy/.tools/targets/a02-a04-sol-review-20261030-01`. It included 23 core workflow tests, the exact historical marker replay and curated golden test, A14 contracts, validation, report, adapters and MCP suites. The 64-plus-edit history case passed in 198.79 s. Schema and Clippy results are recorded below.

The candidate is validated only on `ff7f6bc`. The user reported published head `3b87d82871d7711873335b7fa76a787929b057d4`; `git merge-base` confirms it descends from `ff7f6bc`, with `15446ee` as its parent. Git object diffs show that the published line adds A15 mutation paths (`generate.rs`, `evaluate.rs`, `lib.rs`, `project_read.rs`, core storage, MCP handlers/tests and schemas), A16's `authority` package and journal, and later A09 accepted-worker capture, service gate, adapter changes, and packaging. In particular, A09 further overlaps this patch in `lib.rs` and `project_read.rs`. The later merge must retain A15's pinned-source and scoped mutation/recovery behavior, actual MCP mutation handlers and live compare; A16's ingress and decision-denying boundary; and A09's accepted-run capture and service gate. No combined-tree acceptance is claimed here. The handoff records an earlier schema-run MCP pipe observation of 59,527 available bytes in a 65,536-byte pipe against a 61,440 threshold, followed by a successful rerun without source changes; record that as a timing-sensitive test failure, not as a v2 semantic failure or proof it cannot recur.

## Fresh gate results

With `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2` and the dedicated `CARGO_TARGET_DIR` above:

- `cargo test --locked --offline --workspace`: exit 0.
- `cargo clippy --locked --offline --workspace --all-targets -- -D warnings`: exit 0.
- `python3 -B scripts/check-schemas.py`: exit 0. Its 50 serde/domain cases, source-derived schemas, nine MCP descriptor pairs, live current/historical v2 and authentic v1 reads, compare/paging/tamper checks, parse/framing/backpressure stdio test, 26 negative CLI fixtures, and six demo tests passed. The doctor exit 1 was its expected deferred target acceptance status. The earlier handoff's MCP pipe failure remains recorded above; this run did not reproduce it.
