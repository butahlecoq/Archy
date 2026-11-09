# Archy site context review

Two implementation agents worked in separate files. The Blender renderer adds a gold site perimeter, green buildable envelope, north/east marker, and site-plus-building framing. The HTML report adds an offline inline SVG site plan, accessible title and description, visible footprint dimensions and coordinates, and a viewport that includes an off-site footprint.

The Blender reviewer found that setup could fail before cleanup began, and that a buildable edge might overlap a legal-edge mass. The cleanup block now begins before temporary scene objects are created. The normal and off-site renders show the guides, and the mass stays separate from the context outlines. The legal-edge overlap remains a visual case to inspect with a boundary-coincident brief.

The HTML reviewer found that the SVG used the detailed description as part of its accessible name and that remote footprints could leave the diagram viewport. The SVG now separates `aria-labelledby` and `aria-describedby`, scales the view to include site and footprint, and shows a visible footprint coordinate caption. Chromium rendered both the standard and off-site comparison pages.

The no-comments review flagged the pre-existing `render_alternatives` narration docstring. It was removed because it restated the function's visible behavior.

The standard workflow completed at `/tmp/archy-study-context-verified`. Blender inspection reports `matches_input: true` for all three alternatives, and the workflow completed IFC verification before producing the manifest. The off-site workflow completed at `/tmp/archy-study-offsite`; its render shows the lot and displaced mass together, and its plan shows the full footprint coordinates.

The local Codex MCP server is enabled. A direct MCP stdio call to `preview_massing_study` returned three PNG images with no tool error.

## Boundary-edge follow-up

The legal-edge alternative at `/tmp/archy-edge-brief.yaml` completed the workflow. The perspective and plan views show the footprint touching the west and south setback boundary, while the remaining green outline stays visible. This resolves the reviewer's visibility question without moving the boundary away from its actual coordinates.

## Audit note

The direct MCP result and generated-output paths were checked against this workspace's Codex session transcript. The Codex user config exists and codex mcp list showed Archy enabled. The final normal, off-site, and setback-edge outputs exist at the paths listed above.
