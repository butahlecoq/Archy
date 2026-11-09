# A23 signed harness repair: independent review handoff

## Scope and trust boundary

This bounded patch starts from `653f04619e614939d961bf05b6ca5c8c1b1b618c` in the A23 harness worktree. The previous independent rereview found a real detached signature could promote a forged all-pass ledger with a zero candidate revision and one text artifact, replay after an Omarchy package revision change, accept a false reviewer name, and miss matrix predicate edits with unchanged IDs. Its review verdict was **issues**; that earlier review does not approve this repair.

The repaired verifier pins the reviewed matrix content, verifies the GPG `VALIDSIG` fingerprint against an administrator-owned fingerprint-to-human policy, checks current source HEAD/tree and tracked cleanliness, spec/matrix/compat bytes, source archive, package, locks, worker artifacts and versions, installed `/usr/bin/archy` hash, exact installed `archy`/`omarchy`/`omarchy-dev` revisions, and an administrator-owned per-installation transaction marker. Every gate gets a distinct hashed manifest bound to that target and named predicate checks; the signed human disposition gives a reason for every passing predicate. The script checks structure and bytes, **not** whether a command log, visual note, approval event or reader output is truthful. A20 must independently inspect the evidence and protect its private signing key. Actual native acceptance remains deferred until that occurs.

The marker is a root-owned JSON file at `/etc/archy/acceptance-transaction.json` containing a new UUID `transaction_id` for each installation or upgrade. The authorized reviewer policy is `/etc/archy/acceptance-reviewers.json`, mapping uppercase full GPG fingerprints to human IDs. Both are read-only to this harness; no system files or keys were installed by this patch. The target worksheet describes the record schema and gate evidence requirements.

## Validation on the WSL development host

- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/acceptance/omarchy -p 'test_*.py' -v`: 16/16 passed. New tests generate a disposable Ed25519 private key, sign real detached records with `/usr/bin/gpg`, and verify via real `/usr/bin/gpgv` against a private scratch keyring. Native host, package and git observations are modeled only inside the tests. Covered attacks: one-text forged evidence, reviewer/fingerprint mismatch, package revision replay, changed candidate commit, binary/source archive/package/lock/worker/spec/matrix/evidence/transaction bytes, missing reviewed predicate, and false independent-review field. The positive fixture reaches `target_acceptance: pass` only under modeled native preconditions and a disposable test reviewer policy; it is not target evidence.
- `python3 scripts/validate-omarchy.py --dry-run`: 27 deferred gates, null source revision, deferred target acceptance.
- `python3 scripts/validate-omarchy.py --probe-readonly`: four bounded fixed-path probes, deferred target acceptance on WSL.
- `git diff --check`: clean. Existing hostile-`PATH` tests remain green.

The rereview's signed forgery was reproduced as an executable attack fixture before repair through its private scratch script. The added regression test constructs the same one-text all-pass shape with a real private signing key and now rejects it. No native Omarchy transaction, backend golden journey, independent IFC read, human approval, visual inspection or authorized A20 signature occurred here.

## Review and integration obligations

Review the `target_acceptance` branch and the limit of the `independent_human_review` attestation carefully: a signature cannot establish the physical independence or honesty of the signer. Verify the administrator marker rotation procedure and package/source provenance on the actual target. The G04/G08/G09/G10/C06 worksheet explicitly defers candidate-specific backend and human procedures until executable commands, expected outputs and native artifacts exist; fixtures do not pass those gates.

The published PR head advanced from `e61c977` to `a9c91d4da8b37c37f82232f5cc93cb62c8ab884f` during this repair, adding A02/A04 canonical v2 and historic v1 authority migration. Integration must preserve and rerun those contracts, update this harness's reviewed matrix hash and predicate inventory if the final matrix changes, and obtain fresh independent review on the integrated tree. The earlier e61-based review and this worktree's WSL tests do not cover the new published head. Do not close A19, A20, A23 or issue #23 from this handoff.
