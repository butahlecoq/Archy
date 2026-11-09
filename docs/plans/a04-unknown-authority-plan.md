# A04 missing authority evidence plan

Base: `ae6e6a6`; isolated detached worktree. No dependency or input format change.

## Case and decision

The supported courtyard brief omits `authority`; v1 normalization preserves this as `assumption`. Geometry can be measured, but no verified legal authority exists. Ordinary `user_brief` constraints do not imply a legal claim. An explicitly declared `source: claimed_regulation` uses the existing provenance field to request legal authority verification; its geometry retains PASS/FAIL and a separate required `authority:<constraint-id>` check is UNKNOWN because no verified evidence model exists. An authority string names a claim and is never verification. Invalid geometry stays an error. This is the smallest new semantic profile on the existing input model; it requires no new field or migration while A02 v2 is under review.

Version evaluation behavior and report schema as v2 when `claimed_regulation` appears. Ordinary runs stay v1 byte compatible. Replay selects the historical v1 evaluator and renderer for stored v1 artifacts. The generation algorithm and variant identity do not change. Empty historical runs remain verifiable. No automated path in this slice upgrades declarations to verified law.

## Work

1. Write failing evaluator and report tests for an explicit regulatory claim, ordinary user constraints, geometric failure, and malformed geometry.
2. Add versioned evaluation dispatch and explicit v2 check explanation. Keep v1 bytes unchanged.
3. Version report output and route trusted replay by stored evaluation/report version. Reject inconsistent versions and rehashed forgeries.
4. Exercise real CLI runs, exact v1 fixture replay, targeted suites, schema sync, and formatting on a unique SSD Cargo target.

## Overlap and review

A02 v2 is under separate review. This slice does not edit project parsing, normalization, or v2 input types. Likely integration overlap is the shared core check/evaluation schema and CLI history dispatch. Freeze changed files and validation in a handoff for independent Sol review.
