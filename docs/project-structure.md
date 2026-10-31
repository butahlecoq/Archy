# Project revisions and study history

An Archy project stores immutable design revisions and study runs. The `current.json` file points to the selected revision. A new proposal does not change that pointer. Promotion checks the revision you expect to be current before it updates the pointer.

```text
housing-project/
├── project.json
├── current.json
├── revisions/
│   └── rev-<uuid>/
│       ├── revision.json
│       └── brief.yaml
└── runs/
    └── run-<uuid>/
        ├── run.json
        ├── brief.yaml
        ├── massing.ifc
        ├── comparison.html
        ├── renders/
        └── manifest.json
```

`project.json` stores the project format version, a generated project ID, the brief's stable project ID, and the creation time. `current.json` stores the selected revision ID and a generation number. Each `revision.json` records the project ID, its parent, creation time, message, and brief SHA-256. Each `run.json` records the project and revision IDs, status, creation time, brief SHA-256, and result counts. A completed run stores the exact brief snapshot and the existing version 2 workflow manifest.

The version 1 brief fields and measurement rules are defined in [massing-brief.schema.json](massing-brief.schema.json) and [massing-operations.md](massing-operations.md). Create and inspect projects with the `archy project` commands in the [README](../README.md#keep-project-revisions-and-studies). History checks project metadata and run snapshots. Use `archy-verify` to check every artifact and the IFC model in a completed run. MCP project tools resolve UUIDs under a server-configured projects directory and never accept project paths from the agent; see [Agent tools](agent-tools.md) for configuration. Git remains available for project-level collaboration and backup.
