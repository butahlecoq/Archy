# Archy study context feature plan

## Feature playbook

1. how over the affected subsystem.
2. architect for parallel design exploration.
3. Write the throughput checkpoint as four todo items. A dimension that genuinely does not apply (single file, no fan-out) keeps its item with `n/a: <reason>` rather than being dropped:
   - Blocking first steps. Gates run before fan-out.
   - Independent workstreams. Disjoint files, services, or layers parallelize. Shared writes serialize.
   - Shared mutable state. Default to splitting the target (the principle-separate-before-serializing-shared-state principle skill). Serialize only for real invariants.
   - Smallest safe decomposition. If one worker is best, name why.
4. Delegate code-writing to a subagent using your configured feature model (default `grok-4.7-xhigh-fast`) with a specific scope (file paths, named data shape and its organizing structure per principle-model-the-domain, a state machine over scattered booleans, a table/registry over branching, a typed model over repeated shape assumptions, chosen before the delegate writes logic, and success criteria). Pick each subagent's reasoning level and fast or standard speed for the job, per the Subagents section. When the implementation admits multiple valid shapes (error handling, abstraction layer, test structure), delegate via the arena skill instead so the runners surface the alternatives and the cross-judge guards the pick. Mandatory: no skip-with-reason escape, and Laziness Protocol does not override it (the gain is review separation, not lines saved). A subagent forbidden to spawn satisfies this by owning the diff directly with the same review separation. No "standing by" reply that waits on a nested agent. Comments per Comments. Surgical edits, re-ground against the source for upstream-derived files. Port shared-primitive improvements to all consumers and verify each. Commit liberally.
5. Verify on the matching surface. "Inconclusive" or wrong-surface is not a pass. Flag it.
6. Rebase into small, ordered commits. Stack follow-ups. Use the sequence-verifiable-units principle, building, verifying, and committing each small unit before the next.
7. If the design is contested, `interrogate` before shipping.
8. Run Opening a PR.

## Throughput checkpoint

- Blocking first steps: inspect the current renderer, comparison report, and real generated preview. Done. The current image shows a mass and ground plane without site or setback context.
- Independent workstreams: one implementer owns `archy/blender_scene.py`; one owns `archy/comparison.py`. They consume the existing evaluated `site` and alternative footprint records.
- Shared mutable state: no shared file writes. Both workstreams derive context from the existing report shape. Review starts after both implementations finish.
- Smallest safe decomposition: the two requested implementers map to two separate review surfaces and avoid concurrent edits to the same module.

## Task sequence

- [x] Run the existing workflow and inspect a generated preview.
- [x] Register Archy's local MCP executable with Codex and confirm it is enabled.
- [x] Implement site boundary and setback-envelope context in Blender renders.
- [x] Implement an inline site-plan and setback comparison in the HTML report.
- [x] Review both changes with two independent GPT-6-Luna medium agents.
- [x] Run the real workflow again and inspect the new rendered artifacts and report.
- [x] Inspect an alternative whose footprint touches the buildable boundary.
- [x] Preserve the uncommitted worktree. Do not commit per the prior session instruction.
