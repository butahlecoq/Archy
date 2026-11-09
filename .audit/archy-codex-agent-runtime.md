# Codex MCP agent runtime verification

## Goal

Verify that a real Codex agent can use Archy's MCP guidance, resources,
evaluation, preview, persistent project, and revision tools to complete a
massing-study workflow on Omarchy.

## Runtime and setup

- Codex CLI: `0.165.0`
- Model: `gpt-6-luna`, medium reasoning
- Transport: configured local stdio MCP server at `.venv/bin/archy-mcp`
- Approval behavior: `approval=never` denied the first tool call. The successful
  headless run used `codex exec --approve-for-me`; the CLI reported
  `approval: on-request` and `sandbox: workspace-write`.
- Persistent store: explicitly scoped to
  `/tmp/archy-codex-mcp-e2e-3/projects` using the per-invocation Codex config
  override `mcp_servers.archy.env.ARCHY_PROJECTS_DIR`.

## Product changes

- Codex CLI v0.165.0 did not present the standard MCP `draft_massing_study`
  prompt as an agent-callable tool, though it exposed resources and tools.
  Archy now preserves the standard prompt and offers the same text through the
  read-only, idempotent `get_massing_study_guidance` tool.
- The shared guidance distinguishes the required nonempty brief-level
  `project.id` from the persistent UUID `project_id` created by Archy. It tells
  agents to preserve the brief-level ID when they create revisions, even when
  the project name changes.
- `docs/agent-tools.md` records the tested Codex configuration and headless
  approval behavior. The parent shell's `ARCHY_PROJECTS_DIR` did not reach the
  configured MCP child in the first run; the per-invocation `-c` override did.

## End-to-end result

The agent used the guidance tool and both massing resources, evaluated and
previewed three alternatives, created a temporary project, retrieved a saved
study with verified artifact integrity, and evaluated and promoted a revision.
All three alternatives passed the setback and area constraints. The third
option touched the east buildable boundary exactly; the rotated alternative
used a 15-degree plan rotation.

| Artifact | ID or location |
| --- | --- |
| Persistent Archy project | `cb505494-b562-4127-bcf2-d1495ee435f1` |
| Brief-level project ID | `massing-e2e-temporary` |
| Initial revision | `ace5527c-6b63-4596-9070-3b5bdad8c1c0` |
| Verified study run | `2003f4da-d52b-4b0e-96b6-c6cf4106659c` |
| Promoted revision | `12b57070-3015-4f9e-b13b-2265d127f3ab` |
| Temporary project store | `/tmp/archy-codex-mcp-e2e-3/projects` |

`archy-verify` passed on the saved run bundle. `current.json` records generation
2 and the promoted revision ID. The default user project store was empty after
the run.

## Integration discovery and cleanup

The first CLI attempt used `approval=never` and the evaluator call was rejected
as requiring approval. A later attempt succeeded with `--approve-for-me`, but a
parent-shell `ARCHY_PROJECTS_DIR` value did not reach the configured MCP child.
That run created one test project in the default store. Its exact UUID was
identified, its artifacts passed `archy-verify`, and its directory was moved
intact to `/tmp/archy-codex-mcp-e2e-2/projects`. A final run used the explicit
per-invocation server environment override and stored its project only in the
temporary location above.

## Reviews and checks

Two implementers added the guidance tool, corrected the brief/persistent ID
distinction, and documented Codex setup. Two reviewers found and resolved the
brief-ID ambiguity, the CLI `-c` syntax ambiguity, and the need to preserve the
brief ID across revisions. They found no remaining issues.

Python compilation and `git diff --check` pass. No automated tests were added
or run. All changes remain local and uncommitted.
