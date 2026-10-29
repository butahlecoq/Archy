# A16/A14 human decision follow-on handoff for independent Sol review

Date: 2026-10-29. Base: published `f1e2477047dbafbacfba3229daddab0ca58ebe02`.
Worktree: `/home/butahlecoq/work/archy/.worktrees/a16-human-decision-f1e2477-20261029`.
The reviewed A16 source was applied from the frozen
`.tools/a16-ingress-follow-on-f1e2477.patch` after its SHA-256 check. The
original reviewed worktree was not edited. No commit, push, host install,
policy change, or child agent was made.

I read the three independent Sol rounds in the original A16 broker worktree.
They accepted the final slice only as untrusted proposal ingress and
read-only review. The remaining `SO_PEERCRED` connected-FD delegation and
listener-path identity limits matter for future authority: this branch does
not reuse that socket credential as consent and still requires native
effective-unit checks. The reviewed availability bound is unchanged.

## Scope

- The reviewed systemd seqpacket proposal ingress, distinct UID checks,
  broker-private proposal store, and read-only review client are retained.
- `OP_DECIDE` is explicitly denied on both existing channels. No final
  selection, withdrawal, authorization token, approved projection or export
  status change exists. A14 `DecisionRecord` remains untrusted data.
- A dedicated broker-private journal primitive stores opaque, bounded UTF-8
  records with sequential hash-chain names. Opening requires the broker UID
  and checks ownership/mode; it uses a
  process mutex and Linux `flock`, writes a synced temporary file, renames
  create-only, syncs the directory, rejects changed/unknown/malformed entries,
  and supports exact idempotent replay. Startup removes only correctly named,
  broker-owned private pre-rename files under the journal lock. The daemon verifies the journal at
  startup but has no code path that appends or interprets a decision event.
  Tests append only explicitly labeled non-decision probe records.
- `authority/DESIGN.md` gives the native system-bus/logind/Polkit and
  protected-evidence integration sequence. It does not claim those steps are
  implemented or tested on WSL.

## Authority limitations and next integration

The journal does **not** bind an approval to a verified proposal, artifact
set or accepted history. The current A14 contract verifies the shape and
some claimed evidence hashes but cannot recompute evaluation or establish
history authority. Existing project history can be agent-writable. The
read-only proposal store protects submitted bytes after ingress, not their
truth. No same-UID CLI, terminal prompt, actor string or local packet can
authorize. The journal also has no independent monotonic rollback anchor
against broker/root compromise.

In a disposable native Arch/Omarchy VM: (1) install root-owned binaries and
fixed policy, create distinct `archy-agent`, `archy-broker` and enrolled human
host UIDs, confine the whole agent tree, and verify effective socket/service
ownership and state filesystem; (2) implement a system-bus review method
that derives unique sender/UID/PID, checks a live active local logind session
without PID reuse, and binds one displayed snapshot/digest to that sender;
(3) after explicit client selection, require a fresh non-cached Polkit
`auth_self` challenge, rejecting immediate YES, retention, cancellation,
disconnect, timeout and session/policy change; (4) move authoritative
history/artifact storage under broker ownership, verify inclusion and hashes,
recompute evaluation and comparison, recheck revision/head under the commit
lock, then append a decision event with broker-derived actor/time; (5) run
human-positive and separate-agent negative probes, crash/replay/rollback
checks, and packaging notice admission before enabling final approval.
Until then, `archy-review` only displays an `UNVERIFIED` proposal and IFC
exports remain `DRAFT`.

WSL2 probe: kernel `6.6.87.2-microsoft-standard-WSL2`, developer UID 1000.
The system bus, logind and Polkit names are present, but no `archy-agent` or
`archy-broker` account exists. A disposable `unshare -Ur` namespace maps the
developer to namespace root; attempting an unmapped second UID with
`setpriv --reuid 1` failed `setresuid: Invalid argument`. Thus no actual
distinct-host-UID broker test or human challenge was obtained here. This is
not evidence of a target Omarchy approval path.

The A15 compare publication `8e36e11d1399dee37dc2b85a8e2225b473571602`
arrived during this work. This worktree remains based on `f1e2477` and does
not import that commit. `git show --stat 8e36e11` shows A15 changes
`Cargo.lock` (one `archy-report` dependency in `archy-mcp`), CLI comparison,
MCP sources/tests, the comparison schema and the schema checker; it does not
change root `Cargo.toml`. At integration, retain that lock entry while adding
the local authority member and its dependencies; keep A15's schema checker,
CLI and comparison semantics, and preserve the separately
versioned A02/A04 history work. The new journal has no A15 schema or CLI
surface. Existing package staging still excludes authority binaries; do not
ship the units until notice and native gates pass.

## Independent Sol review focus

Use independent Sol at medium reasoning to review the frozen diff against
`f1e2477`, not the original A16 worktree.
Challenge whether the journal's lock spans scanning and rename, replay after
uncertain fsync is durable, malformed files fail closed, rollback and external
writer assumptions are stated accurately, and no seqpacket route reaches
`Journal::append`. Inspect the explicit denial of decision opcodes, display
escaping, distinct UID startup, packaging scope and A15 merge guidance.
Do not simulate Polkit success or create an approval record as test data.

## Validation and frozen identity

Final focused `cargo test -p archy-authority --locked --offline --quiet`
exited 0: 2 unit tests, 13 seqpacket boundary tests, 7 journal boundary tests
and 1 display test; no failures. Final `cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
exited 0. An earlier focused rerun failed once because parallel tests used
the same temporary fixture path (`AlreadyExists`); both authority test
fixtures now use a process-local atomic suffix, and the final rerun passed.
The final-source `timeout 420s cargo test --workspace --locked --offline
--quiet` exited 0: 47 reported suites, 222 passed, zero failed, two ignored
target gates. Log:
`/home/butahlecoq/work/archy/.tools/a16-human-decision-workspace-test-final.log`.
`scripts/check-schemas.py` exited 0: 50 A14 cases, nine MCP schema
pairs and the six demo cases passed; the doctor CLI report was structurally
valid while target acceptance remains deferred. Log:
`/home/butahlecoq/work/archy/.tools/a16-human-decision-schema.log`.
Unique SSD target:
`/home/butahlecoq/work/archy/.cargo-target-a16-human-decision-f1e2477-20261029`.
The frozen patch is
`/home/butahlecoq/work/archy/.tools/a16-human-decision-f1e2477.patch`,
outside all worktrees. Verify its adjacent `.sha256` before reviewing or
applying.
The current source hash manifest will be
`docs/plans/a16-human-decision-source.sha256`; the inherited ingress manifest
is historical input identity only.

Direct executable probes on this host: `archy-authorityd` exited 1 with
`authority unavailable: required account missing`; `archy-review` exited 1
with `review unavailable: No such file or directory (os error 2)` for the
absent installed socket. These show closed local startup, not native account
or human authentication acceptance.
