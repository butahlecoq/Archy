# Omarchy platform and open-source reuse research

Research date: 29 October 2026. This is the architectural decision basis for implementation, not target-machine test evidence. The supplied release specification remains binding, with the founder's later requirement making Omarchy the primary platform. Windows/WSL are development environments. Actual Omarchy execution and visual acceptance are explicitly deferred until a machine is available.

## 1. Platform baseline: Quattro, not the obsolete desktop stack

Official GitHub releases and the download site identify **4.0.6** as stable; **4.5.0-rc2** is a prerelease. The default `quattro` branch can contain unreleased changes. Use version-tagged 4.0.6 documentation and feature detection, record installed package/version information, and maintain an explicit 4.5 migration test rather than treating the latest branch as installed behavior.

Quattro consolidates the bar, menus, notifications and other desktop services in `omarchy-shell`, a Quickshell process with plugins. Old recipes that edit Waybar, Walker or Mako are inappropriate for this baseline. Package-owned files are under `/usr/share/omarchy`; user overrides are under `~/.config`. Stable tagged docs use Hyprland Lua files, including `bindings.lua`, not older `.conf` recipes. These findings materially change our desktop integration plan.

Sources: [official releases](https://github.com/omacom/omarchy/releases), [stable manual](https://github.com/omacom/omarchy/tree/v4.0.6/manual), [dotfile ownership](https://github.com/omacom/omarchy/blob/v4.0.6/manual/31-dotfiles.md).

Decision: Archy is an Omarchy architectural workspace. Its deterministic Rust libraries can be tested anywhere, but its installation, process containment, desktop integration and acceptance profile target Omarchy. Do not build a separate Windows product or generic desktop compatibility layer.

## 2. Desktop interaction: reuse the shell and existing applications

Use a small `archy`-namespaced Quickshell plugin for current project/run status and opening the report, previews and Blender. Plugin manifests use schemaVersion 1 and declared QML entry points; third-party IDs cannot use the reserved `omarchy.` namespace. Install into user-owned plugin locations. Core workflow stays typed CLI/MCP, so no essential operation depends on a click. A shell panel is a launcher/status surface, not a new viewport or CAD editor.

The shell supports plugin discovery, validation, enable/disable and hot reload. Use theme-provided colors/components rather than duplicating a theme engine. Support optional menu entries and documented, opt-in keybindings; never replace a user's existing bindings or modify package defaults. User config edits need preview, collision detection and reversible backups. A plugin in this monorepo requires an explicit local installation path or a separately packaged plugin root: `omarchy plugin add` expects a manifest at repository root, so do not advertise that command against the Archy monorepo without providing that layout.

Critical boundary: upstream explicitly says third-party plugins execute unsandboxed in the shell process. The plugin is not an authority boundary. It must not execute project-provided commands, grant approval, accept arbitrary paths from worker responses or embed agent output as code. Use fixed argv to the installed CLI and bounded structured output. Surface failures without treating missing status as success.

Sources: [plugin contract](https://github.com/omacom/omarchy/blob/v4.0.6/manual/32-shell-plugins.md), [shell API](https://github.com/omacom/omarchy/blob/v4.0.6/shell/README.md), [theme guidance](https://github.com/omacom/omarchy/blob/v4.0.6/default/agents/skills/omarchy/theming.md).

## 3. Agent workflow: reuse Omarchy's agents and sessions

Omarchy already exposes default-agent selection, agent launchers, generic skills discovery and usage/crash-diagnosis integration. It also includes Herdr configuration and terminal/tmux workflows. Archy should supply a provider-neutral project skill/playbook and stdio MCP server, plus a documented optional Herdr/tmux layout for brief, agent, logs and review artifacts. Do not build an agent scheduler, require an API key, replace the installed agent client or introduce a hosted service.

Omarchy agent launch shortcuts may run clients with permissive approval settings. Archy must enforce scoped mutation and human approval within its own interfaces regardless of client mode. The agent's textual claim to be human is not consent. MCP can propose an approval; a local human interaction or deliberately configured approval mechanism must record it. Existing usage panels belong to Omarchy, not a new token-meter implementation in Archy.

Herdr's current upstream describes persistent terminal sessions and a socket/CLI surface. Keep session orchestration optional and avoid taking a dependency on its changing internal socket protocol in release 0.1. A working MCP client is sufficient for the golden journey.

Sources: [stable AI manual](https://github.com/omacom/omarchy/blob/v4.0.6/manual/17-ai.md), [Herdr](https://github.com/omacom/herdr), [stable Herdr configuration](https://github.com/omacom/omarchy/blob/v4.0.6/config/herdr/config.toml).

## 4. Installation, dependencies and upgrades

Reuse Omarchy's mise-managed development environments and Arch package tooling. Provide project-local Rust/Python pins and task definitions; do not replace global runtimes. Rust 1.99.1 and the resolved Cargo.lock are the current development baseline. Python IFC dependencies need a separate locked environment; Blender runs its bundled Python. Do not install Python packages into Arch's system interpreter or assume Blender's interpreter equals the IFC interpreter.

Provide a reviewed PKGBUILD/install manifest for Archy and a non-mutating doctor command that reports Omarchy version, required executables, worker versions, resource controls, and missing capabilities. Arch's rolling Blender package may differ from the reproducible Blender pin; record and reject unsupported versions instead of silently changing the geometry/render baseline. Exact backend choices and hashes are in `../plans/backend-research.md` and must be revalidated at implementation and deployment.

Omarchy supports post-update and theme hooks. Use hooks only for lightweight compatibility/status refresh; never silently upgrade project schemas, rewrite accepted runs, launch expensive rendering or modify package-managed files during an OS update. A manual doctor invocation remains available if hooks are disabled. Packaging must include license notices and uninstall behavior that preserves projects and user decisions.

Sources: [development tooling](https://github.com/omacom/omarchy/blob/v4.0.6/manual/18-development-tools.md), [hooks](https://github.com/omacom/omarchy/blob/v4.0.6/default/agents/skills/omarchy/hooks.md), [uv](https://github.com/astral-sh/uv), [pacman PKGBUILD specification](https://man.archlinux.org/man/PKGBUILD.5.en).

## 5. Geometry: reuse proven predicates, retain a small explicit model

For release 0.1, use canonical SI coordinates, a closed consistently wound rectangle or L polygon, repeated storey volumes and a rectangular site. Seeded generation must have bounded attempts and deterministic duplicate suppression. Candidate identity must include canonical parameters and algorithm version, separate from unique execution/run IDs. Reject self-intersections; never silently repair a user's geometry.

GeoRust offers planar types, validity/topology, area, bounds and distance algorithms; `geo` 0.33.1 was current in the inspected docs. Reuse it where it reduces custom predicate risk, with exact dependency resolution and independent analytic fixtures. Do not introduce geodetic distances, PROJ, a NURBS kernel or an arbitrary CAD model for local Cartesian rectangles. A small documented deterministic sampler is legitimate product logic; avoid relying on unspecified standard-library RNG/hash iteration order.

Measure area using polygon area, GFA as area times storeys, height as storeys times floor height, and per-edge clearances in the site's actual translated coordinates. Keep length and area tolerances dimensionally distinct. Constraint checks run before rendering, retain failures and represent UNKNOWN explicitly. Target area is a soft objective unless marked hard. Preferences survive input normalization but do not become fabricated numerical metrics or laws.

Sources: [GeoRust](https://github.com/georust/geo), [geo APIs](https://docs.rs/geo/0.33.1/geo/). Scope and exact golden values derive from supplied specification sections 3, 4 and 7.

## 6. Blender: reuse bpy, not a general-purpose code-execution bridge

Use a packaged trusted headless worker, deterministic camera/material/light presets, three views, GLB and checked PNG output. Blender owns modeling/rendering; Rust owns authority, inputs, evaluation and acceptance. CPU rendering is the initial reproducible baseline; GPU acceleration is an explicitly tested optional profile. Record device/driver, engine, samples and color management. Check GLB world transforms and Y-up conversion against Z-up analytic bounds.

`ahujasid/mcp-for-blender` offers live inspection and explicitly exposes arbitrary Python execution. `seehiong/blender-mcp-bridge` offers many live addon-backed tools. Both are useful research/debug references, but neither satisfies Archy's narrow production authority contract as an unrestricted dependency. Do not install their MCP config automatically or relay arbitrary code to them. Reuse Blender itself and its official Python API; keep optional interactive exploration outside the default scoped workflow.

Sources: [Blender API](https://docs.blender.org/api/4.5/), [MCP for Blender and security limitations](https://github.com/ahujasid/mcp-for-blender), [Blender MCP Bridge](https://github.com/seehiong/blender-mcp-bridge). Detailed proposed 4.5.15 LTS commands and verification gates: `../plans/backend-research.md`.

## 7. IFC and inspection: reuse IfcOpenShell and independent readers

Use IfcOpenShell directly for IFC4 project/site/building/storey relationships and schematic per-storey IfcBuildingElementProxy extrusions. Do not call masses walls/slabs or infer construction semantics. Include units, local placements, schematic/draft labels and provenance. Reopen in a fresh process and validate hierarchy, counts, transforms, quantities and generated geometry bounds. Bonsai is an appropriate existing interactive IFC inspection tool; no new BIM viewer is needed.

IfcMCP exposes broad API mutations and ShapeBuilder operations; its documentation is valuable for discovery, but exposing arbitrary API names would broaden Archy's mutation surface beyond nine scoped tools. Keep it optional outside the default production pipeline. IfcTester/IDS can encode schema/semantic expectations; they do not establish local planning compliance. A second engine such as ThatOpen web-ifc can provide independent parsing/mesh evidence; an IfcOpenShell reopen alone is not independent-engine validation.

Sources: [IfcOpenShell](https://github.com/IfcOpenShell/IfcOpenShell), [IfcMCP](https://docs.ifcopenshell.org/ifcmcp.html), [IfcTester](https://docs.ifcopenshell.org/ifctester.html), [web-ifc](https://github.com/ThatOpen/engine_web-ifc). Version/API and conceptual mapping research: `../plans/backend-research.md`.

## 8. MCP, state and reports

Reuse the official Rust MCP SDK (`rmcp`, inspected version 3.8.0) with stdio and schema-derived tools. Do not implement a partial JSON-RPC imitation or add a network listener/OAuth service unnecessarily. Use the same core operations as CLI; mutations carry destination, idempotency key and expected revision. A duplicate must return its original outcome, a stale revision must fail, and concurrent writes must serialize. Tool responses and project descriptions remain untrusted data.

Persist original input, normalized parameters, variants, metrics and artifact metadata in immutable run directories, with atomic publication and explicit state transitions. Hashes prove byte identity, not architectural correctness. Verify both file content and declared hashes before success. Reports distinguish geometry, heuristic ranking, uncomputed daylight/cost and legal/professional limits. Favor deterministic sort order and cite metric/run IDs. Editable alternatives create a new recorded iteration rather than overwriting accepted snapshots.

Sources: [official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk), [rmcp API](https://docs.rs/rmcp/3.8.0/rmcp/), supplied specification sections 4–5 and 8–9.

## 9. Process isolation and reliability on Omarchy

Use unprivileged Linux isolation, not membership in the Docker group. Current Omarchy deliberately avoids granting that group by default. Bubblewrap can supply filesystem/network namespaces, but is only a mechanism: Archy must define the actual allowlist. Permit a read-only packaged worker/runtime and bounded input, a writable fresh staging output, minimal environment and required device access only. No home directory or arbitrary host paths. Missing containment must fail explicitly rather than silently downgrade production execution.

Use systemd user resources/cgroups where available for bounded memory, CPU/tasks and whole-job cancellation; distinguish user-manager capabilities from system services. Timeout must reap descendants and preserve previous accepted artifacts. CPU headless jobs need no compositor socket; GPU access needs a separately tested device profile. Test symlink/traversal/absolute paths, output collisions, malformed responses, huge output, cancellation and crash before manifest publication. Canonical-path prechecks alone do not resolve race conditions; design descriptor-relative/no-follow publication where needed.

Sources: [Bubblewrap security model](https://github.com/containers/bubblewrap), [systemd resources](https://www.freedesktop.org/software/systemd/man/latest/systemd.resource-control.html), [Omarchy security](https://github.com/omacom/omarchy/blob/v4.0.6/manual/48-security.md).

## 10. Recovery is layered, and home data needs its own protection

Omarchy system snapshots restore root, not `/home` or user config. Therefore they cannot replace project revisions, immutable runs, approval history or backups of integration edits. Keep Archy's rollback local to project state; document system rollback only for OS/package breakage. A theme/post-update hook must not imply it protects project data. Existing notification and crash-diagnosis facilities can help surface backend failures, while structured stderr/job logs remain the authoritative Archy evidence.

Source: [stable snapshot manual](https://github.com/omacom/omarchy/blob/v4.0.6/manual/47-system-snapshots.md).

## 11. Reuse decisions and licensing

The repository metadata/readmes were inspected on the research date. Recent activity is evidence of activity, not quality or security. Exact source revisions and dependency licenses must be captured in locks/notices before distribution.

| Project | Decision | Reason and boundary |
|---|---|---|
| Omarchy/Quickshell plugins | Integrate | Native launcher/status/theme surface; MIT Omarchy source, package ownership respected |
| Herdr and tmux | Optional workflow reuse | Sessions already exist; no Archy scheduler |
| Blender bpy | Required backend | Existing scene/render/GLB engine; keep bpy worker license compatible with Blender obligations |
| IfcOpenShell/Bonsai | Required library / optional UI | IFC semantics, geometry and inspection; review LGPL/GPL component boundaries |
| GeoRust geo | Adopt where predicates needed | MIT/Apache-2.0 planar algorithms, analytic cross-checks |
| official rmcp | Adopt | Real MCP lifecycle/stdio and schemas; verify actual license files, not GitHub NOASSERTION metadata |
| uv / mise | Reuse tooling | Locked Python and existing Omarchy runtime management |
| Bubblewrap/systemd | Reuse isolation mechanisms | Must supply/test policy; not a blanket sandbox claim |
| web-ifc | Independent test dependency | MPL-2.0, separate IFC engine; no custom viewer needed |
| Blender MCP / Blender MCP Bridge | Reference, not default runtime | Broad live execution/addon surfaces violate narrow default tool authority |
| IfcMCP | Reference / optional expert tool | Broad mutation API; Archy tools stay bounded |
| FreeCAD / neka-nat FreeCAD MCP / spkane robust MCP | Defer | Precise CAD/FEM and arbitrary macro execution exceed massing release; revisit explicit geometric need |
| Truck | Defer | Apache-2.0 Rust CAD kernel; no demonstrated need for B-rep/NURBS here |
| Sverchok / IfcSverchok | Optional later interactive workflow | Existing parametric node system; GPL obligations, no new node editor |
| Impertio skill package | Reference selected API guidance | MIT metadata, but includes direct Blender code-execution configuration; do not import configuration or trust marketing correctness claims |
| design-os-3d-blender | Reference only, no code reuse | Current README declares PolyForm Shield 1.0.0, source-available rather than an unrestricted open-source reuse grant; mechanical design focus |

Additional inspected sources: [FreeCAD MCP](https://github.com/neka-nat/freecad-mcp), [robust FreeCAD MCP](https://github.com/spkane/freecad-addon-robust-mcp-server), [Truck](https://github.com/ricosjp/truck), [Sverchok](https://github.com/nortikin/sverchok), [Impertio skills](https://github.com/Impertio-Studio/Blender-Bonsai-ifcOpenshell-Sverchok-Claude-Skill-Package), [DESIGN:OS](https://github.com/jangtrinh/design-os-3d-blender).

## 12. Consequences for the delivery backlog

Retain all A01–A20 specification tickets. Add explicit tickets for Omarchy doctor/packaging/mise (A21), native shell plugin and reversible integration (A22), and agent/session playbooks plus deferred target acceptance harness (A23). These implement the founder's platform clarification without introducing a viewport, scheduler or cloud service. Add audit refinements for protected-main evidence, schema fields, preference preservation, immutable edits, evaluate-before-render and dimensioned tolerances.

Develop and review source, fixtures, locks, CLI/MCP behavior and Linux process tests now. Record WSL/Ubuntu and CI results honestly. Prepare but do not claim completion of actual Omarchy install, plugin validation/hot reload/theme behavior, Hyprland launch bindings, systemd/bubblewrap policy, GPU previews, clean-machine golden journey or visual IFC review. Those remain named deferred gates; no fake success, automatic test waiver or surprise Omarchy install on this Windows PC.

### Geometry implementation references checked after initial research

Geo0.33.1 exposes Validation::check_validation and Area::unsigned_area. rand_chacha0.10.0 documents deterministic portable streams; prefer a named pinned RNG over platform-default randomness. sha2 0.11.1 provides SHA-256 for parameter/artifact identity. These are implementation candidates, not proof of compatibility until Cargo resolution/tests pass. Official references: https://docs.rs/geo/0.33.1/geo/algorithm/validation/trait.Validation.html ; https://docs.rs/geo/0.33.1/geo/algorithm/area/trait.Area.html ; https://docs.rs/rand_chacha/0.10.0/rand_chacha/ ; https://docs.rs/sha2/0.11.1/sha2/ . Exact task contracts and analytic fixtures are in ../plans/task-02.md.

### Storage research for the subsequent run/persistence slice

The standard Rust1.99.1 File API includes advisory lock/try_lock/unlock; process-shared locking need not add an obsolete locking crate. Capability-relative directories in cap-std4.0.3 provide a reusable basis for project-scoped operations. tempfile3.27.0 offers persist_noclobber, but its documentation discusses path-reopen/cleanup races and platform limits; it is not by itself a complete transactional or symlink boundary. Prefer descriptor-relative access and explicit no-overwrite publication; make crash recovery, duplicate-key replay and manifest revision rules concrete in the owning task. Sources: https://doc.rust-lang.org/std/fs/struct.File.html#method.lock ; https://docs.rs/cap-std/4.0.3/cap_std/fs/struct.Dir.html ; https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist_noclobber . These are researched mechanisms, not implemented/validated guarantees.
