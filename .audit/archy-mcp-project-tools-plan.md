# Persistent project tools over MCP

## Goal

Allow a local MCP agent to create, read, revise, run, inspect, and promote
Archy projects while keeping all persistent files inside one server-configured
storage directory. MCP arguments contain brief text and generated IDs, never
filesystem paths.

## Chosen boundary

- Configure storage at server startup with `--projects-dir` or
  `ARCHY_PROJECTS_DIR`; default to `$XDG_DATA_HOME/archy/projects`, then
  `~/.local/share/archy/projects`.
- MCP-managed projects are immediate child directories of the configured
  store. Project IDs are resolved from validated project metadata, not from
  caller-provided path fragments. Tool calls accept project/revision IDs only.
- Register project lifecycle tools through `archy/mcp_projects.py`; reuse the
  project domain API, returning explicit metadata without local paths.
- Keep revision promotion compare-and-swap through
  `expected_current_revision_id`.
- Preserve the temporary preview tool. Persistent study runs return bounded
  comparison data and previews while leaving the complete verified bundle in
  the project history.
- A cancelled persistent run is allowed to finish and publish its durable
  result before cancellation is propagated; a later history call can discover
  the run.

## Tool surface

- `list_archy_projects()`
- `create_archy_project(brief_yaml)`
- `read_archy_project(project_id, revision_id=None)`
- `create_archy_revision(project_id, brief_yaml, message, parent_revision_id=None)`
- `run_archy_project_study(project_id, revision_id=None)`
- `get_archy_project_history(project_id)`
- `promote_archy_revision(project_id, revision_id, expected_current_revision_id)`

## Invariants

- No MCP call can select an arbitrary input, output, or project path.
- The configured store is the only persistent project root used by MCP.
- Project and revision IDs are validated before resolution; project symlinks,
  mismatched IDs, and corrupt metadata are rejected.
- Tool responses omit host paths. Errors redact the configured store path.
- Input briefs retain the existing 1,000,000 UTF-8 byte bound. Persistent runs
  retain the six-alternative cap and existing preview size limits; successful
  calls return the comparison and available PNGs.
- Run execution does not block the MCP event loop. Completed or failed attempts
  remain available through history.

## Throughput checkpoint

- Blocking first steps: choose the store-root boundary, tool signatures, and
  safe ID-to-project resolution before code.
- Independent workstreams: one implementer owns `project.py`,
  `mcp_projects.py`, and any media change; the other owns server startup,
  prompt guidance, and documentation.
- Shared mutable state: implementers write disjoint files. Runtime projects
  have unique UUID directories; only a project's current pointer is mutable,
  guarded by existing compare-and-swap promotion.
- Smallest safe decomposition: keep storage configuration in the MCP server,
  lifecycle adapters in `mcp_projects.py`, and persistent invariants in the
  existing project API.

## Review and proof

Use two GPT-6-Luna medium reviewers: one checks spec/behavior and one checks
standards/boundaries. Include a comment review in both assignments. Then use
the installed MCP SDK over real stdio to create, list, read, revise, run,
inspect history, and promote a fresh project in a temporary store. Check that
tool responses contain no filesystem paths and that the successful run bundle
passes the existing `archy-verify` command. Do not add or run automated tests.

## Work log

- [x] Independently compared managed storage with MCP Roots; selected the
  server-configured store and ID-only calls.
- [x] Implement the MCP tools and startup configuration in disjoint modules.
- [x] Review behavior/spec and standards/boundaries with two reviewers; fixed
  the UUID directory identity mismatch and both reviewers confirmed the fix.
- [x] Verify the full tool lifecycle over MCP stdio and inspect the resulting
  project bundle.
- [x] Keep this local work uncommitted; no PR was requested.
