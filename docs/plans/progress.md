# SDD ledger — plan: docs/plans/backlog.md

## State

Base upstream: 155df35. Workspace: C:/work/archy/repository. Branch: feat/release-0.1. Full supplied release baseline retained in docs/specs/release-0.1.txt. All A01-A20 remain open until real acceptance evidence exists. Local ticket definitions are written; GitHub has zero issues as observed through REST. No issue publication or push claimed.

## Preflight rulings

| Tasks/interfaces | Inspection/ruling |
|---|---|
| A01/A02 workspace and shared types | Implement core+CLI only initially; reserve future crates in docs. No empty placeholder implementations. Exact public types recorded by implementer. |
| A02/A03 normalized dimensions | Unsuffixed values obey declared units; SI aliases accepted only under m, ambiguous duplicates rejected. Explicit translated rectangle coordinates retained. |
| A02/A04 constraint validation | Malformed/contradictory input fails early; valid variants violating constraints remain retained and evaluated. |
| A03/A04/A05 variant metrics/run data | Shared deterministic IDs, schema and algorithm version; evaluator consumes canonical footprints. Final signatures follow first slice handoff. |
| A05/A07/A09 persisted run and worker protocol | Snapshot immutable normalized inputs and params; worker cannot choose authoritative paths. Artifact verification owns success. |
| A06/A07/A08/A10 Blender | Pin version before production worker and require real target execution; native Windows limitations do not waive Linux acceptance. |
| A09/A11/A12/A13 adapter and IFC | Typed process boundary reused; IFC masses are proxies, independent reopen verifies hierarchy/bounds. |
| A14/A15 approval/revisions | Local drafts allowed; MCP cannot self-approve. Concurrent mutations require revision/idempotency checks. |
| A09/A15/A16/A17 safety and reliability | Sandbox/path/timeout/integrity controls required in owning slice and adversarial hardening; no acceptance deferred by mocks. |
| A18/A19/A20 docs/release | Document actual tested state; clean Omarchy install, visual checks and all acceptance gates still required. |
| A01-A20 task internal checks | Each backlog item lists intended behavior, file scope, tests, dependencies and evidence; none declares completion. A01 lockfile requires actual resolution, cannot be invented. A02 snapshot-per-run belongs to A05 when runs exist. |

## Work

- spec_backlog (Sol medium): full backlog and task-01 brief written, no remote issue publication.
- toolchain (Sol medium): official Cargo downloaded and SHA-256 verified, execution blocked by Application Control; WSL E_ACCESSDENIED. See ../.tools/toolchain-status.md outside repository.
- core_validation (Sol medium): implementing D1-D2; independent review pending.
- backend research underway with Sol medium.

## Validation blockers

GitHub CLI reports unusable authentication and returns no credential; Python HTTPS public clone/read works. Native Git HTTPS fails Windows TLS. Rust binary execution is denied by machine policy. No Rust compile, format, lint, tests, lock resolution or Linux runtime gates have passed. Keep goal active while useful implementation/review work remains.

License ruling: coordinator selects MIT for original Rust/core under authorized implementation; separately applicable worker/dependency licenses are required. No extra founder approval gate for this routine repository choice.

## Current-state correction (29 October 2026)

See current-handoff.md: GitHub ADMIN authentication and WSL Rust 1.99.1 now work. Cargo.lock resolved; build/clippy passed, tests and formatting require fixes. The older blockers above are historical. Comprehensive Omarchy/reuse research is recorded in ../research/omarchy-and-reuse.md. Original agents are no longer active. Task-01 reviewed findings remain open. SI suffix ruling is superseded by explicit-SI-always-SI; generic units still normalize and duplicate representations fail. No target Omarchy actions are authorized for execution on this Windows host; prepare them and defer target acceptance.

## Verified progress after task01

- Tracker publication complete:26 remote issues, independent review PASS (b9e436f), stable body-marker reconciliation corrected. Main protection and draft PR27 established.
- Task01 fix round1: original six findings addressed in59d3c90; independent review found P2 scalar-looking init IDs (573efce).
- Task01 fix round2/5: P2 corrected in c11157b with red-green and scoped regression checks. Final independent review PASS13f8f4d; no open task01 review findings. Code CI40877097234 and40877108347 passed.
- Task01 slice complete (5b8622a..c11157b product changes, review clean). Broader A01/A02 integration criteria stay tracked; this is not a full release claim.
- Task02 dispatched to massing_evaluation (Sol medium) at base13f8f4d; task-02.md includes canonical shape/derived footprint consistency. All other release requirements retained.
