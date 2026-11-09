# Archy 0.1 Implementation Plan and Issue Backlog

> **For agentic workers:** Implement one bounded issue at a time using superpowers:subagent-driven-development or superpowers:executing-plans, as authorized by the user. The requested implementation and review agents use Sol with medium reasoning. Each issue needs an implementation plan, risk note, independent review, and evidence before closure; implementers do not merge their own PRs by default.

**Goal:** Deliver the complete local-first residential massing workflow specified for release 0.1.

**Architecture:** Rust owns versioned project state, deterministic generation/evaluation, typed CLI/MCP operations, provenance, reports and authority. Bounded Python workers run Blender and IfcOpenShell; their files are checked before artifacts are accepted. Structured manifests remain the source of truth.

**Tech stack:** Rust workspace, clap/serde/schemars/uuid as justified and pinned, Python workers, pinned Blender and IfcOpenShell, and a thin MCP facade. Exact versions and licenses must be verified from official sources during the owning ticket, then locked.

**Spec:** [release-0.1.txt](../specs/release-0.1.txt), version 0.1, 29 October 2026, supplied as `C:/work/archy/specification.txt`. Read [design-decisions.md](design-decisions.md) for coordinated decisions. This backlog is a traceable implementation baseline, not a claim that any ticket is complete.

## Tracker and execution rules

The stable local IDs `A01`–`A20` map to D1–D20; `A21`–`A23` implement the founder's Omarchy platform clarification. All 23 release tickets and three explicitly excluded future-roadmap tickets are published in GitHub; see [issue-map.md](issue-map.md) for verified URLs and current tracker evidence. GitHub authentication works with ADMIN permission; the earlier authentication failure is historical. Labels: `release:0.1` or `roadmap`, plus the component labels indicated below. Issue publication is not implementation acceptance or release completion. Dependencies use stable IDs and linked GitHub issues.

On 29 October 2026, the main branch protection API initially returned HTTP 404, “Branch not protected.” After the coordinator configured protection, a fresh API read verified strict required `rust` checks, one approving review, stale-review dismissal, administrator enforcement, and disabled force pushes/deletions. A01/A20 must reverify required checks and review rules against the final CI before release. The founder selected MIT for original core code; A01 must add the license file and A20 must still verify distributed worker/dependency notices and compatibility.

Every ticket follows this checklist:

- [ ] Post or reconcile its GitHub issue; record implementation plan, risks and dependency rationale before edits.
- [ ] Add the named failing tests/fixtures, observe the failure, implement only the stated file scope, and verify the named behavior.
- [ ] Run relevant formatting/lint/tests and attach command output and user-visible evidence to the issue/PR.
- [ ] Obtain an independent review of requirements, diff, security and reproducibility; resolve findings.
- [ ] Record changed files, commands/results, blockers, next action and unresolved decisions in a session handoff; close only when acceptance evidence exists.

Paths below are intended new file boundaries in the initially empty repository. Shared workspace dependency files may change only for the owning component, with rationale. Each implementation plan can refine internal filenames without changing ownership or contracts; cross-ticket contract changes require updating this backlog and affected tests.

## Global constraints and contracts

- “Do not build a new viewport, CAD kernel, foundation model, or general-purpose node editor in Month 1.”
- “Store lengths in meters internally; display units are presentation only.” Local coordinates are right-handed, Z up; v0.1 site is a closed simple rectangle.
- “A run with any UNKNOWN required check cannot be marked compliant or automatically approved.” Checks use `PASS`, `FAIL`, `WARN`, `UNKNOWN`; no report certifies regulatory compliance.
- “Never silently overwrite user design files.” New run IDs, immutable snapshots, atomic writes, append-only decisions and recoverable history bound mutations.
- “Every mutating tool accepts idempotency_key, expected_project_revision, and a project-relative destination.” All adapters use project-scoped destinations and typed requests, never arbitrary prompt-supplied Python.
- “No approved variant exists until explicit human approval is recorded; unapproved exports are tagged draft.” Use draft local exports as the baseline allowed by section 7; record any stricter chosen policy in an ADR. External release/publishing always requires explicit authority.
- Keep generated runs out of Git except deliberately curated small fixtures; large previews belong in CI/release artifacts. Pin dependencies and commit lockfiles.
- Target acceptance is Linux/Omarchy with real pinned workers. Windows development and mocked CI evidence cannot substitute for clean target-machine evidence.

Field-level fixtures in A01/A02/A04/A09/A14 must assert the complete §4 contract: Project id/schema_version/site/brief/constraints; Site polygon_xy_m/north_angle_deg; Constraint id/kind/bound/unit/source/authority; Variant id/seed/footprint/storeys/floor_to_floor_m; Metric name/value/unit/method_version; Check constraint_id/result/measured/limit/explanation; Artifact path/sha256/media_type/producer; Approval variant_id/actor/timestamp/decision. Illustrative aliases are input conveniences, never substitute canonical output fields; missing legal authority is explicitly unknown/unverified.

Shared interfaces to establish in A01/A02: `Project`, `NormalizedSpec`, `Variant`, `Metric`, `Check`, `Artifact`, `Approval`, `RunManifest`, and `ArchyError`. Errors expose stable code, actionable details and explicit retriable flag: `INVALID_SCHEMA`, `INVALID_GEOMETRY`, `CONSTRAINT_FAIL`, `BACKEND_UNAVAILABLE`, `BACKEND_TIMEOUT`, `ARTIFACT_INVALID`, `REVISION_CONFLICT`, `PERMISSION_DENIED`, `INTERNAL_ERROR`. Preserve variant failures separately from process failures; failed rendering never masquerades as successful artifact production.

