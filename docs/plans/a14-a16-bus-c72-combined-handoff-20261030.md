# A14/A16 human-bus combined candidate on c72, 30 October 2026

## Scope and provenance

The first integration commit `b2ed5ae5786bda01667f8e2dfdd86937d48e690a` ports the frozen A14/A16 bus candidate onto published `c72f5309a681f36f56cb05e3932486bb7410a3f0`, preserving all twelve A23 additions and the current v2/historical verifier. This follow-on applies the six-file source patch from reviewed commit `5a7f2e774d66854669bf6c2d78ed0cfc65896a90`; the patch SHA-256 was verified as `483039964b2c2be1a7152a52383bba57483d39d44f49046d7a6d7a1cf0f66319`. `git apply --check` succeeded without conflicts. The review PASS is scoped to that old-base source patch; this combined tree is frozen separately for independent review.

The patch adds a final unique-sender, pidfd, credential and active local session recheck before `Review` returns protected bytes. It adds a private-bus session transition fixture. The daemon retains its 65,536-byte receive cap, gives each short incident burst a finite four-incident budget, and resets that budget after a connection lasting at least 60 seconds. Proposed systemd unit, bus policy and deployment notes are source files only; they are neither packaged nor installed.

The first integration's historical `a14-a16-bus-a9-integration-source.sha256` describes `b2ed5ae5` and is superseded in this tree. The new `a14-a16-bus-c72-combined-source.sha256` lists every path changed against `c72f5309` except itself. The commit ID supplies the exact Git tree identity.

## Reconciliation and checks

The follow-on patch does not edit `crates/archy-cli`, `crates/archy-core`, schemas, MCP or A23 paths. The current verifier still checks canonical v2 `normalized.json` and `profile.json`, accepts authentic historical v1, and binds accepted artifact bytes in protected snapshots. After patch application, `cargo +1.99.1 test --locked --offline -p archy-cli --test golden_artifacts --test accepted_worker --test desktop_readers` exited 0 (4, 5 and 8 tests). The earlier first-integration full authority/core/CLI/MCP suite and schema/serializer script exited 0; see its handoff for exact commands.

From this combined tree, `cargo +1.99.1 test --locked --offline -p archy-authority --lib --bin archy-human-busd --test human_bus_boundary --test bus_recovery --test zbus_receive_bound --test protected_project` exited 0 (6 library, 1 daemon, 1 private-bus boundary, 1 recovery, 4 receive-bound and 10 protected-project tests). `cargo +1.99.1 clippy --locked --offline -p archy-authority --all-targets -- -D warnings` and `cargo +1.99.1 fmt --all -- --check` exited 0. Cargo used `TMPDIR=/home/butahlecoq/work/archy` and unique ext4 `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/a14-a16-bus-a9-target-20261030-codex`. The proposed policy XML parsed with three policy elements.

## Open gates

`Confirm` and both seqpacket `OP_DECIDE` routes remain denied. No trusted human display or affirmative action, fresh non-retained Polkit authorization, lock-bound journal decision, approved projection or target/security/human approval success is asserted. The private-bus fixture manually reconstructs connections; it does not test installed daemon recovery. Effective root-owned bus policy, distinct UIDs, native logind changes, installed unit supervision, package notices, rate control and sustained enrolled-sender abuse remain deployment gates. The separately reviewed operating witness `99aaed2d56d6f497d89c39c83de53523419fd951` is not merged here; later reconciliation needs a root-owned journal parent and cannot prove removal of records never extended into the witness.
