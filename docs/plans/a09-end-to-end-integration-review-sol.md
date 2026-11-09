# A09 combined gate, controller, accepted run and capture review

29 October 2026. Reviewed the current uncommitted product patch in the detached
`a09-end-to-end-integration-20261029` worktree at
`f1e2477047dbafbacfba3229daddab0ca58ebe02`. I read `AGENTS.md`, the
integration handoff and freeze, the original gate round-three and controller
round-three reviews, the controller handoff, and the accepted-binding review and
handoff. No product source, other worktree, installed file, commit or remote was
changed; no child agent was used.

## Verdict

**A09 is not ready for production acceptance.** Two combined-path defects were
reproduced: IFC output is written under a directory that capture never reads,
and a structured worker error is converted to generic service unavailability
before capture. The public adapter entry point also remains a provenance bypass
for direct callers. The fixed gate and runtime are absent at their installed
paths, so no positive controller-to-worker run or Omarchy acceptance is claimed.

## Reproducible findings

### P1: successful IFC output cannot be captured

`accepted_service_request` constructs IFC with `output_dir = "output/ifc"`
(`crates/archy-cli/src/accepted_worker.rs:43-49`). The gate binds the scratch
descriptor at `/job/output`, passes `/job` to the IFC worker, and checks
`/job/output/ifc/response.json` (`crates/archy-service-gate/src/main.rs:566-583,
715-746`). The worker creates the requested output directory and writes its
response there (`workers/ifc/archy_ifc_worker/worker.py:86-103, 230-250`). After
verified teardown, `run_accepted_service` gives the scratch **root** to
`capture_ifc`, which opens `response.json`, `model.ifc` and `validation.json`
from that root (`accepted_worker.rs:94-98`,
`crates/archy-adapters/src/artifacts.rs:24-27, 381-399`). It will return
`ARTIFACT_INVALID` for a successful IFC worker's nested response; the nested
`ifc` directory also violates capture's root allowlist. Existing adapter
fixtures create flat root files, so they do not exercise this combination.

Reproduce without an installed runtime by invoking the actual IFC worker's
`main` with a valid fixture request, `output_dir="output/ifc"`, and
`unittest.mock.patch.object(worker, "run", return_value={"status":"ok"})`.
The disposable probe in this review printed:

```text
worker_status= 0 gate_response_exists= True capture_root_response_exists= False
```

The mock replaces only backend generation; the worker's request parser, output
directory creation and response write are real. The gate and capture paths above
establish the resulting mismatch. A positive installed run is still needed
after correction.

### P2: structured worker errors are discarded before capture

For handled worker errors, both workers write a response with `status="error"` and exit 20
(`workers/blender/worker.py:194-211`,
`workers/ifc/archy_ifc_worker/worker.py:239-250`). The gate's `inside` path
rejects every nonzero worker status before its response check; the outer gate
then exits unsuccessfully (`crates/archy-service-gate/src/main.rs:465-482,
716-749`). `ServiceSession::finish` verifies teardown but returns
`BACKEND_UNAVAILABLE` for that unsuccessful unit before `run_accepted_service`
can call `capture_blender` or `capture_ifc`
(`crates/archy-adapters/src/service.rs:723-773`). The bounded, request-bound
IFC error envelope and Blender diagnostic response are lost to the caller.

A disposable invocation of the actual IFC worker's `main`, with a valid fixture
request and `run` patched to raise a `ProtocolError` after parsing, printed:

```text
worker_status= 20 response_status= error code= ARTIFACT_INVALID request_id= req-1
```

This is a source-to-source integration proof of the dropped response. It does
not claim a real backend failure was exercised through systemd. The gate must
distinguish a validated worker error response from a crash while preserving
the verified teardown and bounded capture boundary.

### P2: direct adapter callers can bypass accepted provenance

`BlenderRequest::new`, `IfcRequest::new`, `ServiceRequest` and
`ServiceSession::start` are public. The constructors validate syntax and
fixed output policy; `ServiceSession::start` calls `request.prepare()` without
`verify_locked` (`crates/archy-adapters/src/request.rs:20-72`,
`crates/archy-adapters/src/service.rs:31-62, 507-510`). A caller can supply a
well-formed arbitrary run ID and digest. The CLI wrapper does verify accepted
history under the project lock before constructing its owned request
(`crates/archy-cli/src/project_read.rs:65-87`,
`accepted_worker.rs:19-56`). Thus accepted provenance is a caller obligation,
not a service API invariant. No live gate was available to turn this into a
worker execution probe; do not describe the public adapter as accepted-only.

## Checked boundaries and limits

