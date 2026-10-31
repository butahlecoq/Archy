# A09 current-head integration: independent Sol review

29 October 2026. Reviewed the staged A09 integration in the detached
`a09-current-head-integration-20261029` worktree at exact HEAD
`ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1`. Read `AGENTS.md`, the
current-head handoff, patch and freeze, and the source A09 round-two review and
freeze. This review wrote only this document. No source edit, commit, push,
install, other worktree edit, or child agent was used.

## Verdict

**No new integration defect found in the focused, fixture-tested boundary.**
The 30 frozen source paths and 21 current-head product paths are byte-identical
to their manifests. The inherited A15 compare/MCP paths and escaped pipeholder
process fixture remain at `ff7f6bc`; the focused Rust, packaging, stdio, and
negative gate checks passed. This is a scoped approval of the current-head
patch for further integration review, not an installed-runtime or release
acceptance verdict.

## Exact baseline and hashes

| Item | Independently observed SHA-256 / result |
| --- | --- |
| Source 30-path freeze manifest | `def291c138778c8a41da75ba10f3c613a17297d3c857968c429a484acc3e40c7`; all 30 `sha256sum -c` entries passed |
| Source product diff against `f1e2477047dbafbacfba3229daddab0ca58ebe02`, restricted to the 30 manifest paths | `389e1a8ba61c7d41536c8c3bee2fdeeec751b2512c0674ac617bfc6254c169a8` |
| Current-head 21-path freeze manifest | `5db3783b944f1754783695850bea091e8027c4f525a6ebc2bb6a68df52e8b147`; all 21 `sha256sum -c` entries passed |
| `a09-current-head-integration.patch` and independently generated `git diff --cached --binary` | Both `42a9f7b86a1bc0862d9c0a7fa43c6973bacad7b21ce96f1e208fa310b37c98f2`; `git apply --reverse --check` and `git diff --cached --check` passed |
| Inherited `crates/archy-adapters/src/process.rs` | `a519321c480723e09eb2e417c5ac10c7d231373c5159ff0407653c31d66197ee`, equal to `git show ff7f6bc:...` |

The source diff digest was reproduced from this worktree using exactly the
source manifest's paths. A broad `crates` diff against `f1e2477` includes the
later `ff7f6bc` process change and has a different digest; it is not the source
A09 patch. `git diff ff7f6bc` was empty for `process.rs`, CLI `compare.rs`, MCP
`server.rs` and `tools.rs`, and the compare schema. The 30-path freeze also
covers the other inherited compare/MCP files. The current CLI `lib.rs` exports
both `render_verified_comparison` and the Linux accepted-worker wrapper;
`Cargo.lock` retains the MCP dependencies while adding the gate.

## Findings and limits

1. **Accepted binding is limited to the CLI wrapper.**
   `accepted_service_request` takes a locked, semantically verified accepted
   run through `BoundProjectRead`, checks the pinned project source and project
   identity, and builds an owned request with the stored run, variant, and input
   digest. The four binding tests passed current and historical, absent,
   foreign, stale, tampered, rehashed semantic forgery, and concurrent writer
   cases. Public `BlenderRequest::new`, `IfcRequest::new`, `ServiceRequest`, and
   `ServiceSession::start` remain direct-call policy/syntax APIs without that
   provenance check. A self-consistent revision chain does not authenticate
   human approval.

2. **Teardown and capture are bounded in the tested layers.**
   `ServiceSession::finish` stops the unit before returning scratch, requires
   inactive state and empty/absent cgroup evidence, and refuses malformed,
   missing, or denied evidence. The adapter library's 26 tests passed,
   including the unchanged escaped pipeholder fixture. Artifact capture uses
   pinned directory/file descriptors, `openat2` beneath/no-symlink constraints,
   type/link/size/identity checks, allowlists, claimed hashes, and aggregate
   bounds. Its nine tests passed missing, extra, symlink, hardlink, FIFO,
   traversal, digest, oversize, and late mutation cases. The actual IFC Python
   entry point wrote and returned response bytes under `output/ifc`; the
   Blender error entry point used flat `output/response.json`. Native backend
   generation was stubbed. IFC error identity is request-bound; Blender error
   bytes are an explicitly unbound diagnostic. Suspicious partial error output
   is recorded as `scratch_issue`, not a successful artifact capture.

3. **Exit and response agreement holds in the exercised boundary.**
   The gate's eight tests passed exact packet/EOF, missing/extra/open-peer,
   forged operation/path, and exit-class checks. Source inspection shows the
   inside gate accepts only worker exit 0 with response `status=ok` or exit 20
   with `status=error`; other exits or signals are unavailable. The CLI accepts
   success capture only with exit 0 and worker-error capture only with exit 20,
   after teardown. The negative disposable systemd probe passed five cases
   with bounded scratch FD, unavailable result, collected unit, and unchanged
   sentinel. It exercised prelaunch refusal with a locally built dynamic gate,
   not positive installed worker execution.

4. **Packaging and installation remain separate.**
   All 32 packaging tests passed static ELF admission, both CLI and gate
   locked-normal-dependency notice coverage, notice pair checksum, and
   prewrite capture/collision checks. `files.txt` has the fixed gate row but no
   runtime inventory or worker runtime. The locally built GNU gate has
   `PT_INTERP` and would be rejected by the static gate admission check; only
   `x86_64-unknown-linux-gnu` is installed as a Rust target here. The fixed
   `/usr/lib/archy/archy-service-gate`, runtime inventory, Blender, and IFC
   Python paths are absent. The gate hashes selected executables and entry
   scripts, not a complete transitive dependency/font set. There was no clean
   Arch package build, root-owned static gate installation, positive
   controller/systemd/bwrap-to-real-worker run, native Blender/IfcOpenShell
   generation, Omarchy acceptance, immutable artifact publication, or A13
   independent IFC verification. The unit-scoped preload environment probe
   was inspected but not run against this dynamic build; it requires a
   release-admissible static gate to support its intended claim.

5. **Known MCP schema flake is unresolved.**
   The source handoff records one `scripts/check-schemas.py` parse-error
   backpressure failure at 59,753 bytes against a 61,440-byte threshold and a
   later pass. This review ran the focused parse-error stdio probe once and it
   passed; that does not establish the full schema sweep is stable. No
   unreviewed repair was copied. Future A02 v2 and A15 mutation integration
   require their own overlap review.

## Commands run

Cargo used `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-current-head-review-sol-20261029`,
`CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`,
and `--locked --offline`.

- `cargo test -p archy-adapters --lib -p archy-adapters --test artifacts -p archy-service-gate --bin archy-service-gate -p archy-cli --test accepted_worker`: exit 0; 26, 9, 8, and 4 tests passed respectively.
- `cargo build -p archy-cli -p archy-mcp -p archy-service-gate`: exit 0.
- `python3 -m unittest -q tests.packaging.test_package tests.packaging.test_notices`: exit 0; 32 tests.
- `python3 crates/archy-mcp/tests/compare_stdio.py`, `stdio_contract.py`, and `parse_errors.py` with the same `CARGO_TARGET_DIR`: each exit 0.
- `python3 tests/security/service_gate_unavailable.py <isolated-target>/debug/archy-service-gate`: exit 0; five negative service cases.
- `cargo fmt -p archy-adapters -p archy-cli -p archy-service-gate -p archy-mcp -- --check`: exit 0.
- `sha256sum -c` for both freezes; scoped source `git diff --binary f1e2477`; `git diff --cached --binary`; `git apply --reverse --check`; `git diff --cached --check`; and the named `git diff ff7f6bc`: passed as detailed above.
