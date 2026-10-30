# A14/A16 bus request bounds follow-on

30 October 2026. Base `1e2313fe0bf20f69f246836334adb761276d2390`; isolated worktree `.worktrees/a14-a16-bus-confirm-20261030-unique`. This follows `a14-a16-bus-confirm-review-sol.md` finding 1. Integration head advanced separately to docs-only `6536a586c5486367e9e2d5e958862a1402e2a31a`; this candidate remains on its base and does not touch those docs. No commit, push, service activation, package staging, or other-worktree write.

## Plan and changed behavior

1. Validate the proposal digest before credential, logind, store or snapshot work. Match the store's exact 64 lowercase hex format; invalid input returns the generic denial.
2. Limit active `Review` calls to four with a fail-fast atomic slot released on completion or cancellation. Limit the service connection's zbus queue to four. Set a one-second timeout for each outbound method call and a three-second overall credential/session deadline. A timeout or overload denies.
3. Exercise the real served interface on a disposable private bus with real bus-assigned unique names and ProcessFDs. Serve a mock logind manager/session and a generated accepted proposal. Verify an exact positive review snapshot, invalid input before logind lookup, inactive/remote/wrong-UID denials, stalled logind deadline, and unconditional `Confirm` denial. Test slot exhaustion and release.

Direct dependencies `async-io =2.6.0` and `futures-lite =2.6.1` are already in the locked zbus graph; they add no registry package. The earlier source freeze remains provenance for the reviewed input. The new `a14-a16-bus-bounds-source.sha256` freezes the integrated source for independent review.

## Evidence and limits

`TMPDIR=/home/butahlecoq/work/archy` and `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/target` were used for Cargo checks on ext4. `cargo test --locked --offline -p archy-authority -p archy-core` passed across authority boundary/journal/protected tests and core suites. After the last source edit, `cargo test --locked --offline -p archy-authority --lib --test human_bus_boundary` passed: five library tests and one private-bus integration test. `cargo clippy --locked --offline -p archy-authority --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` exited 0. The initial mock test failed because its method was named `GetSessionByPid`; the fixture was corrected to the exact `GetSessionByPID` name and rerun successfully. The first clippy run flagged `&PathBuf` in the new fixture; it was changed to `&Path`, then clippy passed.

zbus 5.19.0 fixes the receive ceiling at 128 MiB in `message/header.rs` and `connection/socket/mod.rs` and exposes `max_queued`, but no per-connection message-byte setting. `Review` borrows `&str` and validates it before any outbound round trip; the transport has already received the message by then. The queue and active-call limits reduce concurrent work, but a smaller allocation limit remains a production bus policy/transport gate. The positive session is a mock on a private bus; native logind, distinct real UIDs, installed system-bus policy, disconnect/race/resource-pressure cases and production receive limits remain unproved. `Confirm` and both seqpacket `OP_DECIDE` routes still deny; no approval, Polkit flow or journal decision exists. The bus binary remains unstaged.

## Independent review paths

- `authority/src/human_bus.rs`: validation, slots, deadline, fail-closed methods.
- `authority/src/bin/human_busd.rs`: service queue and outbound timeout; still fixed identities and system bus.
- `authority/tests/human_bus_boundary.rs`: private-bus positive/negative/stalled logind fixture.
- `authority/Cargo.toml`, `Cargo.lock`: two direct edges to already locked packages.
- `docs/plans/a14-a16-bus-confirm-review-sol.md`, `docs/plans/a14-a16-bus-confirm-handoff-20261030.md`: finding and original authority boundaries.