## Review focus

1. YAML aliases, duplicate keys, unknown fields, NaN/infinity and overflow must not create ambiguous normalized state (A02).
2. Narrow feasible regions, fewer available shapes than requested and coincident seeded samples terminate with explicit evidence (A03).
3. Symlinks, traversal, absolute paths, output collisions and child processes surviving timeout cannot escape a project or damage accepted runs (A09/A16).
4. Repeated/stale concurrent calls and interruption between artifact production and manifest commit cannot double-apply mutations (A15/A17).
5. Approved state, exports and reports cannot be forged through project descriptions, worker output or an agent claiming to be human (A14/A15/A16).

## A01 / D1 — Workspace, schema draft and engineering baseline

**Labels:** spec, core, testing. **Dependencies:** none.

**Problem:** There is no executable repository baseline or documented source-of-truth decision.

**Intended behavior/scope:** Establish the Rust workspace and component ownership, initial versioned JSON schemas, stable error envelope, CI format/clippy/test and Python lint/test jobs. Record ADRs for structured source of truth and typed subprocess workers. Add the founder-selected MIT license for original core code and record separate worker/dependency distribution obligations. Inspect protected-main/ruleset settings, required CI checks and review rules; capture actual settings evidence and record missing protection as an unresolved gate. Configure repository settings only with authorization. Define project/run/revision identifiers and relative artifact semantics before downstream work.

**Files:** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `crates/*/Cargo.toml`, minimal `crates/*/src/lib.rs` / CLI `main.rs`, `schemas/project.schema.json`, `schemas/variant.schema.json`, `schemas/backend.schema.json`, `schemas/error.schema.json`, `.github/workflows/ci.yml`, `.gitignore`, `docs/adr/0001-source-of-truth.md`, `docs/adr/0002-typed-workers.md`, `docs/dependencies.md`, license decision record. No generator/worker implementation.

**Tests:** workspace builds; schema examples validate; error code serialization snapshot; CI executes Rust checks and configured Python checks when workers arrive.

**Acceptance evidence:** passing baseline CI, committed lockfile, schema fixture output, ADRs, dependency/license decision status. MIT core license file and third-party distribution review are separate acceptance evidence; do not claim protected main without verified settings.

## A02 / D2 — Initialize, parse and normalize projects

**Labels:** core, testing. **Dependencies:** A01.

**Problem:** Agents need actionable validation before using design data.

**Intended behavior/scope:** `archy init --name --dir` creates a non-destructive skeleton and schema. `archy validate --project --json` accepts YAML/JSON, returns canonical SI `NormalizedSpec` or structured field errors/nonzero exit. Preserve a bounded, declarative preference field through normalization and snapshots; reject executable preferences and unsupported numeric performance claims. Explicit `_m`/`_m2` input fields are SI; generic dimensions obey declared units; reject simultaneous generic/SI representations. Define schema migration rejection, units conversion, dimensions/storeys/floor height, rectangle coordinates and north angle; constraints retain source/authority. Distinguish target GFA objective from a hard bound. Snapshot original and normalized inputs per run. Unknown regulatory values remain explicit assumptions/UNKNOWN instead of invented law.

**Files:** `crates/archy-core/src/{project,units,error,validation}.rs`, `crates/archy-cli/src/{main,init,validate}.rs`, `schemas/project.schema.json`, `tests/fixtures/briefs/`, `examples/courtyard-housing/project.yaml`.

**Tests:** valid YAML/JSON equivalence; supported unit equivalence; missing fields, zero/negative/non-finite/overflow dimensions, invalid storeys, contradictory limits, nonrectangular/self-intersecting site, unsupported schema/units, duplicate/unknown keys and YAML aliases; init refuses occupied user files. Golden brief is 20×35 m, 3 storeys, 3.2 m, 12 m cap, 3 m setback, 900 m² objective.

**Acceptance evidence:** CLI JSON snapshots and field errors, passing fixtures, normalized golden manifest; input files preserved.

## A03 / D3 — Seeded rectangular and L-shaped alternatives

**Labels:** core, testing. **Dependencies:** A02.

**Problem:** A repeatable agent workflow needs distinct editable massing parameters.

**Intended behavior/scope:** `generate_variants(spec, count, seed)` creates deterministic IDs, immutable parameter snapshots and closed consistently ordered simple footprints for rectangles and L shapes. Use bounded sampling with duplicate suppression, stable ordering and algorithm version. Retain invalid candidates with reasons; produce three feasible distinct golden options when possible. Exhausted feasible space returns fewer alternatives with an explicit explanation, never an endless loop or silent candidate filtering.

**Files:** `crates/archy-generate/src/{lib,sampling,rectangle,l_shape}.rs`, `crates/archy-core/src/variant.rs`, `tests/fixtures/generation/`.

**Tests:** same normalized input/seed repeats exact manifests; different footprints have different IDs; rectangle/L geometry validity; candidate bounds; impossible/narrow sites; count limits; repeated samples and exhaustion terminate; analytic shape fixtures.

**Acceptance evidence:** seed-42 golden variants, duplicate/exhaustion test outputs, parameter snapshots and algorithm provenance.

## A04 / D4 — Deterministic metrics and constraint checks

**Labels:** core, testing. **Dependencies:** A02, A03.

**Problem:** Plausible geometry is insufficient evidence for dimensions or constraints.

