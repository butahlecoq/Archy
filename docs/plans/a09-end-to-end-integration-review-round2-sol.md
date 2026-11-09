# A09 end-to-end integration: independent round-two review

29 October 2026. Reviewed the revised, uncommitted A09 product in the detached
`a09-end-to-end-integration-20261029` worktree at
`f1e2477047dbafbacfba3229daddab0ca58ebe02`. Read `AGENTS.md`, the A09
handoff, the prior Sol review, and the refreshed freeze. No product source,
other worktree, installed file, commit, push, or remote was changed. No child
agent was used.

## Verdict

**The two previously reproduced combined-path defects are corrected within the
fixture-tested boundary. A09 is not a complete release or installed gate.**
The actual IFC worker entry point now writes under `output/ifc`, where capture
reads through a pinned directory; a handled worker error exits 20 and its typed
response survives capture after verified teardown. I found no new defect in
these focused paths. Positive gate-to-worker execution under the production
systemd/bwrap/runtime installation is still unverified. Direct public adapter
callers still bypass the CLI's accepted-run provenance check.

## Freeze and baseline

- `sha256sum -c docs/plans/a09-end-to-end-integration-freeze.sha256`: all 30
  entries `OK`. Manifest SHA-256:
  `def291c138778c8a41da75ba10f3c613a17297d3c857968c429a484acc3e40c7`.
  `git diff --binary f1e2477 -- Cargo.lock Cargo.toml crates packaging
  schemas scripts tests docs/mcp/README.md | sha256sum` reproduced product
  patch SHA-256
  `389e1a8ba61c7d41536c8c3bee2fdeeec751b2512c0674ac617bfc6254c169a8`.
  Key source hashes: gate `2d9e89a0`, service `6ed943e7`, capture `b71147c3`,
  accepted wrapper `cb61338d` (full values in the verified manifest).
- This worktree's `HEAD` is `f1e2477`, not `ff7f6bc`. The separate
  `a16-integration-current-20261029` worktree was read only and resolved to
  `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1`; its `git merge-base` with
  A09 is `f1e2477`. Relative to that head, A09 still differs in `Cargo.lock`,
  `Cargo.toml`, CLI `lib.rs` and `project_read.rs`. The later head's comparison
  export is present, but its integration state and active A02/A15 overlap
  cannot be inferred from this A09 test run. No claim of a merged baseline.

## Findings and exercised behavior

1. **IFC layout and capture: corrected in the fixture.**
   `accepted_service_request` selects `output/ifc`; the actual IFC `main`
   creates it and writes `response.json` there. The artifact integration test
   invokes that entry point with only native generation stubbed. It observed
   exit 0, then captured the response, `model.ifc` and `validation.json` from
   the outer `output` descriptor. `capture_ifc` permits only `ifc` at that
   root, opens it with `openat2` and `BENEATH|NO_SYMLINKS|NO_MAGICLINKS|NO_XDEV`,
   checks its identity again after capture, and checks the nested allowlist,
   claimed hashes and byte limits. The same test refused forged identity,
   missing response and an extra root file; the symlink test refused an `ifc`
   link to an outside sentinel. The library test rejected a late same-size
   write. These are descriptor-level fixture checks, not a live tmpfs mount.

2. **Handled errors and exit/response agreement: corrected in the tested
   layers.** The actual IFC entry point, with native `run` made to raise a
   `ProtocolError`, exited 20 and produced a request-bound error response in
   `output/ifc`. The Blender entry point, with a stub `bpy` import and `_run`
   failure, exited 20 and produced its flat `output/response.json` diagnostic.
   Capture retained both response byte streams; IFC error identity was checked
   against the request, while Blender's schema has no request identity and is
   explicitly marked `UnboundDiagnostic`. Gate code accepts only exit 0 with
   status `ok` or exit 20 with status `error`; its unit test rejects crashes
   and signals. `ServiceSession::finish` stops and verifies the unit before
   returning `WorkerExit`; the CLI wrapper rejects either success/error
   mismatch after typed capture. No live systemd injection of a mismatched
   response was performed, so this conclusion combines unit execution with
   source inspection of the gate and wrapper.

3. **Tampering and cleanup: fail-closed cases exercised.** The artifact suite
   refused missing, extra, symlink, hardlink, FIFO, traversal, wrong digest,
   oversized and malformed output. A handled error with suspicious partial
   scratch remains a `WorkerError` carrying `scratch_issue`; that issue is not
   silently promoted to a successful capture. Adapter library tests verified
   cgroup proof refuses missing or malformed events and denied lookup. The
   disposable negative service probe passed five cases (GO only, one request,
   extra packets and open peer), each reporting bounded scratch, unavailable,
   collected unit and unchanged outside sentinel. This probe used the earlier
   reviewed release gate ELF (`f61ce41849dca52a244492b6d2c33d821e1e7011a5b59dc0de925eb23de4bb5a`),
   so it establishes negative service behavior only, not the revised gate's
   positive path or its exit-20 propagation under installation.

4. **Provenance remains a scoped limitation.** The four accepted-worker tests
   passed current/historical binding, absent/foreign/stale/tampered evidence,
   semantic forgery and concurrent lock cases. The CLI builds an owned request
   through `BoundProjectRead::with_verified_locked_run` under its cooperating
   project lock. Public `BlenderRequest::new`, `IfcRequest::new`,
   `ServiceRequest` and `ServiceSession::start` still validate syntax and fixed
   policy without independently verifying accepted history. Thus the CLI
   wrapper is provenance checked; the adapter API is not accepted-only. A
   self-consistent revision chain also does not authenticate human approval.

## Commands and limits

Cargo used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-e2e-review-round2-sol-20261029`,
`CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`,
`--locked --offline` from this worktree.

- `cargo test -p archy-adapters --test artifacts -p archy-service-gate --bin
  archy-service-gate -p archy-cli --test accepted_worker`: exit 0; 9 artifact,
  8 gate and 4 accepted-worker tests passed.
- `cargo test -p archy-adapters --lib`: exit 0; 26 tests passed, including
  descriptor mutation and service teardown refusal cases.
- `python3 -m unittest -q tests.packaging.test_package
  tests.packaging.test_notices`: exit 0; 32 tests passed.
- `python3 tests/security/service_gate_unavailable.py
  <reviewed-release-gate>`: exit 0; five negative disposable service cases.
- `git diff --check`: exit 0.

`/usr/lib/archy/archy-service-gate`, the root-owned runtime inventory,
`/usr/bin/blender` and the fixed IFC Python are absent here. There was no
positive installed controller/gate/backend run, real native Blender or
IfcOpenShell generation, full dependency inventory, clean package build,
immutable artifact publication, A13 independent IFC verification, or Omarchy
acceptance. A02 v2 and active A15 integration still need reconciliation against
the separate current baseline. The focused verdict must not be read as release
approval.