- The 28-file freeze manifest passes `sha256sum -c`; its SHA-256 is
  `ad5f70c252f786a46ca92a0a15118aa2a5035d383cc86de45dc4f691846e0880`.
  `git diff --binary f1e2477 -- Cargo.lock Cargo.toml crates packaging schemas scripts tests docs/mcp/README.md | sha256sum`
  reproduces the exact product patch SHA-256
  `fb11dc7be5e04f270e09e6dbb30511175116c11ee8c410ce76d0a2ea76c9db12`.
  Key actual file hashes: adapter `service.rs`
  `4e2d99754375541f8f5cacdfcb96b142da59093459463dea4bfc29473badfa0f`,
  unchanged `request.rs`
  `f0372fcaf865b0aa20402a20a028f98f3aa7189c38b6c85de7de54830d43c5ad`,
  CLI `accepted_worker.rs`
  `5be7a7bfb7fb0e407263bf6ed6843adf81f748648bb524799b94311a1e7390c2`,
  gate `main.rs`
  `5820c1f6c17820ac4218fe25222a6ac2d1c955ec9b0bea30a4cb6e02aa07af5c`,
  package `package.py`
  `d70231b4b35ea3c486a128748ff38da1d3f6f82da6c870fb1b65b1f542316c05`,
  notices `notices.py`
  `25f0d9a7f60c2a91e685f621a9ed3039f344e9971bc1f22761443caf6483974f`,
  and combined `Cargo.lock`
  `1e0607b6c71306d5e16566a65e2a12434d55b1cb861480a83f22f6b1958318e6`.
- The CLI wrapper verifies accepted revision membership, v1 semantic replay,
  project ID, variant ID and stored input digest under a cooperating lock. Its
  pre/post pinned root and source checks reject tested replacements. The owned
  request survives lock release without re-reading live source. A same-user
  noncooperating writer or an ABA rewrite is not excluded by that lock, and the
  self-consistent revision hash chain is not authenticated human approval.
- The controller uses fixed gate/systemd paths, verifies service identity,
  sends `GO` and one bounded typed seqpacket, then closes for EOF. Gate tests
  refuse missing/extra/empty packets, open peer, forged IDs/operations and
  unsafe output paths. `finish` stops the unit and verifies teardown before
  returning its retained scratch descriptor; capture caps each file and the
  aggregate at 128 MiB, validates response association and rejects extra,
  changed, linked or malformed files. No-observed-cgroup cleanup still fails
  closed. These checks do not remove the IFC layout or error-exit findings.
- The fixed package row, static ELF64 x86_64 program-header and `DT_NEEDED`
  predicate, and prewrite capture of all manifest bytes are present. The
  reviewed 2,064,008-byte release gate ELF has SHA-256
  `f61ce41849dca52a244492b6d2c33d821e1e7011a5b59dc0de925eb23de4bb5a`
  and passes `require_static_gate`. The A21 locked normal dependency check
  runs before destination creation over both CLI and gate roots; live offline
  metadata yielded 91 registry crates. Later-stage rollback is not generally
  transactional. There is no `vendor/` tree or clean Arch package build here.
- A15 `compare.rs`, MCP `server.rs`/`tools.rs`, `compare_stdio.py`, the compare
  schema and `check-schemas.py` match published `8e36e11d1399dee37dc2b85a8e2225b473571602`
  byte for byte. The stdio probe checked CLI parity, paging, tamper refusal,
  fixed source and no writes. A02 v2 verifier and active A15 mutation work
  still overlap CLI reader/exports, MCP surfaces and schemas; this review does
  not reconcile those unpublished changes.

## Validation

All Cargo work used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-e2e-review-sol-20261029`,
`CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0` and
`--locked --offline`. Commands were run from this worktree.

- `cargo test -p archy-cli --test accepted_worker -p archy-service-gate -p archy-mcp --lib`:
  exit 0, four binding tests passed. The `--lib` selection did not run gate
  binary tests; those ran in the next command.
- `cargo test -p archy-adapters -p archy-service-gate`: exit 0; 26 adapter unit,
  artifact/isolation/protocol suites, and seven gate binary tests passed.
  Native IFC and live receiver fixtures were ignored in this invocation.
- `python3 -m unittest -q tests.packaging.test_package tests.packaging.test_notices`:
  exit 0, 32 tests, including `PT_INTERP`-free `DT_NEEDED` refusal, static PIE
  admission, stale notice coverage and prewrite failure cases.
- `python3 tests/security/service_gate_unavailable.py <reviewed-release-gate>`:
  exit 0, five disposable WSL user services. GO-only, one request, extra
  nonempty/empty packets and open peer all received bounded private scratch,
  failed unavailable, were collected, and preserved the outside sentinel.
- `python3 crates/archy-mcp/tests/compare_stdio.py` with the same target env:
  exit 0 after building CLI and MCP binaries. The initial invocation without
  `CARGO_TARGET_DIR` failed because it sought an unbuilt `target/debug/archy`;
  that was a probe setup error, not an A15 failure.
- `cargo test -p archy-cli --test desktop_readers bound_reader_rejects_source_and_root_replacement_without_creating_history`:
  exit 0, one source/root replacement case passed.
- `python3 scripts/check-schemas.py`: exit 0 on this single independent run;
  nine MCP schema pairs, A15 compare stdio and six demo tests completed. This
  result does not resolve the separately investigated parse-error pipe-fill
  flake observed in the handoff's first schema run.

The installed `/usr/lib/archy/archy-service-gate` and
`/usr/lib/archy/runtime/inventory.json` are absent. No positive real-worker
run, cancellation/OOM/manager-loss teardown, complete worker and transitive
library/font dependency inventory, output publication, A13 independent IFC
verification, or Omarchy acceptance was established. The gate hashes selected
executables and entry scripts, not every transitive dependency. The A02 v1
verifier cannot admit v2 evidence; A15 mutation wiring and approval/publication
semantics remain separate integration work. The known schema parse-error
pipe-fill flake is under separate investigation; a rerun alone must not be
used to call that gate stable.