**Intended behavior/scope:** `evaluate_variant(spec, variant)` computes footprint area, GFA = area × storeys, explicit building height = storeys × floor-to-floor, bounding box and minimum clearances to all rectangular site edges. Use polygon predicates and a recorded length tolerance (initial analytic baseline 1e-6 m), separately dimensioned area/GFA tolerances in m² and transform comparison policy/version; document how length error propagates into polygon area and storey-sum GFA; reject invalid polygons. Emit measured/limit/explanation and required-check status without equating geometric success to code compliance. Missing evidence yields UNKNOWN; user constraints without legal source are never labeled law.

**Files:** `crates/archy-eval/src/{lib,geometry,metrics,checks}.rs`, `crates/archy-core/src/{metric,check}.rs`, `tests/fixtures/analytic/`.

**Tests:** analytic rectangle and concave L areas; clockwise/closed polygon handling; self-intersection and degenerate edges; boundary touching and values immediately above/below tolerance; translated coordinates; outside-site and excessive-height failures; required UNKNOWN gating; objective versus hard GFA.

**Acceptance evidence:** fixture calculations and metric JSON match tolerance, stable failure explanations, no unsupported compliance claim.

## A05 / D5 — Persistent generate/evaluate/report vertical slice

**Labels:** core, testing. **Dependencies:** A03, A04.

**Problem:** Independent algorithms do not yet form an inspectable repeat/compare workflow.

**Intended behavior/scope:** Wire `generate --project --count --seed`, `evaluate --variant|--all` and basic JSON/Markdown reports. Persist variants, metrics, validation, seed/version provenance and structured run logs under run IDs. Introduce revisioned atomic state updates and immutable run input snapshots; repeated runs remain accessible. Rendering is downstream of evaluation and invalid alternatives stay inspectable. Demonstrate a supported parameter-file edit producing a new immutable run/variant with parent/provenance and unchanged hashes for the original; preferences survive in snapshots and reports.

**Files:** `crates/archy-core/src/{state,run,storage}.rs`, `crates/archy-cli/src/{generate,evaluate}.rs`, `crates/archy-report/{Cargo.toml,src/lib.rs,src/templates.rs}`, workspace member update, `tests/golden/core_workflow.rs`, `examples/courtyard-housing/expected/`.

**Tests:** full no-Blender CLI golden run twice; exact parametric manifest/metric equality excluding intentionally varying run metadata; failed candidate retention; atomic write failure leaves prior state readable; outputs confined to project; inspect/restore earlier immutable run.

**Acceptance evidence:** two-run reproducibility diff, three variant manifests and reports, stored logs/snapshots and a documented recovery example.

## A06 / D6 — Pin and prove headless Blender on target Linux

**Labels:** adapter, documentation. **Dependencies:** A01.

**Problem:** Unverified backend availability/version assumptions threaten the workflow.

**Intended behavior/scope:** Verify official Blender compatibility/licensing documentation, pin a concrete known-good Blender/Python/target GPU combination, and run a bounded background smoke job. Document installation and availability diagnostics; unavailable binary maps to BACKEND_UNAVAILABLE. No live-session MCP dependency.

**Files:** `workers/blender/{README.md,versions.json,smoke.py}`, `docs/dependencies.md`, `docs/adr/0003-blender-runtime.md`, `tests/backend/blender_smoke.*`.

**Tests:** pinned version check; real background process creates/reopens a minimal scene and exits; missing binary fails clearly. Record Omarchy hardware/software and command.

**Acceptance evidence:** real target Linux smoke log and scene metadata, exact pin and official references. If unavailable locally, ticket remains externally blocked rather than passing on a mock.

## A07 / D7 — Typed Blender scene worker

**Labels:** adapter, testing. **Dependencies:** A03, A05, A06.

**Problem:** Core variants need a backend representation with matching geometry semantics.

**Intended behavior/scope:** Define bounded backend request containing project/variant/run IDs, schema version, params and output directory; response contains backend version, produced files/hashes, warnings and status. Build rectangle/L repeated storey volumes with deterministic names, local origin and SI/Z-up convention. Only validated typed operations enter the worker.

**Files:** `crates/archy-adapters/src/{lib,protocol,blender}.rs`, `workers/blender/{worker.py,scene.py,protocol.py}`, `schemas/backend.schema.json`, `workers/blender/tests/test_scene.py`.

**Tests:** request/response validation rejects unknown operations/malformed numeric data; mocked protocol contract; real Blender rectangle/L scene has expected names, storeys, transforms and dimensions.

**Acceptance evidence:** worker request/response fixtures and inspected scene bounds matching input params.

## A08 / D8 — Camera, material and render presets

**Labels:** adapter, testing. **Dependencies:** A07.

**Problem:** Alternatives need consistent inspectable visual evidence.

**Intended behavior/scope:** Produce three views per variant using documented orthographic/isometric presets, framing all massing and the site, basic materials and lighting. Record camera transforms, engine, samples, resolution, seed and color settings; document GPU pixel tolerance separately from exact geometry reproducibility.

**Files:** `workers/blender/{cameras.py,materials.py,render.py}`, `workers/blender/tests/test_render_presets.py`, `docs/render-reproducibility.md`.

**Tests:** camera count/transforms stable; all geometry inside framing for wide/deep/tall shapes; preset settings snapshot; real preview smoke has nonempty valid image dimensions and visible scene.

**Acceptance evidence:** three labeled preview views for one rectangle and one L, settings manifest, visual inspection note and documented image comparison tolerance.

## A09 / D9 — Verified artifacts and bounded worker execution

**Labels:** adapter, security, testing. **Dependencies:** A05, A07, A08.

