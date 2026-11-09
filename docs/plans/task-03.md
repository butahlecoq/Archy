# Task 03: A05 persistent generate/evaluate/report plan

**Status:** Research and implementation handoff, 29 October 2026. No implementation or acceptance claim. Task02 remains owned by its existing Sol implementers/reviewers. Start integration only after its reviewed contracts and integrity fix land. Actual Omarchy execution is deferred.

**Goal:** Make deterministic generation and evaluation an inspectable, recoverable workflow with immutable inputs/results, basic JSON/Markdown reports, and a supported parameter edit creating a new run without changing earlier evidence.

**Spec:** [release-0.1.txt](../specs/release-0.1.txt), read in full; repository `AGENTS.md`; [A05 backlog](backlog.md); [task02 contract](task-02.md) and [implementation plan](task-02-implementation.md). Existing [reuse research](../research/omarchy-and-reuse.md), [backend research](backend-research.md), task01 handoff and design decisions were consulted, not treated as fresh version verification.

**Scope:** A05 only. First deliver the storage/integrity boundary below, then CLI integration, reports and editable-run proof. The first slice does not close A05. No workers, MCP server, comparison/ranking engine, approval mechanism, generic database, desktop integration or target installation. Preserve later A09/A15/A17 reuse without implementing those tickets.

## Verified findings and decisions

These are primary documentation observations, not compilation, download-checksum or filesystem test evidence. Recommendations are coordinator proposals for this slice.

