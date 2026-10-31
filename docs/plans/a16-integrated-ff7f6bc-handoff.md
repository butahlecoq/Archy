# A16 proposal ingress and protected journal integration handoff

Date: 2026-10-29. Base: `ff7f6bc60dcc530acd834fe979ff63fb1d3e32e1`.
Worktree: `/home/butahlecoq/work/archy/.worktrees/a16-integrated-ff7f6bc-20261029` (detached, uncommitted).
This is a review candidate, not an authority deployment or approval path.

## Scope and source identity

Copied the 16 authority crate, test, design and package-input files from the independently reviewed A16 human-decision tree. The 14 unchanged files match `a16-human-decision-source.sha256`; `journal.rs` and `journal_boundary.rs` match the later `a16-journal-encoded-size-fix.sha256`. The size-fix patch itself matches its reviewed SHA-256. The current-head ingress freeze matched 15/16 entries in its source tree; its `Cargo.lock` entry had changed after that freeze, so the lockfile was reconciled from `ff7f6bc` instead of copied. The integrated authority files were checked against the review manifests after copying.

The root manifest adds only `authority` to the existing workspace member list. `Cargo.lock` adds only the local `archy-authority` package with its already locked dependencies; it retains the A15 `archy-mcp` dependency on `archy-report`. No registry package or version changed. The integrated source and retained overlap files are frozen in `docs/plans/a16-integrated-ff7f6bc-source.sha256` (26 entries). The review patch and adjacent checksum are in `/home/butahlecoq/work/archy/.tools/a16-integrated-ff7f6bc.patch` and `.patch.sha256`.

The subsequent published head `160787d1f6c65a85d0e022a111d56ee7b0eb2b86` adds reviewed A15 MCP generate/evaluate work on `ff7f6bc`. It changes `crates/archy-mcp/src/server.rs`, `crates/archy-mcp/src/tools.rs`, and `scripts/check-schemas.py`, among other files. This A16 patch remains pinned to `ff7f6bc`; it has no diff in those three paths, and the 26-entry source freeze records the `ff7f6bc` bytes for them. A later integration must retain the `160787d` versions and its CLI, storage, schema, test, and documentation changes, then refresh any combined-tree freeze and validation. `160787d` did not alter `Cargo.toml`, `Cargo.lock`, or `authority/`.
The frozen patch passed `git apply --cached --check` against a disposable index loaded from `160787d`; this checks patch applicability only, not combined-head behavior.

## Overlap with published work

`ff7f6bc` contains A02/A04 versioned history and recovery, A09 fail-closed service control, A15 comparison/MCP work, and the escaped pipeholder process fix. Relative to this base, no file under `crates/`, `schemas/`, `scripts/`, `packaging/`, or `.github/` changed. In particular, this integration leaves the A02/A04 history code, A09 controller and process code, A15 comparison CLI/MCP/schema checker, and package notice/staging logic at their published bytes. The root manifest and lockfile are the only overlapping tracked files. The authority package inputs remain unstaged by the Arch package; their native unit behavior and runtime notice inventory need separate acceptance before shipping.

The A14 decision contract is unchanged. The daemon opens and verifies the broker-private journal at startup, but has no append call. Both seqpacket channels deny `OP_DECIDE`. The journal accepts opaque, bounded non-decision records and enforces the serialized 64 KiB limit before file creation. There is no decision writer, system-bus method, session/Polkit binding, protected-history recheck, approved export, or final authority deployment. IFC exports stay `DRAFT`.

## Validation

All Cargo commands use `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.cargo-target-a16-integrated-ff7f6bc-20261029`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `RUSTUP_AUTO_INSTALL=0`.

- `cargo check --workspace --offline`: passed.
- `cargo test -p archy-authority --locked --offline --quiet`: passed; 2 library, 1 display, 13 real socket boundary, and 8 journal boundary tests.
- Direct probe compiled against this tree's `archy-authority` library: 65,536 encoded bytes accepted and reopened; 65,537 rejected with zero files; exact replay retained sequence/hash; subsequent append received sequence 2; maximum escaping rejected with a clean reopen; pending recovery and malformed pending denial passed. Probe source is `/tmp/a16_round2_probe_sol.rs` from the independent rereview, compiled to `.tools/a16-integrated-exact-probe`.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed after changing only the edited manifest line ending to LF.
- `python3 -B -m unittest discover -s tests/packaging -q`: passed, 24 tests. This tests existing CLI package staging and notices, not authority installation.
- `cargo metadata --locked --offline --format-version 1 --no-deps`: passed; eight workspace members including `archy-authority`.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: exited 0. Log: `/home/butahlecoq/work/archy/.tools/a16-integrated-ff7f6bc-clippy.log`; exit file: adjacent `.exit`.
- `timeout 420s cargo test --workspace --locked --offline --quiet`: the first detached process ended with completed suites but no retained exit status, so it is not counted. A fresh rerun with an explicit exit file exited 0: 47 suites, 223 passed, zero failed, two ignored. Log: `/home/butahlecoq/work/archy/.tools/a16-integrated-ff7f6bc-workspace-test-final.log`; exit file: adjacent `.exit`.
- `python3 scripts/check-schemas.py`: exited 0. Log: `/home/butahlecoq/work/archy/.tools/a16-integrated-ff7f6bc-schema.log`; exit file: adjacent `.exit`. It includes the current `ff7f6bc` MCP schema and stdio checks, not the later `160787d` mutation checks.
- The integrated 26-entry source freeze passed `sha256sum -c`. All 16 authority inputs match the independent review's source hashes, using the two reviewed post-fix hashes for `journal.rs` and `journal_boundary.rs`. The direct exact-size probe exited 0 on this tree: 65,536-byte accepted and reopened, 65,537-byte rejected, maximum-escape rejected, pending recovery and malformed pending denial exercised.

Final patch identity is recorded in the adjacent `.patch.sha256` file; verify it before review or application.

## Review focus and evidence limits

A separate integration reviewer should inspect the patch against `ff7f6bc`, verify the adjacent checksum and source freeze, and rerun the exact-size and socket tests. Check that the journal size fix precedes hashing/file creation; `OP_DECIDE` cannot reach `Journal::append`; the daemon has no append route; the lockfile retains A15 comparison dependencies; and A02/A09 source is untouched. The first independent review found the escaped-payload defect, and its round-two rereview accepted the fix for this scoped primitive. This combined patch has not yet received a separate independent review.

This WSL2 host does not provide native Arch/Omarchy effective systemd units, distinct installed agent/broker/human identities, a verified local session, Polkit challenge, or installed authority package. The tests use one host UID for transport fixtures and configured negative UID cases. They do not establish a human-positive approval path. The journal hash chain detects local corruption but has no independent rollback anchor and does not validate proposal truth or protected run history. No power-cut durability test was run. No commit, push, install, approval writer, policy change, or child agent was made.