**Problem:** A worker exit alone cannot prove valid PNG/GLB artifacts or safe completion.

**Intended behavior/scope:** Wire `render --variant|--all`, GLB/PNG exports and render manifests. A generated but unevaluated variant must first be evaluated through core APIs or refused with an actionable status; absence of evaluation never implies passed constraints. Preserve failed design checks independently of render state. Run workers with argument arrays, scoped cwd/output, timeout/cancellation, process-tree termination and captured bounded stderr. Verify file existence, nonzero size, safe relative path, hash and format parse before atomically accepting artifacts. Failures use explicit retriable/error status and cannot damage earlier artifacts.

**Files:** `crates/archy-adapters/src/{process,artifacts}.rs`, `crates/archy-cli/src/render.rs`, `workers/blender/{worker.py,exports.py}`, `tests/backend/process_contract.rs`, `tests/fixtures/backend/`.

**Tests:** render-before-evaluate enforces ordering; Artifact fixtures require path, sha256, media_type and producer; valid PNG/GLB parses; missing/empty/truncated/wrong-hash file fails ARTIFACT_INVALID; malformed worker JSON; timeout/cancel kills descendants; stderr bounded; traversal/symlink response refused; interrupted output is not committed.

**Acceptance evidence:** real PNG+GLB and checked manifest hashes, injected-failure results, cancellation log and accepted-run preservation check.

## A10 / D10 — Three-variant rendered workflow and geometry parity

**Labels:** adapter, testing. **Dependencies:** A04, A09.

**Problem:** Worker geometry can diverge from analytic measurements despite valid files.

**Intended behavior/scope:** Run golden generation/evaluation/render across all three alternatives. Independently read exported GLB transforms/bounds and compare to core footprints/heights; show render state separately from design/check state. Retain failed render diagnostics alongside successful evaluation.

**Files:** `tests/golden/render_workflow.*`, `tests/backend/geometry_parity.*`, `scripts/demo-render.*`, curated small `tests/fixtures/glb/`; production fixes stay in A07–A09 ownership.

**Tests:** unevaluated render is evaluated or clearly refused; rectangle/L exported world bounds equal evaluator within recorded length/transform tolerances, with dimensioned area comparisons where applicable, including translations/storeys; three alternatives each have GLB and three PNG views; one backend failure leaves other variants/results readable.

**Acceptance evidence:** real target run manifest, nine view artifacts, parsed GLB parity table and readable partial-failure report.

## A11 / D11 — Pin IfcOpenShell and build IFC4 hierarchy

**Labels:** adapter, documentation, testing. **Dependencies:** A01, A05.

**Problem:** IFC export needs a supported deterministic runtime and spatial hierarchy.

**Intended behavior/scope:** Verify official API/version/license compatibility, pin IfcOpenShell/Python environment and create IFC4 project/site/building/storey structure with SI units and local transforms. Record representation choice and schema in an ADR. Worker remains independently executable without an LLM.

**Files:** `workers/ifc/{requirements.lock,README.md,worker.py,hierarchy.py,protocol.py}`, `workers/ifc/tests/test_hierarchy.py`, `docs/adr/0004-schematic-ifc.md`, `docs/dependencies.md`.

**Tests:** pinned install/import; minimal IFC writes/reopens; correct schema, units, containment chain, count and storey elevations; bad request fails typed protocol.

**Acceptance evidence:** pin/environment record, reopen log and hierarchy inventory of a minimal IFC4 artifact.

## A12 / D12 — Schematic selected-variant IFC export

**Labels:** adapter, core, testing. **Dependencies:** A04, A09, A11.

**Problem:** A selected massing must transfer without falsely asserting detailed BIM semantics.

**Intended behavior/scope:** `export-ifc --project --variant` creates schematic conceptual proxy mass elements in the spatial hierarchy with footprint extrusion per storey, correct transforms, dimensions and provenance. Explicitly label schematic scope and draft status until approval. Use bounded typed adapter and artifact verification; no conversion of arbitrary meshes into walls/slabs. Include design manifest linking selection, source parameters, metrics, hashes, versions and approval state.

**Files:** `workers/ifc/{geometry.py,properties.py,worker.py}`, `crates/archy-adapters/src/ifc.rs`, `crates/archy-cli/src/export_ifc.rs`, `schemas/design-manifest.schema.json`, `workers/ifc/tests/test_export.py`.

**Tests:** rectangle/L selected variant exported, correct per-storey placements; schematic classification/property present; unapproved export draft; project-scoped hash-checked output; unsupported semantics never emitted as detailed elements.

**Acceptance evidence:** golden selected-variant IFC, design manifest and explicit mapping/limitations documentation.

## A13 / D13 — Reopen and independently measure IFC

**Labels:** adapter, testing. **Dependencies:** A12.

**Problem:** Successful IFC writing does not establish valid exchange geometry or hierarchy.

**Intended behavior/scope:** Reopen every accepted IFC with IfcOpenShell, check IFC4 schema, spatial relationships, typed conceptual element count, world transforms, geometry bounds and simple quantities against evaluator. Reject invalid export with ARTIFACT_INVALID and attach validation findings. Add an independent parser acceptance path for A19; do not describe the same writer's reopen as independent validation.

**Files:** `workers/ifc/{validate.py,worker.py}`, `workers/ifc/tests/test_validation.py`, `tests/golden/ifc_workflow.*`, `docs/ifc-mapping.md`.

