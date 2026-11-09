# A15 proposal transaction plan

Base `8e6c8987431a3c613563345ee38f95f530181b12`. Implements the bounded design in the prior A15 blocker handoff. No authority service is used.

1. Extend accepted history with a read-only lookup of a run's preceding head hash. Add a `propose` verifier branch that checks exact source snapshot, selected variant/evaluation, comparison, A14 `Proposal`, expected pre-proposal revision/head, and no extra artifacts or outcomes. Keep the existing verified-run type so later history reads remain valid.
2. Add a bound project transaction that derives the A14 proposal from a verified generation/edit run, recomputes comparison under lock, and commits via checked recovery/publication. The request key covers source, variant and reason. Exact replay returns the original receipt; conflict and stale revision have no effect.
3. Expose only the typed MCP `propose_approval` tool with a proposal receipt schema. Update agent skill/playbook and MCP notes; retain nine names and the A23 matrix. Add live protocol checks for replay, conflict, stale, path/field errors, accepted history and no approval state.
4. Run focused Rust and protocol tests, schema regeneration/checks, full 27 A23 tests, fmt and clippy. Freeze a local commit and report exact results for independent review. No push or target claims.

Risk focus: proposal pre-head versus post-commit head; an older accepted source run; replay after a later revision; orphan publication; v1/v2 evidence profiles; forged approval fields; source swap.
