"""Register prompts that guide an agent through a massing study."""

from typing import Any


def _draft_massing_study_guidance(project_name: str, site_details: str, design_goals: str) -> str:
    return f"""Help prepare a version 1 Archy massing brief and compare its alternatives.

Project name:
{project_name}

Site details supplied by the user:
{site_details}

Design goals supplied by the user:
{design_goals}

First check whether the site details give the site width and depth in metres,
all four setbacks (north, east, south, and west), the maximum footprint area
in square metres, and the target gross floor area in square metres. A maximum
building height in metres is optional: include it only when the user supplies
one. If a required fact is missing or unclear, ask the user before drafting a
brief; if a supplied height limit is unclear, clarify it before using it. Do
not invent site dimensions, setbacks, area limits, or code rules.

When the required site facts are clear, read the
`archy://massing/brief-schema-v1` and `archy://massing/operations-v1` MCP
resources before drafting. Use their field names and measurement rules.

Draft three distinct rectangular massing alternatives that respond to the
design goals. State any proposed floor counts, floor-to-floor heights,
footprint dimensions, and locations as design assumptions for the user to
review. Give the required brief-level `project.id` a nonempty value: prefer a
user-provided ID, or use a simple path-safe slug from the project name. This is
separate from the persistent Archy `project_id`; do not derive that UUID or a
filesystem path, because Archy creates the UUID and owns storage paths when
`create_archy_project` is called. When revising an existing Archy project,
preserve its original brief-level `project.id` even if the project name
changes; the project store enforces this across revisions. Use metres, square
metres, and the version 1 coordinate system: the southwest site corner is the
origin, X points east, Y points north, and Z points up. Footprints may include
an optional `rotation_degrees` in `[0, 360)`, measured counter-clockwise from
east about the local lower-left corner at (`origin_x_m`, `origin_y_m`). Omit
the field for an axis-aligned footprint. Keep footprints inside the setback
rectangle where possible, and use the evaluator's polygon-based setback result
for rotated alternatives.

Build a complete version 1 YAML brief with schema_version and contract_version
set to 1. Include the project, units, coordinate_system, site, and alternatives
fields required by the massing brief schema. Call `evaluate_massing_brief`
with that YAML. Use its measurements and named violations to compare the
alternatives. If the brief is malformed, correct it and call the tool again.
Then call `preview_massing_study` with the same brief. Inspect each returned
image and comparison result. Revise the design only when the images or
measurements reveal a mismatch with the supplied goals. Preserve failed
alternatives in the comparison and explain what failed instead of hiding or
silently changing a user goal.

After the user has reviewed the proposed brief and wants to keep it, call
`create_archy_project` with the complete YAML. Save the returned `project_id`
for later calls. Use `run_archy_project_study` to create a persistent study,
`read_archy_project` to retrieve a brief revision, and
`get_archy_project_history` to inspect revision and study metadata. Propose
changes with `create_archy_revision`; promote one only after review and with
the current revision id returned by history as
`expected_current_revision_id`. `list_archy_projects` lists available project
ids. To reopen a saved study, select a run UUID from history and call
`get_archy_project_run` for its saved comparison and previews. Never pass
filesystem paths to these tools; the server stores projects under its
configured projects directory. Persistent project operations are
available through these eight tools: `list_archy_projects`,
`create_archy_project`, `read_archy_project`, `create_archy_revision`,
`run_archy_project_study`, `get_archy_project_history`,
`get_archy_project_run`, and
`promote_archy_revision`.

Return the YAML brief, a concise comparison, and the assumptions that need
user review. These constraint checks describe only the supplied brief. They do
not establish building-code compliance, structural safety, or construction
readiness."""


def register_prompts(server: Any) -> None:
    """Add MCP prompt and tool interfaces for massing-study guidance."""
    from mcp.types import ToolAnnotations

    @server.prompt()
    def draft_massing_study(project_name: str, site_details: str, design_goals: str) -> str:
        """Draft and evaluate a rectangular massing study from supplied project facts."""
        return _draft_massing_study_guidance(project_name, site_details, design_goals)

    @server.tool(
        name="get_massing_study_guidance",
        title="Get Massing Study Guidance",
        description=(
            "Return the massing-study drafting instructions for agent runtimes that cannot call MCP prompts. "
            "Retrieve this guidance before drafting alternatives."
        ),
        annotations=ToolAnnotations(
            readOnlyHint=True,
            destructiveHint=False,
            idempotentHint=True,
            openWorldHint=False,
        ),
    )
    def get_massing_study_guidance(project_name: str, site_details: str, design_goals: str) -> str:
        """Return the same read-only guidance as the `draft_massing_study` MCP prompt."""
        return _draft_massing_study_guidance(project_name, site_details, design_goals)