**Tests:** corrupted/truncated files, wrong schema/counts/units/transforms/quantities fail; valid L/rectangle bounds and GFA/storeys match analytic expected values with versioned length (m), area/GFA (m²), and transform tolerances; validation result references artifact hash.

**Acceptance evidence:** reopened hierarchy and geometry/quantity comparison report, deliberately corrupted export failure log.

## A14 / D14 — Comparison and explicit human decisions

**Labels:** core, security, testing. **Dependencies:** A05, A10, A13.

**Problem:** Tradeoffs and human selection need evidence and an auditable authority boundary.

**Intended behavior/scope:** `compare --project` emits Markdown/JSON rankings using named deterministic metrics/objectives, includes invalid candidates, and distinguishes exact measurements, heuristic/subjective preferences and uncomputed criteria. Warn that daylight/cost are not quantitatively evaluated. `approve --project --variant --reason` appends actor/timestamp/decision plus reason to an immutable decision event, updates revision, and never infers consent from generation, rendering or export. Document explicit local human mechanism, selected variant and draft-to-approved behavior; expose historical state for rollback without rewriting events.

**Files:** `crates/archy-report/src/{compare,templates}.rs`, `crates/archy-core/src/approval.rs`, `crates/archy-cli/src/{compare,approve}.rs`, `tests/golden/review_workflow.rs`, `docs/adr/0005-approval-policy.md`.

**Tests:** canonical Metric method_version, Constraint unit/source/authority and Approval fields survive reports; preferences remain subjective; metric sources and ties stable; required UNKNOWN prevents compliant/automatic approved status; absent human event remains unapproved; forged input approval rejected; append-only approval event, reason validation, non-existent variant and stale revision; draft artifact metadata remains an immutable historical fact after later approval.

**Acceptance evidence:** comparison report with measured tradeoffs and disclaimers, explicit human decision event, unauthorized approval negative test and historical decision log.

## A15 / D15 — Scoped MCP facade and mutation transactions

**Labels:** core, security, testing. **Dependencies:** A02, A05, A09, A12, A14.

**Problem:** Existing agents need typed tools with the same semantics as CLI operations.

**Intended behavior/scope:** Pin and justify a maintained Rust MCP implementation or documented Python fallback. Expose `get_project`, `validate_project`, `list_variants`, `generate_variants`, `evaluate_variant`, `render_variant`, `compare_variants`, `export_ifc`, `propose_approval`; reuse core APIs. Every mutation validates idempotency key, expected revision and project-relative destination. Replays return the original result; conflicting reuse/stale revision fails without effects. `propose_approval` only creates a proposal and cannot call final human approval. Register explicit capabilities and schemas; no arbitrary process/Python/path tool.

**Files:** `crates/archy-mcp/src/{lib,server,tools,transaction}.rs` or narrowly documented `workers/mcp/` fallback, `schemas/mcp/`, `tests/mcp/{contracts,authority,idempotency}.*`, `docs/agent-playbooks/golden-workflow.md`.

**Tests:** enumerate exactly named tools; CLI/tool result equivalence; malformed arguments; missing revision/key/destination; duplicate calls, concurrent same key, mismatched replay and stale revision; proposal cannot approve; read operations do not mutate; unknown tool/direct code requests rejected.

**Acceptance evidence:** protocol transcript running generation→evaluation→render→compare with existing agent client, schema fixtures, idempotency/revision/approval gating test outputs.

## A16 / D16 — Security boundary hardening

**Labels:** security, testing. **Dependencies:** A09, A12, A15.

**Problem:** Untrusted brief text, worker results and paths can undermine scoped authority.

**Intended behavior/scope:** Audit and enforce canonical path containment including symlinks, safe create/overwrite policy, process resource/time/output limits and explicit default network policy. Treat input, imports and tool output as data; prompt text never adds capability. Keep arbitrary-code escape hatches disabled in normal configuration. Separate reads, reversible design mutations, local exports and external publishing authority; snapshot before an authorized destructive replacement.

**Files:** `crates/archy-core/src/{paths,storage}.rs`, `crates/archy-adapters/src/process.rs`, `crates/archy-mcp/src/{tools,transaction}.rs`, `tests/security/`, `docs/security.md`.

**Tests:** `..`, absolute/UNC/drive paths, symlink escape, output collision, filename injection, malicious JSON/description/worker instructions, process forks/timeouts, output exhaustion, network policy configuration, forged human actor; prior user design files remain byte-identical after refusals.

**Acceptance evidence:** threat-boundary review and malicious-input test transcript; documented platform isolation limits and no unrestricted execution capability exposed by default.

## A17 / D17 — Failure recovery and deterministic reruns

**Labels:** testing, core. **Dependencies:** A10, A13, A15, A16.

**Problem:** Crashes and interrupted concurrent runs must not corrupt accepted history.

**Intended behavior/scope:** Fault-inject backend crash, malformed response, interruption during staging/commit, stale revisions and duplicate mutations. Recover abandoned staging safely, retain failure logs, preserve accepted manifests and permit repeat/review/restore of earlier runs. Confirm complete deterministic manifest/metric behavior with pinned versions; isolate variable timestamps/paths in run envelopes rather than deterministic design records.

**Files:** `tests/reliability/`, `tests/golden/reproducibility.*`, narrowly scoped fixes in `crates/archy-core/src/{state,run,storage}.rs`, adapter process and MCP transaction owners, `docs/recovery.md`.

**Tests:** restart at each transaction boundary; accepted file/hash inventory unchanged after all fault classes; retry flags accurate; duplicate/concurrent requests single-apply; parameter edit creates a reproducible child iteration with retained parent/provenance and unchanged prior hashes; deterministic two-run and restart rerun equality; cancellation recovery.

