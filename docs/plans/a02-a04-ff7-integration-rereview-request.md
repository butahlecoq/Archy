# A02+A04 ff7 authority-bypass rereview request

30 October 2026. Review the updated base-relative patch on `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1` independently. This request follows the failed Sol review in `a02-a04-ff7-integration-review-sol.md`; it is not a review verdict.

1. Reproduce the authentic two-revision v1 claimed marker fixture. Verify exact saved bytes and unchanged-key replay. Try fresh edit, explicit evaluation, and default evaluation. Each new child must preserve its parent run ID and saved input bytes, publish canonical v2/profile bytes and required `authority:height` UNKNOWN, and report aggregate false. Evaluate the migrated edit and restore its original parameters as further v2 children. Replays of both old v1 and new migrated v2 keys must return the same receipts.
2. Confirm fresh v2 and imported v1 sources generate with the inventoried authority v2 profile and `authority:height` UNKNOWN; v2 edits and evaluations retain source lineage and false required status. Confirm non-claimed v1 history can still evaluate and compare.
3. Compare a historical claimed v1 run. It must return an explicit authority-verification error instead of a passing ranking. Bind its exact saved normalized, variant, and evaluation bytes through A14; claimed checks must return false. Check normal v2 comparison and A14 behavior.
4. Try selective rehashes of normalized, profile, evaluation, report, and revision material. Distinguish verifier rejection from the unavoidable full local chain rewrite limit without an external anchor.
5. Inspect the release-spec and A14 boundaries. A geometric PASS and accepted local history are not regulatory approval. Check the narrow v1-to-v2 parent transition, numeric equality, and refusal before staging if the current parser rejects a legacy source.
6. Review the overlap with published head `1e2313fe0bf20f69f246836334adb761276d2390`: preserve A15 scoped locked mutation/recovery and MCP writes, A16 ingress/journal, A09 accepted-worker capture/service gate, and the subsequent MCP backpressure fix. This patch is validated only on `ff7f6bc`; it makes no combined-head acceptance claim.

Read the updated handoff for exact command outcomes, hashes, and the preserved ENOSPC observation. No child agent was used to produce this request.
