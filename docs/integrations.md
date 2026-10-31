# Candidate integrations

These projects are starting points for investigation, not installed dependencies or commitments. Their capabilities, setup, compatibility, licenses, and safety properties need verification before use.

## Omarchy considerations

The full local workflow has been run on Omarchy with Python 3.14.8, PyYAML 6.0.3, Blender 5.2.2 LTS, and IfcOpenShell 0.9.0post1. It generates and independently verifies an IFC4 massing model in a separate Python process from Blender. The local MCP server uses stdio: evaluation and project creation accept brief text, while project operations use UUIDs and a configured storage directory instead of caller-supplied paths. The tested IfcOpenShell runtime and its Python dependencies were placed on `PYTHONPATH`; package installation through the system Python remains unavailable because it has no `pip` module. Blender MCP, IfcMCP, GUI operation, and their security properties remain unverified. Keep IFC generation in a separate worker environment rather than assuming packages installed for system Python are visible to Blender's embedded Python.

## Modeling and BIM

- [Blender MCP](https://github.com/ahujasid/mcp-for-blender) connects an agent to Blender for scene manipulation and rendering. Investigate its operations and the risks of arbitrary Python execution.
- [IfcMCP](https://docs.ifcopenshell.org/ifcmcp.html) provides an agent interface to IFC models in the IfcOpenShell ecosystem. Investigate model editing, persistence, semantic coverage, and opportunities to contribute upstream.
- [FreeCAD MCP](https://github.com/neka-nat/freecad-mcp) is a candidate for precise CAD operations after the initial massing study.
- [Blender MCP Bridge](https://github.com/seehiong/blender-mcp-bridge) is another Blender integration to compare for explicit modeling and inspection operations.
- [FreeCAD robust MCP server](https://github.com/spkane/freecad-addon-robust-mcp-server) is another candidate for precise CAD workflows, including its advertised Linux embedded mode.

## Related work

- [Blender, Bonsai, IfcOpenShell, and Sverchok agent skills](https://github.com/impertio-studio/blender-bonsai-ifcopenshell-sverchok-claude-skill-package) combines agent instructions with architectural tooling.
- [Design OS 3D for Blender](https://github.com/jangtrinh/design-os-3d-blender) explores agent-oriented procedural modeling and validation.
- [CAD-Assistant](https://arxiv.org/abs/2412.13810) studies an agent workflow using CAD tools and geometric feedback.

Archy's proposed contribution is the architecture-specific workflow, including explicit constraints, comparison, and BIM deliverables. The existence of related projects does not establish that the approach is unique.