**Acceptance evidence:** fault matrix, before/after artifact hash inventories, exact manifest/metric diffs and usable recovery instructions.

## A18 / D18 — Installation, one-command demo and contributor handoff

**Labels:** documentation, testing. **Dependencies:** A14, A15, A17.

**Problem:** A working developer machine does not make the release reproducible for others.

**Intended behavior/scope:** Provide Omarchy install/version checks, worker environment setup, CLI/MCP reference, complete golden example and a single-command demo through generation/evaluation/render/compare. It records proposal/selection and pauses at explicit human approval as necessary; it cannot silently approve. Explain draft local IFC export policy, hard objectives, approximate GFA, unsupported daylight/cost/legal claims, artifacts/history/recovery and contributor workflow. Reconcile ADRs and daily/session run-log template. Update or mark older architecture/integrations/project-structure/roadmap proposals superseded where they conflict with structured manifests, Rust core and typed workers. Use golden and analytic examples as a small benchmark set; record commands, hardware, elapsed time and artifact sizes without inventing a throughput target.

**Files:** `README.md`, `CONTRIBUTING.md`, `docs/{install,cli,mcp,limitations}.md`, `docs/agent-playbooks/`, `docs/adr/`, `examples/courtyard-housing/`, `scripts/demo.*`, `docs/templates/session-handoff.md`.

**Tests:** documented commands on a fresh project; one command produces full review artifacts without manual modeling; schema/examples lint; demo handles backend unavailability and approval boundary with clear status.

**Acceptance evidence:** command transcript, generated report/artifact links, complete agent playbook, version matrix and reviewed accurate limitations.

## A19 / D19 — Clean target installation and independent artifact QA

**Labels:** testing, adapter. **Dependencies:** A18.

**Problem:** Real target installation, visual quality and interoperable IFC remain release risks.

**Intended behavior/scope:** Install from documented instructions on a clean target Omarchy/Linux machine. Run the one-command golden workflow with pinned versions, inspect all three variants visually, independently parse IFC and compare hierarchy/geometry bounds. Record machine, dependency/GPU versions, exact commands and hashes. Select and document a separate IFC parser/viewer during this ticket using official compatibility sources; do not mislabel IfcOpenShell reopening as independent parsing.

**Files:** `docs/validation/clean-target.md`, `docs/validation/visual-qa.md`, `scripts/validate-release.*`, `tests/acceptance/`, small curated expected comparison fixtures.

**Tests:** clean installation and all required acceptance commands; at least one valid PNG/variant (three views remain workflow requirement), GLB parsing, independent IFC hierarchy/world-bounds comparison; visual framing/orientation/material checks; no manual 3D edits.

**Acceptance evidence:** target-machine install/demo logs, three-variant contact sheet and visual notes, independent parser/version/output, artifact hash inventory. Missing target access or parser is an open release blocker.

## A20 / D20 — Traceable release candidate and acceptance gate

**Labels:** spec, testing, documentation. **Dependencies:** A01–A19, A21–A23.

**Problem:** Release claims need a complete audit trail and explicit failure disposition.

**Intended behavior/scope:** Assemble release candidate with locked dependencies, acceptance matrix, known limitations, GitHub issue dispositions, reproducibility evidence and demo walkthrough. Verify MIT core license file and distribution compatibility, protected-main/ruleset settings with required checks/reviews, current README, all required CI, native integration and clean-machine evidence. Keep actual Omarchy gates explicitly deferred until a target exists; preparation is not a waiver. No required failing acceptance can be relabeled an optional exclusion; scope changes require ADR and updated tests/spec acceptance. Prepare release assets without publishing absent explicit external-release authority.

**Files:** `CHANGELOG.md`, `docs/releases/0.1.0.md`, `docs/validation/acceptance-matrix.md`, `docs/plans/backlog.md` issue links/status only, release packaging/workflow configuration as needed.

**Tests:** final Rust format/clippy/test, Python lint/test, schemas, security/reliability, golden demo and artifact validation; acceptance document links exact commit, commands and evidence; installation instructions match candidate.

**Acceptance evidence:** candidate commit/tag proposal, all matrix rows passing on named platforms, demo walkthrough, known limitations and blockers/exclusions individually triaged in GitHub. No release completion claim while clean-target or required acceptance evidence is absent.

## A21 — Omarchy doctor, mise environments and Arch packaging

**Labels:** omarchy, core, documentation, testing. **Dependencies:** A01, A06, A11, A16.

**Problem:** An Omarchy workspace needs reproducible installation and honest capability diagnostics without replacing global runtimes.

**Intended behavior/scope:** Add non-mutating `archy doctor --json` reporting Omarchy/package versions, executables, pinned worker compatibility, user-systemd/cgroup and bubblewrap capabilities, explicit unavailable/deferred results and actionable remedies. Use project-local mise Rust/Python tasks and separate locked IFC environment; Blender retains bundled Python. Provide reviewed PKGBUILD/install manifest with license notices and uninstall preserving projects, decisions and user configuration. Detect supported Omarchy 4.0.6 features; keep 4.5 migration explicit. Never modify Arch system Python, install Omarchy on the Windows host, silently relax isolation, auto-migrate projects or upgrade accepted runtime baselines.

**Files:** `crates/archy-cli/src/doctor.rs`, `.mise.toml`, `packaging/arch/{PKGBUILD,archy.install}`, `docs/omarchy/install.md`, `tests/doctor/`, packaging fixtures. Reuse A06/A11 pins and A16 containment policy; shared dependency edits require rationale.