| Mechanism/source inspected | Finding and A05 decision |
| --- | --- |
| [rustix 1.1.4 openat2](https://docs.rs/rustix/1.1.4/rustix/fs/fn.openat2.html), [ResolveFlags](https://docs.rs/rustix/1.1.4/rustix/fs/struct.ResolveFlags.html), [renameat_with](https://docs.rs/rustix/1.1.4/rustix/fs/fn.renameat_with.html), [RenameFlags](https://docs.rs/rustix/1.1.4/rustix/fs/struct.RenameFlags.html) | Available safe descriptor-relative APIs with `fs` feature; `BENEATH`, `NO_SYMLINKS`, `NO_MAGICLINKS`, `NO_XDEV` and `NOREPLACE` support the proposed Linux boundary. Recommend exact candidate 1.1.4, subject to Cargo resolution/build and filesystem tests. No handwritten syscall bindings. |
| [Linux openat2](https://man7.org/linux/man-pages/man2/openat2.2.html) | Kernel-enforced resolution constraints are stronger than canonicalize-then-open. Use a pinned root descriptor and reject symlink traversal. Do not rely on newer flags appearing in current documentation, such as `OPENAT2_REGULAR`; verify regular-file type using descriptor metadata. |
| [Linux rename](https://man7.org/linux/man-pages/man2/rename.2.html) | Ordinary rename can replace an existing destination. Publish with `RENAME_NOREPLACE`; filesystem support is required. Never emulate it with exists-check followed by ordinary rename or cross-device copy. |
| [Linux fsync](https://man7.org/linux/man-pages/man2/fsync.2.html) | File synchronization does not itself persist its directory entry. Explicitly synchronize files and affected directories, and propagate failures before acknowledging durable success. |
| [Rust File](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock) | Documentation labels locks stable since 1.89.0; inspected current page reports 1.99.1. Reuse `try_lock` on a persistent lock file, with explicit contention errors. Locks coordinate cooperating processes; they do not stop malicious same-user writes. |
| [cap-std 4.0.3 Dir](https://docs.rs/cap-std/4.0.3/cap_std/fs/struct.Dir.html) | Reusable directory-capability alternative. Prefer one rustix-backed storage boundary for this Linux slice because resolution flags and no-replace publication must be explicit; do not add both libraries without demonstrated need. |
| [tempfile 3.27.0 NamedTempFile](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist_noclobber) | No-clobber persistence exists, but documentation describes path-based security limits, possible leftover links and platform-dependent atomicity. Suitable for test fixtures; not the authority boundary or a multi-file transaction engine. |
| [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html) | Mature transactional alternative. Defer: A05 needs inspectable structured files, and a database would still need coordination with external artifact publication. Reconsider only if file-history indexing becomes a measured problem. |
| [uuid API](https://docs.rs/uuid/latest/uuid/struct.Uuid.html#method.new_v4) | Inspected page reports 1.29.0 and feature-gated `new_v4`. Use UUID v4 for execution IDs, separate from task02 deterministic variant IDs. Exact dependency/license/transitive resolution remains an implementation gate. |
| [rustix tagged Cargo metadata](https://github.com/bytecodealliance/rustix/blob/v1.1.4/Cargo.toml) | Declares Rust 1.63 minimum and `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`. Record chosen license/notices and resolved transitives in dependency evidence before distribution. Metadata alone is not a distribution audit. |

Network evidence: initial shell `curl` failed with exit 6, DNS resolution failure. After network enablement, `curl` returned HTTP 200 for rustix's versioned RenameFlags page; subsequent GETs succeeded for that page (containing `NOREPLACE`) and cap-std 4.0.3. The web tool still failed those exact URLs, and Python urllib still encountered DNS failure: access differed by tool. Other sources above were available through the web tool. No dependency fetch/build was attempted. The newer rustix 1.1.6 API page was visible, but it is not the proposed baseline. Old backend, geometry, MCP and Omarchy version claims were not revalidated for this A05 task and are not adopted here.

GitHub status: the local 26-issue map is historical evidence. The user subsequently reports authenticated API ADMIN access and successful `git ls-remote`; this planning task did not independently reverify current issue contents, protection or permissions. No remote mutation is required for this artifact.

## Contracts and integrity boundary

Consume task02's reviewed `generate_variants(&NormalizedSpec, u32, u64) -> Result<GenerationResult, ArchyError>` and `evaluate_variant(&NormalizedSpec, &Variant) -> Result<EvaluationReport, ArchyError>`. Reuse shared `parse_project`, canonical variant construction/validation and the existing error envelope. Observed source was in progress; no task02 handoff file was present at inspection. Reconcile final structs with its handoff before coding integration; do not edit its sampler, geometry identity or evaluator to fit storage.

Task02 review found a forged deserialized `required_checks_passed` could be accepted; its implementer is fixing this. **Persistence must independently enforce the trust boundary:** hashes establish byte consistency, not truth. On every accepted-run read, validate source/normalized correspondence, canonical variant parameters/footprint/ID/project association, and recompute evaluation using the recorded supported algorithm version. Compare the full stored evaluation, including metrics, checks, tolerances, professional limits and aggregate boolean, against recomputation. Reject mismatches as `ARTIFACT_INVALID`; malformed external variant input remains `INVALID_GEOMETRY`. Never use stored aggregate booleans, reports or logs as authority. Test forgery even when the attacker updates manifest hashes consistently.

Unknown historical algorithm/schema versions must return an explicit unsupported error, not silently recompute using today's implementation or grant authority. Preserve bytes for inspection. A legitimate required FAIL/UNKNOWN stays inspectable and cannot become approval. Persist error outcomes separately from valid geometric evaluations; never fabricate zero metrics for malformed geometry.

Proposed new types in `archy-core` (versioned, strict serde/schemars):

- `Artifact { path, sha256, media_type, producer }`: canonical project-relative path; no arbitrary host paths.
- `RunManifest`: schema/project/run IDs, operation, parent run/variant references, input/normalized digests, source format, seed/count, algorithm/build/lock provenance, ordered artifact inventory and per-variant outcome references. Execution time belongs here, never inside deterministic variants/metrics.
- `RevisionRecord`: monotonically increasing `u64` revision, predecessor-record hash, run ID/manifest hash, idempotency key, request digest and stable result receipt. Genesis is revision 0; first accepted mutation creates revision 1. Reject overflow.
- `MutationContext`: idempotency key, expected project revision and validated relative destination. CLI may obtain defaults under lock; explicit caller values are never overridden.
- `VerifiedRun`: private construction by the verifying reader, containing the recomputed trusted view. Deserializing `RunManifest` or `EvaluationReport` must not construct it.

Storage owns files and revision transactions, not generator/evaluator dependencies. Expose a narrow `ProjectStore`/`PendingRun` API in core: open root, lock, read verified byte inventory, begin request, write new artifact, publish run and commit revision. Integrity orchestration in CLI calls the shared canonical validator/evaluator before constructing its trusted view; keep core independent of `archy-eval` to avoid a dependency cycle. Exact Rust field types/signatures are finalized with task02's reviewed types in step 1, before integration.

## Publication, confinement and recovery protocol

Project root is the opened parent directory of `--project FILE`; source bytes are read once through that root and reused for parsing, hashing and snapshotting. All managed paths are relative to the pinned descriptor. Default run destination is `runs`; internal metadata lives in `.archy`. Reserve internal destinations so caller-selected outputs cannot collide with state, staging or source files.

```text
.archy/lock                         persistent inode; never unlink/replace
.archy/staging/<operation-id>/      private, unaccepted work
.archy/revisions/00000000000000000001.json
runs/<run-id>/input.<yaml|json>
runs/<run-id>/normalized.json
runs/<run-id>/variants/<variant-id>.json
runs/<run-id>/evaluations/<variant-id>.json
runs/<run-id>/report.json, report.md, events.jsonl, manifest.json
```

1. Open trusted root once. Reject absolute/parent/empty components, unsupported path forms, symlinks and mount crossings beneath it. Open directories using descriptor-relative constrained resolution; create leaves exclusively, with no-follow semantics and regular-file checks. Apply the same rules to reads, staging, state, locks and recovery. Do not reopen via ambient absolute paths or `/proc/self/fd` strings. New files must not be hard-linked aliases; reject unexpected link counts on managed evidence. Never truncate an existing file.
2. Acquire the persistent project lock before recovery, request lookup and revision checks. Hold it through this bounded CPU-only transaction. Use nonblocking contention (`REVISION_CONFLICT`, retriable true) rather than indefinite waiting. Bootstrap managed directories non-destructively; occupied incompatible entries fail. Do not remove partial initialization automatically.
3. Validate the contiguous revision chain and its referenced manifests. Lookup idempotency before stale-revision rejection: same key and identical request returns the original verified receipt, even after later revisions. Same key with different request is `REVISION_CONFLICT`. A new key with stale revision fails without publication. Fingerprint versioned typed request bytes, input digest, operation/options, resolved source run, destination and expected revision; do not fingerprint timestamps. Reject changed input under a replayed key. Resolve implicit source selection from the original receipt for replay.
4. Stage on the same filesystem in a fresh exclusively created directory. Serialize deterministic payloads consistently, preserve exact input bytes/preferences, compute checksums and verify serialized contents. Freeze logs and reports before manifest creation. Manifest lists payload hashes; revision record stores manifest hash, avoiding self-hash cycles.
5. Synchronize payload files and manifest, then staging directories bottom-up. Publish the complete directory using descriptor-relative no-replace rename to `<destination>/<run-id>`. Synchronize both affected parent directories. No acknowledged result yet; an unreferenced published directory is an orphan, not an accepted run.
6. Prepare the next revision record in a fresh temporary file, synchronize it, publish to its absent numbered name with no-replace rename, then synchronize its directory (and source parent if different). This record is the authoritative commit point; there is no separately authoritative mutable HEAD or idempotency index. Readers derive state from the validated chain. Return success only after required synchronization succeeds.
7. Read accepted evidence through the constrained reader and integrity checks above. Never append metrics/logs to a committed run. `evaluate` creates a new run referencing its immutable source; later backend tickets must also publish new evidence rather than mutate this snapshot.

This is application-level immutability, not protection from an OS administrator or another process with the same user's write privileges. Restrict managed directories to the owner; locks coordinate Archy writers. Directory descriptors prevent path substitution from redirecting operations to a replacement tree, but do not make hostile same-user directory relocation impossible. A16 process isolation remains separate. Unsupported kernel/filesystem primitives fail explicitly; no weaker fallback. Local Linux filesystems are the tested target; network filesystem durability is unclaimed.

| Interruption/failure | Required result |
| --- | --- |
| Before run publication | Prior revision readable; staging remains unaccepted and preserved for diagnosis. No automatic recursive cleanup. |
| After run publication, before revision publication | Orphan remains unaccepted. Retry with the same key/request may reuse it only after full verification and if the expected revision still matches; otherwise report conflict and preserve it. Other requests may proceed from the accepted chain. |
| After revision publication, before response | Recover the valid record, verify evidence and synchronize required directories before replay; return the recorded result without a second revision. |
| fsync failure after namespace publication | Return `INTERNAL_ERROR` with explicit uncertain-commit/retry-same-key guidance and retriable true. Never report definite rollback or silently create another run. |
| Missing/corrupt referenced payload, revision gap or broken chain | `ARTIFACT_INVALID`; refuse further mutation until explicit recovery. Earlier verified runs remain inspectable; do not truncate history or promote arbitrary orphan directories. |
| Existing destination, symlink or unsupported confinement | Preserve it; actionable `PERMISSION_DENIED` or `INTERNAL_ERROR` for unsupported storage capability, non-retriable. No overwrite fallback. |

Recoverability here means verifying history, replaying interrupted requests and restoring an earlier design into a **new** run. Garbage collection, backups, arbitrary repair and backend recovery remain later work. Process-kill tests do not prove sudden-power-loss durability; target filesystem evidence remains required.

## CLI and complete A05 behavior

- `generate --project FILE --count N --seed S [--json]` persists source bytes, normalized spec, generation result/exhaustion details, canonical variants, per-variant evaluations, reports and structured logs in one transaction. Retain valid candidates with failing checks. Do not claim feasibility merely because N candidates exist.
- `evaluate --project FILE (--variant ID | --all) [--run RUN_ID] [--json]` uses the selected immutable run's normalized input and verified variants. Without `--run`, select the latest accepted generation/edit run under lock; `--all` means its variants, not all historical duplicates. Current source-file edits are not silently substituted. Persist a new evaluation run with source references.
- Both commands accept `--idempotency-key`, `--expected-project-revision` and `--destination` (default `runs`). A supplied key requires an explicit expected revision; without either, create a fresh key and capture revision under lock. This keeps the golden CLI simple while allowing reliable caller retries. Output one JSON receipt containing run/revision, paths, variant IDs and outcome; diagnostics go to stderr. Keep clap usage errors distinct from structured operation errors.
- A published run with legitimate failed/unknown required checks is a successfully persisted operation with explicit unsuccessful design status. It is not an I/O error and never an approval. CLI exits zero for successful persistence; malformed requests/storage failures return nonzero existing error envelopes. Document this distinction and test it.
- Add `generate --from-run RUN_ID --variant ID --params PROJECT_RELATIVE_JSON` as the bounded edit mode, mutually exclusive with sampling count/seed. The parameter file contains editable shape/storey inputs only, no authoritative ID/footprint/check flags. Reuse task02's canonical constructor to derive a new variant; preserve source seed and record edit provenance separately. Parent run/variant and parameter-file bytes/hash are persisted. A real geometry edit must change variant ID; original hashes remain identical. Reusing an earlier run's parameter values creates a new restoration run without overwriting source files.
- `archy-report` emits basic JSON/Markdown from verified typed data: measured values/units/methods, checks and explanations, source/authority declarations, preferences, exhaustion, provenance and professional limits. Daylight/cost remain uncomputed. Escape untrusted text; do not interpret it as templates/HTML/commands. Ranking, compare command and human approval stay A14.

## Bounded implementation sequence

### 1. First slice: storage, revisions and integrity reader

**Files:** create `crates/archy-core/src/{run,state,storage}.rs`, focused tests `crates/archy-core/tests/{storage,run_integrity}.rs`; extend core exports and source-derived schema example. Coordinate Cargo files/lock and dependency rationale after task02 finishes. Keep storage tests independent of Blender/Python. CLI-side recomputation is exercised in step 2; core reader returns only verified bytes/typed structural data, never a trusted evaluation flag.

- [ ] Confirm task02 handoff and canonical validation/recomputation interfaces; freeze exact run/revision/request schemas and limits. Use input limit 1,048,576 bytes and generation count 1..1000 from existing contracts; bound stored JSON/log sizes and inventory count explicitly before allocating.
- [ ] Add failing tests for exclusive creation, every occupied target type, traversal, symlinks at every component, hardlinks, path replacement races and unsupported primitives. Assert outside sentinels and prior-run hashes never change.
- [ ] Implement the protocol above with rustix and standard locks; resolve/pin dependencies and document licenses. Tests cover two competing processes, stale revisions, same-key identical/different requests and full-chain validation.
- [ ] Inject failures at file write/sync, directory sync, both rename points and response delivery; kill a child between phases. Assert exactly one accepted revision per key and the recovery table's outcomes. Do not rely only on mocked successful writes.

**Deliverable:** independently reviewable no-overwrite storage API and executable failure/replay evidence. It is safe to build CLI persistence on this; it is not A05 completion.

### 2. Persist generate/evaluate through shared algorithms

**Files:** `crates/archy-cli/src/{main,generate,evaluate}.rs`, new focused `crates/archy-cli/src/run_integrity.rs`, CLI Cargo dependencies, `crates/archy-cli/tests/core_workflow.rs` (Cargo-discovered integration target).

- [ ] Test CLI golden generation twice with different execution keys, then evaluate one/all from explicit and default source runs. Assert deterministic variant and metric bytes are identical without deleting meaningful provenance fields; run IDs/revisions differ.
- [ ] Implement shared read-time canonical validation and evaluation recomputation; test forged aggregate booleans, checks, metrics, footprint, ID, project binding and normalized input, including correctly rehashed forgeries. Unsupported recorded algorithms fail explicitly.
- [ ] Test retained FAIL/UNKNOWN candidates, exhaustion, edited current project files, selector ambiguity, JSON error shape, replay after later revisions and preservation of earlier runs. Implement only orchestration; task02 algorithms remain owned there.

### 3. Reports and immutable parameter edits

**Files:** create `crates/archy-report/{Cargo.toml,src/lib.rs,src/templates.rs}` and its tests; extend generate edit-mode parsing; workspace membership; curated `examples/courtyard-housing/expected/` and `tests/golden/` fixtures.

- [ ] Test source/preferences/professional limits in both formats, stable ordering, safe handling of untrusted Markdown/HTML-like text, and no approval/compliance inference.
- [ ] Test edit mode creates a derived ID and parent provenance while every original artifact hash stays fixed. Restore earlier parameters into another run and verify historical access. Reject forged IDs/footprints/summary flags in edit files and conflicting CLI modes.
- [ ] Generate reports from recomputed trusted data; preserve edit input bytes and provenance. No generic templating dependency unless simple deterministic formatting proves insufficient.

### 4. Evidence and A05 review gate

**Files:** source-derived run/artifact/revision schemas and `scripts/check-schemas.py`; dependency rationale; concise task03 handoff/recovery instructions and README CLI examples during implementation. This research session writes only this plan.

- [ ] Run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, and `python3 scripts/check-schemas.py` using the existing configured toolchain/target directory. Register golden tests in the CLI crate; a standalone root `tests/golden/*.rs` is not discovered by a virtual workspace automatically.
- [ ] Attach two-run byte comparisons, three golden manifests/evaluations/reports, failure retention, edit/restore hash inventory, crash/replay transcript and exact environment/dependency versions. Run the golden CLI against temporary copies, not the tracked example directory.
- [ ] Obtain independent requirements/security review after implementation, including the forged-summary regression and publication ordering. No agents are dispatched by this research task. Keep actual Omarchy/filesystem acceptance explicitly deferred and do not close A05 based solely on the first slice.

## Remaining uncertainties and handoff

No research blocker prevents this plan's first slice. Implementation prerequisites are the reviewed task02 handoff/fix and actual Cargo resolution for proposed additions. Exact UUID license/transitive inventory, crate compilation and target filesystem support remain unverified. Final task02 serialization/version strings must be taken from its reviewed source, not the in-progress snapshot observed here. No existing dependency/toolchain pins were changed or independently rebuilt.

Review focus: authority forgery with consistent hashes; interruption after publication but before acknowledgement; concurrent replay versus stale revision; symlink/hardlink/directory replacement; unsupported historical versions and corrupted history. Each is assigned a concrete test above. Revisions order Archy mutations; external source-file changes are detected by input digests, not magically serialized by the lock.

Completed in this session: full specification/guidance and relevant contracts/research read; primary-source mechanism checks; bounded design and test plan. Not run: product tests, Cargo resolution, backend or Omarchy execution. No product changes, commits, pushes, issue edits or agents. A05's full scope remains steps 1–4; the next authorized implementation can start with step 1 after task02 review.
