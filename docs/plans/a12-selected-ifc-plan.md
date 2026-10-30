# A12 selected IFC export plan (2026-10-30)

Base: published `160787d1f6c65a85d0e022a111d56ee7b0eb2b86`; isolated `a12-ifc-selected-20261030` worktree. No commit, push, install or child agent.

1. Reconcile the reviewed A09 current-head patch with the newer A15/A02 base. Preserve A09's fixed service, teardown and capture contract. Review overlap in `project_read.rs` explicitly.
2. Bind `export-ifc --project --variant` to one verified accepted generation/edit run and pinned project source. Recompute the evaluation, retain its failed/UNKNOWN checks, and use the real IFC request and service. No approval assertion.
3. Publish only after verified worker success, using `PendingRun` immutable writes. Store `model.ifc`, `validation.json` and a design manifest with source/run/variant, metrics, digests, versions, DRAFT and schematic labels. A failed attempt leaves prior runs intact and the retry key usable.
4. Exercise rectangle/L, placement and no detailed elements with the pinned worker and independent reader. Test invalid paths/hashes/formats, replay, and prior-run preservation. Use an isolated harness for development if the fixed installed gate is unavailable; record target-dependent gates.

Risks: A09 is reviewed as a patch, not installed; 160787d includes later CLI reader changes. The installed fixed gate is absent in WSL. Keep target acceptance explicit and preserve provenance rather than simulate a successful service.

Dependency rationale: reuse existing Rust core/storage, A09 adapter and Python IFC worker; no new crate or runtime dependency. The independent web-ifc reader is test-only.