**Tests:** fixture-driven complete/missing/unsupported version and capability reports; doctor makes no filesystem/config mutations; package manifest/lint and staged install/uninstall preserve user/project sentinel files; task environment separation; unsupported rolling Blender rejected. Real clean-Omarchy install and capability checks remain deferred target gates, not mocked successes.

**Acceptance evidence:** JSON/text diagnostic snapshots, package inspection/lint output, installation/uninstall file inventory, exact dependency/license pins and explicit pending target-machine commands/results.

## A22 — Native Quickshell plugin and reversible desktop integration

**Labels:** omarchy, adapter, security, testing, documentation. **Dependencies:** A14, A16, A21.

**Problem:** Omarchy users need a native project/run launcher and status surface without a new viewport or unsafe shell authority.

**Intended behavior/scope:** Supply an `archy`-namespaced schemaVersion-1 Quickshell plugin using theme components/colors and bounded structured CLI status. Open report/previews/Blender using fixed argv. Provide a valid separately packaged/local plugin root with manifest; do not claim `omarchy plugin add` works against the monorepo root. Optional user-owned menu/keybinding and lightweight theme/post-update refresh hooks need preview, collision detection, backups and reversible uninstall; package-owned `/usr/share/omarchy` is read-only. Feature-detect tagged 4.0.6 APIs and Hyprland Lua binding files. Third-party plugins are unsandboxed: never execute project text/worker commands or grant approval; no viewport, theme engine or schema upgrades.

**Files:** `integrations/omarchy/plugin/{manifest.json,Main.qml}`, `integrations/omarchy/{install,uninstall}.*`, `integrations/omarchy/templates/`, `docs/omarchy/desktop.md`, `tests/omarchy/integration/`. CLI remains usable without plugin.

**Tests:** plugin manifest/API fixtures, malformed/missing CLI status, injection/path refusal and fixed-argv checks; install preview/collision/refusal, idempotent install, backup restore and uninstall sentinel preservation; package defaults never changed. Actual plugin discovery/validation/enable-disable/hot reload, theme changes and Hyprland launch behavior await real Omarchy.

**Acceptance evidence:** reviewed manifest/local installation layout, fixture/test outputs, config before/after hashes and rollback transcript; target logs/screenshots explicitly pending until actual execution.

## A23 — Provider-neutral agent/session playbooks and deferred Omarchy harness

**Labels:** omarchy, documentation, testing, security. **Dependencies:** A15, A17, A18, A21, A22.

**Problem:** Existing Omarchy agents and sessions need a bounded Archy workflow and an executable target acceptance checklist with truthful deferred status.

**Intended behavior/scope:** Supply a provider-neutral project skill/playbook for the nine stdio MCP tools, evidence citations, reversible edits and human approval. Document optional Herdr/tmux layout for brief, agent, logs and artifact review, using supported launch commands instead of depending on Herdr internal sockets. No API-key requirement, scheduler, hosted service, agent-client replacement or token meter. Prepare a target harness for doctor, pinned workers, isolated resources/cancellation, clean golden workflow, plugin/theme/keybinding behavior, CPU baseline/optional GPU, independent IFC inspection and visual review. Separate source/CI/WSL evidence from named actual Omarchy gates; record deferred results until target access, never auto-waive or call them passing.

**Files:** `docs/agent-playbooks/{omarchy,herdr-tmux}.md`, `integrations/agents/archy/SKILL.md`, `scripts/validate-omarchy.*`, `tests/acceptance/omarchy/`, `docs/validation/omarchy-gates.md`. Coordinate A19 clean-machine evidence and A20 acceptance matrix without duplicating core/MCP implementation.

**Tests:** playbook/tool-name/schema checks; dry-run harness enumerates every required gate and returns explicit deferred/unavailable status with machine/version requirements; simulated failures remain failures; agent-provided actor/proposal never counts as consent; session layout optional and CLI/MCP sufficient without Herdr. Execute source checks now; actual target/GPU/visual acceptance deferred.

**Acceptance evidence:** complete runnable command list, sample dry-run report marked deferred, protocol/client transcript when available, gate-to-A19/A20 mapping; later real machine logs/contact sheets/IFC reader output required for target gate closure.

## Release requirement traceability

