# Project revisions and study history

## Goal

Give an architect or agent a persistent project folder where design changes become reviewable revisions, runs retain the exact brief they evaluated, and a user can promote a revision explicitly.

## Chosen model

A project owns immutable revisions and immutable run records. `current.json` is the only shared mutable pointer. Revision and run IDs are generated UUIDs. The massing brief's project ID is domain data and never becomes a path.

```text
project/
├── project.json
├── current.json
├── .project.lock
├── revisions/
│   └── rev-<uuid>/
│       ├── revision.json
│       └── brief.yaml
└── runs/
    └── run-<uuid>/
        ├── run.json
        ├── brief.yaml
        └── [existing workflow artifacts and manifest]
```

`project.json` stores `format_version`, generated `project_id`, the stable `brief_project_id`, and `created_at`. `current.json` stores `format_version`, `revision_id`, and monotonic `generation`. `revision.json` stores `format_version`, its `project_id` and `revision_id`, nullable `parent_revision_id`, `created_at`, `message`, and the exact brief SHA-256. `run.json` stores `format_version`, its `project_id`, `run_id`, `revision_id`, `created_at`, `status`, `brief_sha256`, and result counts or a failure message. A successful run also includes its exact `brief.yaml` and `run.json` in the existing artifact manifest so `archy-verify` can check them.

## Operations

- `archy project init DIR --brief FILE` creates an initial revision and points `current.json` at it.
- `archy project revise DIR --brief FILE --message TEXT [--parent REVISION]` creates a new immutable proposal. The message is required in the CLI and project API. The default parent is the current revision. It does not move the current pointer.
- `archy project run DIR [--revision REVISION]` runs the selected revision, or the current revision by default, into a unique run directory. A failed attempt is recorded as a failed run.
- `archy project history DIR` lists every revision and run, including failures, and marks the current revision.
- `archy project promote DIR REVISION --expected-current REVISION` atomically updates `current.json` only when the expected current revision still matches.

## Invariants

- Existing `archy BRIEF` evaluation and `archy-workflow BRIEF OUTPUT` behavior stay intact.
- Each revision and run has one generated, path-safe ID and is never replaced.
- A run uses a private byte-for-byte brief snapshot and verifies its SHA-256 against the revision.
- A run becomes visible under its final ID only after it is complete. Failed attempts get a complete failure record and no partial workflow bundle.
- Revision and run creation use independent staging directories and atomic rename. They need no shared write lock.
- Promotion uses `.project.lock`, checks the caller's expected current revision, writes a same-directory temporary pointer, and replaces `current.json` atomically.
- Project metadata and managed directories reject symlinks and paths that escape the project root.
- The existing workflow manifest remains contract version 2. Project run metadata is additive and its extra artifacts are hash-listed.
- History checks run records, exact brief snapshots, and their small manifest entries. `archy-verify` checks the full generated bundle and IFC model.

## Throughput checkpoint

- Blocking first steps: agree on project metadata, revision/run records, and `promote` compare-and-swap semantics before code.
- Independent workstreams: `archy/project.py` owns storage and operations; `archy/project_cli.py` owns argument parsing and output; the root agent integrates the command and updates user documentation.
- Shared mutable state: implementation agents write separate modules. Runtime revisions and runs use unique paths. Only `current.json` is shared mutable state, protected by a project lock and expected-current check.
- Smallest safe decomposition: the storage module and CLI adapter can be implemented independently against the contracts above, then wired and reviewed as one feature.

## Review and proof

Use two implementers and two reviewers, all GPT-6-Luna at medium reasoning. Review code and comments in the same two review assignments. Verify through the installed `archy project` command against a fresh temporary project, including revision, run, history, promotion, stale-promotion rejection, and `archy-verify` on the run bundle. Do not change the existing one-shot output manifest contract.

## Feature playbook checklist

- [x] 1. `how` over the affected subsystem.
- [x] 2. `architect` for parallel design exploration.
- [x] 3. Write the throughput checkpoint as four todo items. A dimension that genuinely does not apply keeps its item with `n/a: <reason>` rather than being dropped:
  - [x] Blocking first steps.
  - [x] Independent workstreams.
  - [x] Shared mutable state.
  - [x] Smallest safe decomposition.
- [x] 4. Delegate code-writing to the two configured implementers with separate files, a named project/revision/run model, and success criteria.
- [x] 5. Verify on the matching surface through the installed CLI and `archy-verify`.
- [ ] 6. Rebase into small, ordered commits. `skip: prior user instruction prohibits committing this local work`.
- [x] 7. Challenge the module-boundary concern with the same two configured reviewers; no verified current failure justified a split. The user cap prevents adding the four-agent interrogate panel.
- [ ] 8. Open a PR. `skip: prior user instruction limits work to the local checkout; no PR requested`.
