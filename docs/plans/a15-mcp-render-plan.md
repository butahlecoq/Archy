# A15 MCP render slice

Base: published `a9c91d4da8b37c37f82232f5cc93cb62c8ab884f`. Own worktree: `.worktrees/a15-mcp-render-20261030`.

Implement `render_variant` with the existing fixed-project `BoundProjectRead` and A10 render transaction. Require source evaluation UUID, one selected variant, `draft` preset, idempotency key, expected revision, and project-relative destination. Return typed run and variant receipts. Preserve worker errors as failed render variants. Keep `export_ifc` and `propose_approval` unavailable; never expose shell, arbitrary paths, or final approval.

Dependency: the provisional A10 candidate supplies bounded service-gate launch, capture, verifier, immutable render record, and CLI render command. Import source separately from the MCP adapter. Reconcile its v1 verifier with a9's canonical v2 and historical v1 profiles. New parser dependencies remain pinned in A10's lockfile; the MCP layer adds no new crate.

Risks: A10 is under independent review and its installed Omarchy service is absent here. A10's integration review identified desktop status/open bugs; the separate integration fixer owns those. The systemic thin-preview source repair is frozen at 44-path manifest SHA-256 `9e596870d265a32489c72b0f44bc0f06bdd10bb37fc6736c10c8becde2598783` in `.worktrees/a10-render-publication-20261029`, pending independent review. Its worker and scene files are imported byte-for-byte; adapter capture/service and render verifier already match. This slice must not claim installed render success. Validate the actual MCP client on the fixed project, no-write invalid requests, fail-closed unavailable receipt, replay/conflict/stale revision, concurrent same-key calls, and tamper rejection. Repeat the positive installed gate on target Omarchy after A10 clearance.