| Requirement from specification | Owning tickets | Required release evidence |
|---|---|---|
| D1–D5 / week 1 | A01–A05 | CI, schemas, validated demo, exact repeat manifest/metric diff |
| D6–D10 / week 2 | A06–A10 | pinned real Blender, three rendered alternatives, geometry parity |
| D11–D15 / week 3 | A11–A15 | validated IFC, comparison, human decision, scoped MCP transcript |
| Omarchy platform clarification | A21–A23, A19, A20 | doctor/package/environment fixtures, reversible native plugin and provider-neutral workflow; actual target gates deferred |
| D16–D20 / week 4 | A16–A20 | threat/recovery tests, clean installation, complete release gate |
| Versioned YAML/JSON project, SI normalization and actionable invalid-input errors (§1.3, §4) | A01, A02 | positive/negative fixtures and canonical normalized spec |
| ≥3 distinct seeded rectangles/L variants, immutable editable parameter snapshots (§1.3, §2) | A03, A05 | seed-42 fixtures, feasible golden alternatives or evidenced infeasibility |
| Area, simple storey-sum GFA, height, site bounds/setbacks, tolerance/version (§3.3, §4.2) | A04, A10 | analytic fixtures plus exported geometry parity |
| Input/variant/metric/provenance/log persistence; repeat, compare, reversible history (§1.3, §2, §9) | A05, A14, A17 | run snapshots, comparison and restore/recovery transcript |
| Typed backend request/response, hashes, bounded jobs/cancellation (§3.2) | A07, A09, A12, A16 | protocol tests, real output verification, timeout/cancel evidence |
| Scene, cameras/materials, three views, GLB/PNG (§2, D7–D10) | A07–A10 | nine previews, three valid GLBs and settings manifests |
| Schematic IFC4 hierarchy, conceptual typing, transforms, quantities and reopen (§1.3, D11–D13) | A11–A13, A19 | IfcOpenShell and independent-parser geometry/hierarchy checks |
| Full CLI command table and stable errors (§5, §5.1) | A02, A05, A09, A12, A14 | CLI contract tests and nonzero structured failures |
| All nine MCP tools; idempotency/revision/destination contracts (§5) | A15 | tool schemas, replay/stale tests and CLI parity |
| Explicit human approval only; unapproved export draft (§5, §7) | A12, A14–A16 | proposal cannot approve; explicit event; forged actor refused |
| Measured vs heuristic vs uncomputed claims, no inferred law (§5.2, §9) | A02, A04, A14, A18 | constraint provenance and report source/limitations review |
| Scoped paths, isolation, no arbitrary agent code, no silent overwrite (§9) | A09, A15–A17 | malicious-input tests and preservation hashes |
| License/dependency decisions, lockfiles, trunk/small PRs/session handoffs (§8, §10, §14) | A01, A06, A11, A15, A18, A20 | ADRs, pins, license record, review and handoff evidence |

## Definition-of-done acceptance ledger

The following are gates, not inferred successes. Copy this table to the candidate acceptance matrix and replace “pending” with evidence links and commit/machine identifiers.

| §7 acceptance | Owner | Evidence to attach | Initial state |
|---|---|---|---|
| Reproducibility | A03, A05, A17 | same seed+versions exact parametric manifests and metric JSON; render tolerance documented | pending |
| Validity | A02–A04 | invalid site/polygon/impossible setback/height stable failure fixtures | pending |
| Variation | A03, A10 | ≥3 feasible geometrically distinct golden alternatives; exhaustion explanation when infeasible | pending |
| Measurement | A04, A10, A13 | analytic and independent artifact bounds/area/height/setback comparison within tolerance | pending |
| Artifact integrity | A09, A12, A13, A17 | parse/size/path/hash checks for every accepted output stored under run ID | pending |
| Blender | A06–A10, A19 | target Omarchy headless GLB and previews for every variant | pending |
| IFC | A11–A13, A19 | schema/hierarchy and selected mass bounds reopen, independent parsing | pending |
| Agent autonomy | A15, A18, A19 | typed generate→evaluate→render→compare transcript, zero hand-modeling | pending |
| Human control | A12, A14–A16 | absent consent unapproved/draft; explicit human event only | pending |
| Resilience | A09, A15–A17 | timeout/crash/malformed/duplicate tests preserve accepted artifacts | pending |
| Release gate (§7.2) | A19, A20 | clean install, every required acceptance passes, no unrestricted agent code, accurate README | pending |

## Decisions and blockers to track

| Decision/blocker | Owner | Disposition required |
|---|---|---|
| GitHub tracker | Coordinator | Authentication restored; A01–A23 and three roadmap tickets published; verified URLs in [issue-map.md](issue-map.md) |
| Core license and distributed dependency compatibility | A01/A20 | Founder selected MIT for original core; add LICENSE and verify separate Blender/IfcOpenShell/MCP distribution notices and obligations |
| Protected main | A01/A20 | Fresh API read verified strict required rust check, one review, stale dismissal, admin enforcement and no force push/deletion after coordinator setup; reverify final required CI checks before closure |
| Initial agent client | A15 | Choose one for demo while keeping provider-neutral tool schemas |
| Blender version, Linux/GPU target | A06 | Real tested pin and reproducibility tolerance |
| IFC representation/labels | A11/A12 | ADR for audited conceptual proxy/storey subset, never detailed walls/slabs |
| Approval/local export policy | A14 | Baseline draft local exports; explicit human final approval mechanism; stricter changes recorded in ADR/tests |
| Independent IFC parser and clean target access | A19 | Real independent validation and clean-machine logs, otherwise release blocked |
| Provisional Archy name | A20 | Record namespace/trademark/domain status as an open product decision; no implied clearance |

## Later roadmap — excluded from release 0.1

Three separate `roadmap` issues are linked in [issue-map.md](issue-map.md); none may silently replace a 0.1 requirement.

- Month 2: richer/courtyard footprints beyond rectangle/L, facade modules, additional explicit objectives and Pareto selection.
- Month 3: room/circulation graphs, preliminary space planning, expanded IFC semantics and interoperability/round-trip fixtures.
- Later: independently validated daylight/energy, cost databases with provenance, FreeCAD/Truck/CAD backends, review UI, collaboration and professional approval workflows.
- Explicitly excluded: detailed layouts/egress/structure/permitting, production RVT, arbitrary mesh-to-semantic BIM, custom viewport/kernel/node editor, cloud service, multi-agent scheduler and unrestricted execution.

## Backlog self-review

All D1–D20 have one owning issue above; every §7 acceptance has owners and evidence criteria. No implementation is included in this planning ticket. Potential release blockers are represented explicitly rather than converted into successful mocked checks. Implementers must update the tracker and this traceability if a dependency, interface, acceptance criterion or scope decision changes.
