# A02+A04 integration on ff7f6bc

30 October 2026. Base: `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1`. Isolated detached worktree; no commit, push, install, or child agents.

1. Verify A02 round-two and A04 authority/historical source freezes and reviewed patch identities, then apply the A04 historical patch and A02 frozen staged diff. Resolve report/schema conflicts while retaining A15 compare and the ff process fixture.
2. Bind fresh native v2 and imported v1 inputs to the canonical v2 source snapshot. Use authority v2 evaluation on every new v2 operation; keep required authority UNKNOWN for a claim and keep historical accepted v1 evaluation exact. Generate/edit/evaluate must retain the source profile, operation algorithm, evaluation version, and report format in verifiable artifacts. Reject mismatches during verification.
3. Verify fresh and historical CLI generate/edit/evaluate/compare/replay, rehashed profile downgrade, the historical claimed marker fixture, schema parity, formatting, and Clippy. Keep the known absence of an external trust anchor explicit.
4. Freeze all changed paths, export a base-relative patch with digest, and hand off for independent review. Document overlap with A02 rereview, A09, and later A15 integration without editing their worktrees.

## Reviewer finding and publication policy

The independent review reproduced a fresh edit and evaluation of the accepted v1 claimed marker chain that inherited `planar-massing-v1` and omitted the required authority UNKNOWN check. A fresh child now reparses its verified parent's saved source under the current parser, requires the numeric normalization to match the verified v1 source, publishes canonical v2/profile/evaluation/report bytes, and keeps the old parent run ID and input bytes. A source the current parser cannot migrate returns an actionable field error before staging or revision. The policy is:

| Path | Claimed v1 source | Current v2 source | Non-claimed v1 source |
| --- | --- | --- | --- |
| Fresh sample generation from project source | Current parser, canonical v2 profile, authority UNKNOWN | Canonical v2 profile | Canonical v2 profile |
| Fresh edit or restore by editing saved parameters | Migrate verified v1 source to canonical v2 with same input/parent; authority UNKNOWN, or explicit parse error | Inherit exact verified source and canonical profile; authority v2 | Inherit verified historical source; v1 evaluation |
| Fresh explicit or default evaluation | Same deterministic v1-to-v2 migration and authority UNKNOWN, or explicit parse error | Inherit exact verified source and canonical profile; authority v2 | Historical v1 evaluation |
| Existing idempotency key | Verify request and accepted run, return exact saved receipt and bytes | Same | Same |
| Read-only comparison | Explicit authority-verification error, no passing ranking | Verified source and authority UNKNOWN retained | Verified historical comparison |
| A14 evidence binder | Historical bytes may bind, but claimed checks return false | Required authority UNKNOWN returns false | Existing geometric result |

The child decision uses the verified parent source, not the mutable project file. The only permitted profile transition is a claimed v1 parent to a v2 child with identical source input and numeric normalization. V2 descendants then inherit that source and canonical bytes. The request profile for replay is derived from the verified accepted child, so saved v1 request bytes and migrated v2 request bytes each replay exactly. Verification recomputes source normalization, profile inventory, variants, evaluation, and report, and rejects selective rehashes that downgrade the profile. Authentic v1 artifacts and replay stay byte exact. The verifier must still accept historical v1 runs, so an attacker able to rewrite the entire local source, artifact set, and revision chain can impersonate a historical v1 run. No external anchor or human approval is inferred from local hashes or from a geometric PASS.
