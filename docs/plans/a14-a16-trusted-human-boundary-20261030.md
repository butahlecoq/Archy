# A14/A16 trusted human boundary: isolated implementation plan

Base: `15446eeaf316846936a093377041b8c1d2874092`. Worktree: `a14-a16-trusted-human-boundary-20261030`. No commit, push, package install, or child agent.

## Scope and threat boundary

The published A16 proposal store protects submitted bytes but contains agent claims; the journal is opaque and has no decision writer. A14 claims cannot become authorization. This slice builds a broker-owned project/evidence verifier and a bounded review snapshot whose digest can be bound to a future unique bus sender. It does not enable a decision endpoint before a real system-bus/Polkit/Omarchy test. A project located in an agent-writable directory is always rejected even when its hashes and checks pass. The legacy project history itself does not enforce ownership; this wrapper must enforce it before calling the existing trusted-run recomputation.

## Steps

1. Freeze the input source hashes in the handoff, and record primary D-Bus/logind/Polkit/zbus semantics and dependency choice. Reuse already locked `archy-cli` verification and comparison code; change `Cargo.lock` only for the local dependency edge, with no new registry packages.
2. Expose the existing accepted-history run verifier to the authority crate. Add a protected project opener that rejects symlinked or non-broker-owned/mode-0700 project roots and non-private/non-broker-owned project files. Check the root identity again before evidence use.
3. Under the project lock, verify the proposal's accepted revision/head, recompute the selected run and comparison, and require exact input/normalized/variant/evaluation/comparison digests and checks. Return a bounded immutable snapshot with exact renderable bytes and a broker-calculated digest.
4. Add negative fixtures for agent-writable roots, forged passing checks, stale head, missing run, changed evidence, and symlink roots. Exercise one genuine generated run with the real CLI verifier and comparison renderer. Probe unique sender/credential behavior on a disposable private D-Bus daemon and reject missing/mismatched pidfd inputs in Rust. Run targeted locked Cargo tests, format and diff checks; document the separate bus, Polkit, protected writer, final journal transaction and Omarchy gates for Sol review.

## Integration overlaps

`Cargo.lock` adds `archy-cli` and already locked `rustix` edges to `archy-authority`; no registry versions change. `authority/Cargo.toml` adds the matching dependencies. `rustix` supplies descriptor-relative `openat2` and `statx` for the path boundary. `crates/archy-cli/src/lib.rs` exports its existing verifier. These are potential overlaps with A15 follow-on work; preserve both exports and any newer lockfile when integrating. No existing A16 ingress/journal behavior is removed.

## Independent review repair (30 October)

Sol reproduced acceptance through a writable or symlinked ancestor using a
real generated run. The repair pins each absolute path component with
descriptor-relative `openat2`, rejects writable ancestors and records device,
inode and mount ID for rechecks. It rejects a source inode or byte change
after opening. The accepted run's source artifact remains the historical
review input, even if a later trusted source edit exists when a new reviewer
opens. The bounded history reader rejects excess entries, bytes and per-run
work before recomputation. Tests use real generated runs and exercise
ancestor swaps, source replacement, hardlinks and resource limits. The final
protected writer, D-Bus/logind/Polkit binding and journal transaction remain
the next separate authorization slice.
