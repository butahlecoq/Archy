# Deferred Omarchy acceptance harness (A23)

This is an executable inventory and negative-evidence ledger for [A19](https://github.com/butahlecoq/Archy/issues/19) clean-machine validation and [A20](https://github.com/butahlecoq/Archy/issues/20) release acceptance. The binding criteria are [release specification §§2, 5, 7–7.2 and 9](../specs/release-0.1.txt) and [the acceptance matrix](acceptance-matrix.md). The harness does not install packages, run design mutations, inspect pixels, approve a variant, or certify a release. Its exit 0 means the inventory was emitted, **not** that any gate passed. Exit 1 means an observed failure remains; exit 2 means invalid source/input. `unknown` and `deferred` remain separate per gate; the aggregate keeps `unknown` when no failure is recorded. `target_acceptance` can become `pass` only for a current native transaction with complete gate evidence and a verified, independent human reviewer's signed attestation. The script checks identity, structure and bytes; the reviewer judges whether observations satisfy the predicates. Inventory and probe reports remain deferred.

## Bounded plan and basis

Implement a standard-library Python entry point with a fixed inventory. Pin the reviewed matrix content before reporting, optionally collect bounded read-only host/doctor observations, and accept only negative or deferred external observations. Positive decisions require a separate human reviewer signature from an administrator-owned trust store and matching native transaction and gate evidence. An agent JSON claim cannot establish a human decision or visual result. No Python package is added; the optional reviewed mode requires system `gpgv`, an administrator-provisioned reviewer keyring and fingerprint policy, and a fresh administrator transaction marker. A matrix edit requires a reviewed update to `REVIEWED_MATRIX_SHA256` and the predicate inventory. A changed native command interface or unavailable target evidence remains deferred.

Official upstream basis: [Omarchy v4.0.6 release](https://github.com/omacom/omarchy/releases/tag/v4.0.6) names the target ISO/tag; the [v4.0.6 shell plugin and IPC contract](https://github.com/omacom/omarchy/blob/v4.0.6/shell/README.md) documents `omarchy plugin`, `omarchy-shell shell ping`, discovery and enablement; the [Omarchy development-tools manual](https://omarchy.org/manual/development-tools/) documents mise and its lazy installation behavior. The pinned project compatibility manifest proposes Blender 4.5.15, isolated Python 3.12.16 and IfcOpenShell 0.9.0.post1 but marks backend runtime support unverified. The package is core-only; its source does not imply installed backend readiness. Do not use unqualified `blender`, `python`, `gh`, or agent commands as version evidence because mise may provision or shadow them.

## Run now: no installation or design mutation

From this repository root:

```sh
python3 scripts/validate-omarchy.py --dry-run > omarchy-dry-run.json
python3 -m unittest discover -s tests/acceptance/omarchy -p 'test_*.py' -v
python3 scripts/validate-omarchy.py --probe-readonly > omarchy-readonly.json
```

The last command executes only `/usr/bin/pacman -Q omarchy omarchy-dev`, `/usr/bin/archy doctor --json`, `/usr/bin/archy doctor`, and `/usr/bin/omarchy-shell shell ping`, from fixed root-owned, non-writable system paths, in a minimal environment, with a five-second bound and no mise auto-install. Missing, writable, or redirected system paths are unavailable. Dry-run does not launch any subprocess and reports `source_revision: null`; the candidate SHA must come from independently reviewed release evidence. A missing or timed-out executable has `status: unavailable` in `probes`; a completed process has `status: observed` and its actual exit, even when nonzero. Neither erases gates nor turns them into passes. Dry-run executes none of these probes. Store reports outside a source checkout for candidate evidence and hash them. The script is safe to rerun; it does not write files itself.

Negative observation input, for example `observations.json`:

```json
{"results":{"G06":{"status":"fail","reason":"candidate GLB did not parse"},"G07":{"status":"unknown","reason":"independent reader unavailable"}}}
```

Run `python3 scripts/validate-omarchy.py --dry-run --observations observations.json`. This exits 1, reports G06 `fail`, G07 `unknown`, and all other gates `deferred`. The input schema accepts only `fail`, `unknown`, or `deferred` with a nonempty reason. It rejects `pass`, extra fields such as a forged actor, and unknown gate IDs. These observations are evidence notes, not a route to positive certification.

Sample dry-run fields (abbreviated; the command emits all 27 gate objects):

```json
{"mode":"dry-run","target_requirement":{"omarchy_version":"4.0.6","platform":"installed native Omarchy/Arch Linux, not WSL/CI/container","architecture":"x86_64 reviewed package profile","blender_version":"4.5.15","python_ifc_version":"3.12.16","ifcopenshell_version":"0.9.0.post1"},"gates":[{"id":"G01","status":"deferred","required":true}],"release_status":"deferred","target_acceptance":"deferred"}
```

## Complete gate map and target evidence

Every ID below is present in each JSON report with `target_action`, status, reason, and `issues: [A19,A20]`. The A19 owner collects native evidence; A20 reviews the same candidate against the matrix. An actual pass requires the candidate commit/archive digest, package and worker lock/hash identity, machine model/host identity, Omarchy and kernel versions, exact command and exit, retained stdout/stderr, artifacts with SHA-256, and independent reviewer disposition. Recheck both the source and installed binary identity. WSL, CI, Arch containers, static source tests, and fixture outputs cannot satisfy a native gate.

| IDs | Required evidence on one named Omarchy 4.0.6 candidate |
| --- | --- |
| G01 | Two independent pinned full runs, identical canonical manifests/metrics, documented per-view pixel tolerance and measured differences. |
| G02 | Invalid site, polygon, setback and height cases with stable errors; required UNKNOWN does not approve. |
| G03 | Three feasible distinct variants at seed 42, or bounded infeasibility explanation. |
| G04 | Analytic measurements and parsed GLB/IFC bounds with explicit tolerance and unit. |
| G05 | Parse/hash every accepted output under its run ID and preserve earlier run digests. |
| G06 | Pinned headless CPU Blender produces three parseable GLBs and at least one preview per variant; separately demonstrate three camera views each (nine PNGs). GPU is optional extra evidence and cannot replace CPU. |
| G07 | IfcOpenShell reopen plus separately implemented web-ifc reader inspection of the candidate-produced IFC4 hierarchy, placement, geometry and quantities. |
| G08 | Real existing client stdio MCP transcript for generate → evaluate → render → compare with no manual 3D edit. |
| G09 | Absent/forged consent rejection, draft preapproval export, distinct local human decision event, unchanged history. A proposal or agent-supplied actor is never that event. |
| G10 | Timeout, crash, malformed response, duplicate, interruption and cancellation with stable errors and before/after accepted-run hashes. |
| G11 | Aggregate of G01–G10, C01–C09, clean install, README, candidate identity, security and independent review. |
| C01–C03 | Brief/provenance, immutable iterations, typed bounded workers and truthful comparison with three views. |
| C04–C06 | Schematic IFC semantics, complete CLI/nine MCP tool contracts, error/security and authority boundaries. |
| C07–C09 | Clean install/demo/benchmark, locks/notices/review, and Omarchy doctor/package/plugin integration. |
| OM-A21-01 | Clean native host identity and reviewed package transaction; `pacman -Q omarchy omarchy-dev` is an observation, not the install proof. |
| OM-A21-02 | Installed `/usr/bin/archy doctor` text/JSON with exits and independent nonmutation trace. |
| OM-A21-03 | Exact Blender bundle and isolated IFC interpreter/library identities, hashes, native smoke outputs. |
| OM-A21-04 | Effective containment: home/network denial, CPU/memory/tasks limits, timeout and descendant cancellation. |
| OM-A21-05 | Disposable target install/upgrade/removal and plugin-uninstall conflicts with user sentinels intact. |
| OM-A22-01 | Plugin validate/discover/enable/disable, panel lifecycle, themes, scaling, focus, Lua binding, fixed viewer launch and screenshots. |
| OM-A23-01 | Clean CPU golden journey, independent IFC inspection and visual contact sheet; optional GPU separately. |

## Target execution worksheet

A19 runs this worksheet on a disposable, clean, named Omarchy 4.0.6 x86_64 host against **one frozen candidate**. Record the candidate full commit and tree SHA, source archive and package SHA-256, lockfiles, installed `/usr/bin/archy` hash, host/kernel/CPU/GPU identity and exact worker versions first. Use a unique evidence directory outside the checkout. For every command below, retain the literal command, UTC time, exit code, complete stdout and stderr in a separate file, plus hashes of produced artifacts. A command expected to fail needs its expected error code and the observed code. A nonzero exit or missing command is a failure or unknown, never a pass. No agent may supply the human approval event or sign the reviewer record. This worksheet itself does not run mutating commands.

Set `CANDIDATE`, `PACKAGE`, `EVIDENCE`, `PROJECT`, `BRIEF`, `RUN`, `VARIANT`, `IFC`, and `EXPECTED` to absolute candidate-specific paths or IDs in a saved `inputs.txt`. `PROJECT` must be a new disposable path and `BRIEF` its frozen demo brief. Capture `sha256sum` inventories before and after each mutation. The demo brief must specify 20 × 35 m, three storeys at 3.2 m, 12 m height cap, 3 m clearance, and 900 m² **soft** GFA target. `scripts/demo.py` currently stops at render on this base; a future candidate must implement the missing production commands before these gates can pass. Do not substitute a worker fixture for the candidate's accepted run.

The command forms below are the minimum runbook. For a candidate that changes its CLI syntax, freeze the actual `--help`, replace the exact command in `inputs.txt`, and have A20 review that substitution against [the matrix](acceptance-matrix.md) and [the spec](../specs/release-0.1.txt). The cited guides supply detailed transaction and reader setup. Each heading is the stable `procedure` link in the JSON inventory.

### G01

Run `python3 scripts/demo.py --archy /usr/bin/archy --project-dir "$PROJECT-a"` and repeat with `"$PROJECT-b"`, using the same brief/seed/pins. Retain both full run trees and compare canonical variant manifests and metric JSON byte for byte after only documented volatile-ID normalization. Render corresponding views on CPU with identical settings; record renderer identity, a predeclared pixel tolerance, measured per-view differences and verdicts. Preserve both raw images and comparison output. Any missing image or over-tolerance view fails.

### G02

Run `/usr/bin/archy validate --project "$BRIEF" --json` as the positive control. Copy the brief into four disposable cases for zero/negative site dimension, self-intersecting polygon, impossible setbacks and excess height; run the same command for each. For geometry that enters only after generation, run `generate`/`evaluate` against that case and retain the rejected variant. Record stable error code, field, exit and required UNKNOWN state. A required UNKNOWN must not produce compliance or approval.

### G03

Run `/usr/bin/archy generate --project "$BRIEF" --count 3 --seed 42 --json`, then `/usr/bin/archy evaluate --project "$BRIEF" --all --run "$RUN" --json`. Retain variant IDs, footprint vertices, feasibility checks and a distinctness comparison. Three feasible geometries pass; a bounded, reproducible fewer-feasible explanation needs explicit A20 review.

### G04

Run `cargo test --workspace --locked`, `python3 -m unittest discover -s tests/backend/blender -p 'test_protocol.py' -v`, and `python3 -m unittest discover -s tests/backend/ifc -p 'test_native.py' -v` with the candidate's pinned backend environment. Retain the named `test_rectangle_from_analytic_source` and `test_analytic_rectangle_and_translated_l` results against `tests/fixtures/analytic/courtyard-shapes.json`; success exits 0 and failures exit nonzero. For an available independent fixture reader, run `cd tests/backend/ifc-reader && npm ci --ignore-scripts --no-audit --no-fund && npm test`; then run `timeout 10s node verify.cjs fixtures/l_shape.ifc fixtures/expected.json l_shape.ifc fixtures/source/l_shape.json` under an external memory/output limit. These are fixture checks only. For the accepted target run, freeze an independently reviewed parser command in `inputs.txt`, compare area, height, setbacks and world bounds from `/usr/bin/archy evaluate`, parsed candidate GLB and parsed candidate IFC, and save raw values, tolerances in metres/square metres, deltas and verdicts. **Deferred until that candidate-specific parser and accepted outputs exist.**

### G05

For every accepted run, save `find "$PROJECT/runs" -type f -print0 | sort -z | xargs -0 sha256sum` and parse JSON, GLB, PNG and IFC with their independent readers. Compare paths, producer, media type, size and digest against the run manifest. After retry/failure tests, regenerate the hash list and compare prior accepted runs byte for byte. Keep the manifest and both inventories.

### G06

Run `/usr/bin/archy render --project "$BRIEF" --all --run "$RUN"` using the pinned headless **CPU** Blender profile. Retain the exact Blender executable hash, version, settings and logs. Parse three GLBs and at least one PNG per variant; separately verify all three camera views for each of three variants (nine PNGs) in the full §2 journey. Open the contact sheet for human visual review. GPU results, if tried, are extra artifacts and cannot replace CPU.

### G07

Run `/usr/bin/archy export-ifc --project "$BRIEF" --variant "$VARIANT"` on the accepted candidate run. Reopen `$IFC` with the pinned isolated IfcOpenShell Python, retaining schema, project/site/building/three-storey hierarchy, conceptual typing, placements, quantities and bounds. Then run `node tests/backend/ifc-reader/verify.cjs "$IFC" "$EXPECTED" candidate.ifc "$PROJECT/canonical-source.json"` with the pinned Node and web-ifc lock from the [reader guide](../../tests/backend/ifc-reader/README.md). `$EXPECTED` and canonical source must be generated from this candidate's accepted run and reviewed independently. Retain both reader outputs and the IFC hash.

### G08

Freeze the actual installed MCP server launch command and the name/version of an existing stdio MCP client in `inputs.txt`. Use that client to call `get_project`, `generate_variants`, `evaluate_variant`, `render_variant`, and `compare_variants` with project-relative destinations, idempotency keys and expected revisions. Save the client's complete request/response transcript, process exit and produced run IDs; match IDs to artifact hashes and record that no manual 3D edit occurred. **Deferred while this candidate's `render_variant` is unavailable; no real-client invocation or expected output can honestly be specified for it yet.**

### G09

On a disposable project, first inspect the approval ledger with no decision. Attempt an agent `propose_approval` containing a forged `actor`; confirm it creates no approved state. Export an unapproved IFC and inspect its DRAFT marker. Then a physically present human uses the candidate's trusted local approval control and records the decision event; retain event timestamp, actor identity from the trusted control, selected variant, before/after ledger and unchanged prior-run hashes. The independent reviewer interviews/checks the human action and signs the evidence bundle. **Deferred until the candidate exposes that trusted control and a human actually uses it.** Agent text, a proposal, or a JSON `actor` field alone fails.

### G10

Run `python3 -m unittest discover -s tests/backend/ifc -p 'test_subprocess.py' -v` and `cargo test --workspace --locked`; retain the named `test_sleeping_child_times_out`, `test_descendant_held_pipes_do_not_hold_parent`, `test_validator_output_limit_is_artifact_error_with_retained_evidence`, and `test_fresh_process_scalar_json_is_artifact_error` results. Success exits 0 and failures exit nonzero. Snapshot accepted-run hashes. In disposable **installed candidate** projects, inject worker timeout, crash and malformed response; repeat a mutating tool call with the same idempotency key; interrupt and cancel a running worker, including descendants. Freeze the candidate-specific injection commands, fixtures and expected stable error codes in `inputs.txt` before execution. Retain process-tree trace, exits and post-test hashes. **Deferred until the candidate has a working accepted backend run and those injection controls; fixture tests alone cannot pass G10.**

### G11

A20 checks all G01–G10 and C01–C09 against this exact candidate, clean target install, default authority, README limits, required CI, issue blockers, licenses, protected-main settings and independent review. Record each URL or file hash at review time, plus a signed explicit release disposition. Any required fail, unknown or deferred blocks the aggregate.

### C01

Run `/usr/bin/archy validate --project "$BRIEF" --json` on equivalent YAML and JSON briefs plus missing/ambiguous unit, invalid field and missing-authority cases. Compare canonical SI output and inspect immutable snapshots for constraint source/authority and declared assumptions. Retain field errors and normalization diffs.

### C02

Starting from an accepted run, perform a candidate-supported edit into a new run with expected revision, then evaluate before render. Save parent/new run IDs, revision receipts, evaluation/render timestamps and hash inventories proving the original bytes persist. Retain a failed design with its explanation; reject any silent overwrite or render-before-evaluate path.

### C03

From the accepted candidate run, retain typed worker request/response JSON, timeout/cancel results, three views and GLB per variant, output verification receipts and comparison report. Check measured, heuristic, uncomputed and legal/professional claims remain distinguishable. Use the same `render`/`compare` commands and artifacts as G06/G08, with this additional contract review.

### C04

Inspect the candidate IFC from G07 for IFC4, schematic mass labels, project/site/building/storeys, local-to-world transforms and quantity parity with the evaluator. Preserve IfcOpenShell and separate web-ifc reader outputs; do not claim detailed wall/slab BIM semantics.

### C05

Save `/usr/bin/archy --help` and run `init`, `validate`, `generate`, `evaluate`, `render`, `compare`, `export-ifc`, `approve` on disposable projects, recording each JSON/text output and exit. Connect a real MCP client and call all nine named tools, including `list_variants`, `export_ifc` and `propose_approval`. Exercise stale revision, duplicate idempotency key and unscoped destination; verify the final approve action is outside agent authority.

### C06

Run `cargo test --workspace --locked`, `python3 -m unittest discover -s tests/backend/ifc -p 'test_protocol.py' -v`, and `python3 -m unittest discover -s tests/backend/blender -p 'test_a10_paths.py' -v`; retain the named traversal, symlink, authority and destination rejection tests, with exit 0 required for each suite. On the native candidate, run `python3 tests/security/scratch_fd_probe.py` and `python3 tests/security/systemd_scratch_fd_probe.py` only after their prerequisites and reviewed modes are recorded in `inputs.txt`. Separately exercise path escape, symlink/overwrite, denied home/network, resource limits, malformed worker response and prompt-sourced authority against the installed candidate. Freeze exact candidate commands and expected errors in `inputs.txt`; retain traces and protected-file hashes before/after. **Deferred until those native commands and observations exist.**

### C07

On a clean target, install the frozen package using the [package transaction guide](../omarchy/package.md), then run `python3 scripts/demo.py --archy /usr/bin/archy --project-dir "$PROJECT"` through render, compare, human review and IFC export. Record elapsed time, CPU/memory/hardware, output sizes, logs, hashes and visual/IFC results. A stop at render remains a failure.

### C08

Run candidate-bound `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, Python lint/tests and `python3 scripts/check-schemas.py`. Review exact lockfiles, shipped notices/license texts, `pacman -Qip/-Qlp "$PACKAGE"`, CI run commit, protected-main required checks and independent review state. Save command logs, package file list and dated tracker/settings snapshots; do not infer acceptance from an earlier CI run.

### C09

Combine the native A21 package/doctor/containment lifecycle and A22 plugin/desktop procedures below with an actual provider-neutral typed client session. Confirm no Herdr, API key or GPU is required for the CPU route. Retain package/plugin states before and after.

### OM-A21-01

On a clean target, run `/usr/bin/pacman -Q omarchy omarchy-dev`, `cat /etc/os-release`, `uname -a`, `lscpu`, `sha256sum "$PACKAGE" /usr/bin/archy`. Save pacman transaction history and compare installed file/hash inventory to the reviewed package. A query alone is inventory, not clean-install proof.

### OM-A21-02

Run `/usr/bin/archy doctor --json` and `/usr/bin/archy doctor`, recording stdout, stderr and exit separately. Trace filesystem and process activity during both calls and compare sentinel hashes before/after for nonmutation. Review reported missing capabilities without converting them to passes.

### OM-A21-03

Hash and run the pinned Blender executable `--version`; record its bundled Python and worker archive digest. Run the isolated IFC interpreter to print Python and IfcOpenShell versions and run native Blender/IFC smoke jobs on candidate requests. Compare installed runtime hashes to reviewed locks; a project `mise` shim is insufficient.

### OM-A21-04

Run the native worker containment fixture with denied home read/write and denied network, then CPU, memory, task-count, timeout and whole-descendant cancellation cases. Preserve `strace`/cgroup/process-tree evidence, exits and protected-file hashes. A declared limit without observed enforcement fails.

### OM-A21-05

On a disposable target with user sentinels, perform package install, upgrade and `pacman -R` plus plugin install/uninstall conflict cases from the [package guide](../omarchy/package.md) and [desktop guide](../omarchy/desktop.md). Snapshot user config and sentinels before/after; verify package and user ownership boundaries. Never run this on a primary desktop.

### OM-A22-01

Run `omarchy plugin validate integrations/omarchy/plugin`, `python3 integrations/omarchy/install.py`, then its reviewed `--apply` path on the disposable account. Record discovery, enable/disable, `omarchy-shell shell ping`, summon/toggle/focus, scale/theme and fixed viewer launch. Merge the sample Lua binding into user config only after checking collisions; capture screenshots and human observations. Disable and uninstall, preserving unrelated config. Follow the [desktop guide](../omarchy/desktop.md).

### OM-A23-01

Execute the clean CPU golden journey from C07, inspect the actual candidate IFC through G07's independent reader, and have a human review the complete nine-view contact sheet against the brief. Save signed visual notes with image/IFC hashes and any defects. If GPU is available, run it as an explicitly separate optional profile; it does not alter the CPU verdict.

## Reviewed record and verification

A19 keeps one flat evidence directory containing raw logs, parsed reports, contact sheet, IFC, command list and SHA-256 inventory. Before review, an administrator creates `/etc/archy/acceptance-transaction.json` with one newly generated UUID `transaction_id` for **this installation**, owned by root and not writable by group or world. Replace it after install, upgrade, or any new acceptance transaction; never reuse a UUID. A20 independently opens every artifact, checks the exact candidate and target installation, assesses each predicate, and writes `review.json` only after that work. A signature authenticates this reviewer's attestation; it does not make a false observation true.

The signed JSON has exactly `target`, `machine`, `reviewer`, `reviewed_at_utc`, `independent_human_review`, `decision`, and `results`. `machine` is the exact observed native host object; `reviewer` is the enrolled ID; `reviewed_at_utc` is a UTC timestamp in `YYYY-MM-DDTHH:MM:SSZ`; `independent_human_review` must be `true` and must describe a real review, not an agent assertion. `target` has exactly `candidate_revision`, `source_tree`, `source_archive`, `spec_sha256`, `matrix_sha256`, `compat_sha256`, `package`, `binary_sha256`, `locks`, `workers`, `installed_packages`, `transaction_id`, and `transaction_sha256`. The archive and package each have `path` and `sha256`; every lock and worker entry has `path`, `sha256`, and nonempty `version`. Use absolute target paths for these inputs, and include every shipped lock and installed worker artifact. `installed_packages` is the exact ordered stdout line list from `/usr/bin/pacman -Q archy omarchy omarchy-dev`. The verifier checks the current source HEAD/tree and clean tracked checkout, source/spec/matrix/compat bytes, archive/package/lock/worker bytes, installed binary, package query and administrator marker against the signed target. A different checkout, matrix predicate, worker, package revision or marker invalidates the record. The archive/package provenance and worker runtime versions still require A20's independent review.

Each of the 27 `results` has `status`, concrete `reason`, `manifest` (`name`, `sha256`), and `verified_predicates` (a map of checked predicate name to the human reviewer's nonempty disposition reason; every required name is present for a pass). Each gate has a distinct flat JSON manifest with exactly `gate`, `target_sha256`, `checks`, and `artifacts`. Compute `target_sha256` as SHA-256 of UTF-8 JSON encoding of `target` with sorted keys and compact separators. `checks` has exactly the predicate names in `PREDICATES` in the harness; each check has `status` and a nonempty list of filenames in the manifest's `artifacts` map. Each artifact filename is unique across gates, flat, and hashed. Use command receipts, actual stdout/stderr, parser outputs, process traces, human witness notes and visual notes as appropriate. The verifier checks names, hashes, gate binding, target binding, complete predicate coverage and that every passing check was explicitly verified by the reviewer. It cannot infer that a receipt or screenshot is honest. Missing/false observations must be `fail`, `unknown`, or `deferred`; a text file copied 27 times is not evidence. G09 requires independently witnessed local human approval evidence, and OM-A23-01 requires human visual review. `decision: "reject"` yields failure even if all rows say pass. No agent may create the independent review or hold the enrolled private key.

An administrator installs **only authorized human reviewers' public keys** in root-owned, non-group/world-writable `/etc/archy/acceptance-reviewers.gpg`. The JSON policy maps each **uppercase full primary certificate fingerprint** to one human reviewer ID. To inspect the primary fingerprint before enrollment, use `gpg --no-default-keyring --keyring /etc/archy/acceptance-reviewers.gpg --with-colons --list-keys`; the `fpr` line immediately following `pub` is the policy key. A signing subkey's `fpr` following `sub` is **never** a policy key. The verifier requires one `gpgv VALIDSIG` status, authorizes its final primary fingerprint against the policy, and reports both that enrolled primary and the actual signing fingerprint. A primary signature has the same value in both fields. A subkey-only policy entry is rejected. Enrollment and real human independence are operational controls; the verifier checks the cryptographic fingerprint and policy mapping, not the person's physical presence. The human signs the exact UTF-8 JSON bytes with `gpg --detach-sign --output review.sig review.json` using their own key. The target runs:

```sh
python3 scripts/validate-omarchy.py --reviewed-record "$EVIDENCE/review.json" --signature "$EVIDENCE/review.sig" > "$EVIDENCE/verified-report.json"
```

The verifier requires native x86_64 Omarchy identity, uses fixed root-owned `/usr/bin/gpgv` and an opened administrator keyring in a clean environment, checks the signed bytes, current transaction and every local evidence digest, and refuses unsigned/partial records. It opens the record, signature, keyring, manifests, policy and marker without following symlinks. Signed record and manifest JSON are parsed from the exact bounded bytes that were verified; evidence and target files are checked again before the disposition. The final checks detect changes during the scan, but separate files, pacman output and git state are not an atomic system snapshot. A writer changing files after the last check can still invalidate the report. Keep the evidence directory immutable and quiesce installation and source changes while verifying; rotate the administrator transaction UUID after every install or upgrade. The UUID cannot prove that rotation happened. It does **not** infer G09 approval from agent fields or certify release independently: A20 still reviews G11, issue disposition and the final candidate. A signed record from a key available to an agent is invalid evidence and must be revoked/reviewed. Retain the signed record, detached signature, trust-key fingerprint and verification report with the candidate.

## Handoff boundary

This checkout includes the provider-neutral Archy skill and Omarchy playbook as well as this single 27-gate harness. It also merges published a9c91d4 canonical v2 and historical v1 authority changes, which need fresh review in this combined tree. Its WSL dry-run and hostile-PATH tests prove only source behavior. No Omarchy package transaction, native worker, GPU, candidate IFC, visual review, human approval or signed target decision occurred here. A19 supplies native files; A20 independently reviews and signs before any positive target report. The integrated tree still needs independent review before publication.

## Validation and next action

On the 2026-10-30 WSL host, `python3 -m unittest discover -s tests/acceptance/omarchy -p 'test_*.py' -v` passed 27 integrated tests. They include disposable GPG keys and real `gpgv` signature checks, forged executable PATH shims, unsigned-review rejection, and signed-record tampering cases. `python3 scripts/validate-omarchy.py --dry-run` emitted all 27 gates as deferred, with zero probes and a null source revision; `--probe-readonly` also deferred all 27 gates and reported four unavailable fixed-path probes. These tests use modeled target inputs and do not establish native Omarchy or human-review evidence.

Next: review this corrected integrated tree, including the signed-record trust path and gate procedures. The Omarchy playbook already links to this single 27-gate inventory; there is no duplicate nine-gate harness in this checkout. A19 must run the candidate on native Omarchy and record actual evidence. A20 must independently review and sign that evidence. Keep issue #23 and the target release gates open; all 27 target gates remain deferred on WSL.
