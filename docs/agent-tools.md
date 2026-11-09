# Agent tools

Archy exposes its deterministic massing evaluator as a local Model Context
Protocol (MCP) server. The server accepts brief text from the calling agent and
returns the same measurements, formulas, constraint outcomes, and source digest
as the `archy` command. The preview tool launches Blender and IFC export in a
temporary child process. Persistent tools manage projects within a configured
storage directory and do not accept arbitrary filesystem paths.

## Set up on Omarchy

Review and trust the repository's `.mise.toml` once, then install its pinned
Python and create the local environment:

```sh
mise trust .mise.toml
mise install
mise run setup
```

The `setup` task installs Archy's MCP and IFC dependencies into `.venv`. It
does not change system Python. The editable install writes ignored
`archy_massing.egg-info` packaging metadata in the checkout. Install Blender
separately and ensure that the MCP host's `PATH` includes Blender.

For Codex, add the server to `~/.codex/config.toml`, replacing the example paths
with the absolute path to this checkout:

```toml
[mcp_servers.archy]
command = "/absolute/path/to/archy/.venv/bin/archy-mcp"
cwd = "/absolute/path/to/archy"
```

Archy uses the XDG data directory by default. In the tested Codex CLI v0.165.0
headless setup, use this command to set a custom store for one invocation:

```sh
codex exec --approve-for-me \
  -c 'mcp_servers.archy.env.ARCHY_PROJECTS_DIR="/absolute/path/to/archy-projects"' \
  "Ask Archy for a massing study."
```

Exporting `ARCHY_PROJECTS_DIR` in the parent shell did not pass it to the
configured MCP process. The `-c` override did.

Other MCP hosts should launch the repository's `.venv/bin/archy-mcp` as a
stdio server using an absolute path. The server exposes three base tools:

- `evaluate_massing_brief` accepts `brief_yaml`, the complete YAML brief as text.
- `preview_massing_study` runs the local workflow and returns comparison data and rendered alternatives.
- `get_massing_study_guidance` returns the brief-drafting and study-review guidance described below.

The evaluation and preview tools limit input to 1,000,000 UTF-8 bytes. The
server also exposes two read-only resources:

- `archy://massing/brief-schema-v1` returns the version 1 JSON Schema.
- `archy://massing/operations-v1` returns the massing operation contract.

The `draft_massing_study` prompt accepts a project name, site details, and
design goals. It directs the agent to ask for missing site facts, draft three
rectangular alternatives, evaluate them, preview the full workflow, inspect the
renders, and report assumptions and failures. When the user wants to keep a
brief, the prompt also explains how to create and manage a persistent Archy
project. Clients that support MCP prompts can use this prompt. Codex CLI
v0.165.0 did not surface it as an agent-callable tool in the tested setup, so
use `get_massing_study_guidance` in that client to retrieve the same guidance.

## Run the tested Codex CLI headless flow

In the tested Codex CLI v0.165.0 setup, `approval=never` denied Archy's MCP
call. Running the isolated flow with `codex exec --approve-for-me` used
on-request approval and allowed the tool call. These results apply to that CLI
version and setup. They do not establish behavior for other versions or
approval modes.

An alternative that violates an area limit or setback is still returned as an
evaluation result with `valid: false` and named violations. Malformed briefs
return a tool error. The server does not change briefs or choose design goals.

The server exposes eight persistent project tools:

- `list_archy_projects()` lists project IDs in the managed store.
- `create_archy_project(brief_yaml)` creates a project and its first revision.
- `read_archy_project(project_id, revision_id?)` reads the current or selected brief.
- `create_archy_revision(project_id, brief_yaml, message, parent_revision_id?)` stores a proposed revision without promoting it.
- `run_archy_project_study(project_id, revision_id?)` runs a persistent study for the current or selected revision.
- `get_archy_project_history(project_id)` returns revision and study metadata.
- `get_archy_project_run(project_id, run_id)` retrieves a saved run's verified comparison and bounded previews. Select a `run_id` from project history; this does not rerun Blender.
- `promote_archy_revision(project_id, revision_id, expected_current_revision_id)` promotes a revision only if the expected current revision still matches.

To reopen a prior study, call `get_archy_project_history(project_id)`, select a
run UUID, then call `get_archy_project_run(project_id, run_id)`. Failed or
corrupt runs return a summary without media. Completed runs return comparison
and preview content only after full artifact verification; if verification or
packaging fails, the tool returns a path-free summary without media. The
summary reports `artifact_integrity` as `verified` or `unverified` and keeps
the persisted run status unchanged.

Project tools accept UUID project and revision IDs, not paths. The server owns
the storage directory; agents cannot choose where project files are read or
written. Set `ARCHY_PROJECTS_DIR` or pass `--projects-dir PATH` to choose it.
By default, Archy uses `$XDG_DATA_HOME/archy/projects` when
`XDG_DATA_HOME` is set, otherwise `~/.local/share/archy/projects`. The server
creates and validates this directory at startup. Back up this directory to
preserve persistent projects.

The transport is stdio for local hosts. The server intentionally does not
expose a network listener or arbitrary filesystem paths. The preview tool runs
in a temporary directory, verifies the generated IFC, returns the comparison
and PNG previews, then removes its files. Limit each preview to six alternatives
and 8 MiB of combined PNG data. Persistent runs are stored in the configured
project directory alongside immutable revisions and run history.
