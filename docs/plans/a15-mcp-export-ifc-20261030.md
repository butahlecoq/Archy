# A15 scoped MCP IFC export plan

Base: `3ad404af5ce4753cea02be86d4c54f1aa2d9a7b4` in a dedicated worktree.

1. Reuse the CLI's IFC export transaction and fixed service gate through the MCP server's pinned project reader. Keep source selection explicit, validate UUID, revision, key, and destination at the existing boundaries.
2. Return a bounded typed receipt containing run identity, revision, artifact path, manifest digest, selected source and variant, and explicit `DRAFT` schematic scope. Do not convert backend failure to success.
3. Exercise the stdio protocol with generation, invalid inputs, missing backend, same-key replay when available, stale revision, conflicting reuse, and safe destinations. Compare successful receipt fields with CLI's selected IFC output where a real backend exists.
4. Regenerate the advertised schema fixture, update MCP documentation, run strict fmt, clippy, relevant Rust and protocol tests, then freeze a commit for separate security and compatibility reviews.

Risks: IFC worker and verifier may be absent locally; tests must preserve `BACKEND_UNAVAILABLE` and never forge successful exports. CLI path reopening would lose the MCP project binding, so the export core must accept the already pinned reader. No new dependency is needed. A13 `verify.py`, A23 runtime files, host services, and wider A12 review are out of scope.
