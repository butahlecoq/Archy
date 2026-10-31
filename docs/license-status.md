# License decision

The coordinator selected MIT for original Archy Rust/core code on 2026-10-29 under the authorized implementation. LICENSE grants that license; package metadata records it. Third-party obligations remain intact.

Original Blender and IfcOpenShell workers and the independent IFC-reader harness now exist in the local integration tree. Runtime distribution must separately account for Blender GPL and the applicable IfcOpenShell component licenses; the core MIT license does not relicense these tools, supplied documents, dependencies or names. The core package foundation does not bundle either backend runtime or a development virtual environment. No code was imported from researched broad Blender MCP bridges. Trademark/name availability remains unresolved.

The package handoff records 89 candidate registry packages from the current CLI normal-dependency graph, with metadata license expressions and four missing top-level notice-file cases. This is a partial inventory, not a completed license/text/source audit or distribution approval. Complete the actual shipped binary, worker, runtime, reader and asset notice/source inventory before release; see `docs/plans/package-handoff.md` and the backend handoffs.
