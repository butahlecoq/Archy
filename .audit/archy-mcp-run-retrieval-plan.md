# Retrieve saved Archy studies through MCP

## Goal

Let an agent reopen a completed or failed study after its original tool
response is gone. The caller supplies only a project UUID and run UUID. A
completed run returns its saved comparison and bounded previews without running
Blender again.

## Grounding

The project store owns immutable revisions and run directories. `project.py`
validates the project and returns `StudyRun` records from `project_history`,
including an internal path for each run. The MCP adapter already resolves
project UUIDs beneath its configured root. `mcp_projects.py` packages a fresh
run by passing its internal `StudyRun.path` to `build_study_result`.
`mcp_media.py` validates comparison shape, render paths, PNG signatures, and
response size, then strips local render and IFC paths. History returns run
metadata but no comparison, measurements, or images, so an agent cannot reopen
an earlier result through MCP. `verify_output` checks manifest hashes,
comparison consistency, geometry bounds, and IFC semantics; the CLI uses this
full verifier. The retrieval design must decide what integrity checks are
needed before it returns old media.

## Required playbook steps

1. `how` over the affected subsystem. [x]
2. `architect` for parallel design exploration. [x]
3. Write the throughput checkpoint as four todo items. [x]
4. Delegate code-writing to subagents using the configured feature model. [x]
5. Verify on the matching surface. [x]
6. Rebase into small, ordered commits. [skip: prior instruction keeps this checkout uncommitted]
7. If the design is contested, interrogate before shipping. [skip: both candidates agreed on full verification and bounded serialization; the root selected the direct lookup on lower read cost and clearer ownership]
8. Run Opening a PR. [skip: this work stays in the local checkout]

## Throughput checkpoint

- [x] **Blocking first steps.** Inspect run state, the bundle verifier, and the media serializer before choosing the retrieval interface.
- [x] **Independent workstreams.** One implementer owns project-run lookup and bundle integrity; the other owns the MCP retrieval adapter and its prompt/docs. Their file lists do not overlap.
- [x] **Shared mutable state.** Retrieval is read-only. Runs are atomically published under unique IDs, so a lookup either sees a completed run or no run.
- [x] **Smallest safe decomposition.** Reuse the `StudyRun` domain record and existing bounded media serializer. Add only a run lookup boundary and MCP tool if the design review confirms this shape.

## Data shape

- Input: `project_id` and `run_id`, both validated UUID strings at the MCP boundary.
- Stored run: existing `StudyRun` with status `completed`, `failed`, or `corrupt`, plus an internal path.
- Output: existing MCP `CallToolResult` containing a path-free run summary, comparison text, optional status note, and bounded PNG content.
- Completed runs require integrity checks before their stored artifacts are returned. Failed or corrupt runs return a safe summary without media. Retrieval never launches Blender or changes project state.

## Architecture synthesis

The two candidates agree on full bundle verification and reuse of the bounded
media serializer. The selected shape adds a single-run lookup to the project
domain API:

```python
run = get_project_run(project_path, run_id)
```

That function validates the run UUID, constructs `runs/run-{run_id}` beneath
the validated project root, checks that the run metadata agrees with its
directory, and returns the existing `StudyRun` record. It uses
`_study_from_dir` rather than scanning every run through `project_history`.
The MCP adapter then checks `run.status`. For a completed run, it calls
`verify_output` in a worker thread before passing the internal path to
`build_study_result`. For a failed or corrupt run, it returns the safe summary
only. The adapter never returns `StudyRun.path` or any artifact path.

The adapter-centered candidate was rejected because it would scan and validate
every run to retrieve one, and would put run-location rules in the MCP module.
Its useful response detail is retained: the retrieved summary reports
`artifact_integrity` separately from the stored run status. A verification
failure therefore does not rewrite history or relabel a completed run.

The arena cross-judge phase is skipped because the configured model pool has
only GPT-6-Luna entries and the user requested that model. The two independent
candidates and the root comparison set the design.

## Review and proof

Use two GPT-6-Luna medium reviewers. One checks behavior against this plan; one checks standards and filesystem/MCP boundaries. Both scan comments and docstrings. Drive the installed MCP server over stdio against the already-persisted run in `/tmp/archy-mcp-projects-st8jiq60/93438658-1164-4d2c-8469-633d351390d7/runs/run-34a5e7b3-c9a9-41d9-87c2-7a00c5c9e391`. Confirm that the tool returns the stored comparison and three previews without invoking Blender, rejects unknown IDs, and exposes no local path. Do not add or run automated tests.

## Work log

- [x] Confirmed that the existing MCP history tool omits past run comparisons and previews.
- [x] Compared two run lookup and integrity designs, then selected the direct project-domain lookup.
- [x] Implement the domain and MCP boundaries in disjoint files.
- [x] Review behavior, standards, boundaries, and comments with two reviewers.
- [x] Recheck failure-detail redaction for quoted and unquoted paths, file URLs, and configured-root boundaries.
- [x] Retrieve the persisted prior run over real MCP stdio and inspect its response.
- [x] Recheck syntax and whitespace after the redaction hardening.
- [x] Keep the work local and uncommitted.
