# A14/A16 local journal witness on published c72f5309

30 October 2026. Base `c72f5309a681f36f56cb05e3932486bb7410a3f0`; reviewed candidate `99aaed2d56d6f497d89c39c83de53523419fd951` and independent review `.tools/github-updates/a14-a16-operating-witness-review-sol.md`. This local integration is an evidence-only root-owned journal witness. It neither writes human decisions nor changes the denied Confirm path.

## Port and dependencies

The candidate binary and separate-UID probe were brought forward as new files. `Journal::with_locked_hashes` was added to the published journal without replacing its current append, replay, or scan code. The candidate's `ProtectedProject::open` path/source checks were extracted into the binary-local `authority/src/bin/journal_witness/project.rs`: openat2 beneath pinned ancestors, root/broker ownership and mode, mount/device/inode identity, single-linked project file, and a bounded source digest. The full candidate `ProtectedProject` also depends on unfinished human review and CLI modules; those were not brought into this witness branch. The current `ProjectStore` supplies the project lock and remains untouched. The direct `rustix` dependency adds one entry to the authority package in `Cargo.lock`.

No core, CLI, MCP, desktop, schema, or A23 source file is changed. Canonical v2 and historic v1 code remains as published. The probe now includes broker-owned parent rejection, deletion of an unextended tail, and same-byte project inode replacement. It uses a dedicated ext4 bind mount when invoked with `ARCHY_WITNESS_LAB=/target/lab`.

## Trust boundary and limits

Trust root UID, the installed witness binary, root-owned 0700 witness state and non-writable path ancestors, the kernel, and witness storage. Treat broker UID as hostile for journal and project bytes. The socket checks `SO_PEERCRED` UID; a broker GID alone is insufficient. The witness extends only verified on-disk record hashes and rechecks the journal under its lock. A broker can ignore advisory locks and mutate records after a scan; a later request detects a divergence. It cannot attest an unextended record that was deleted before observation. `EXACT` is evidence about the surviving journal and bound project, not human approval.

A broker-owned journal parent is ineligible; the existing production path requires root-owned provisioning before use. There is no human decision writer or approval integration, and Confirm remains denied. Target power-loss behavior and Omarchy service provisioning are untested.

## Combined review conflicts

Likely shared edits with the parallel human bus and Parfit work are `authority/Cargo.toml` and `Cargo.lock`; `authority/src/journal.rs` may also overlap if their decision writer changes journal locking. The binary-local `project.rs` duplicates the candidate path/source checks in their `protected_project.rs`; review both for drift and unify only after preserving the witness's pinned checks. The witness binary and UID probe are new isolated paths. No parallel agent worktree was modified.

## Validation

All Cargo commands used `--locked --offline`, Rust 1.99.1, `CARGO_BUILD_JOBS=2`, `TMPDIR=/home/butahlecoq/work/archy`, and a dedicated ext4 `CARGO_TARGET_DIR` under `.tools/targets/a14-a16-witness-c72-20261030`.

* Authority library, boundary, and journal boundary: 2 + 13 + 8 tests passed. The boundary suite includes denial of decision operations.
* Core storage and validation: 21 + 22 tests passed, including historic v1 receipt and canonical v2 cases.
* CLI desktop readers: 8 tests passed.
* MCP bounds: 6 tests passed.
* Witness binary build: exit 0.
* Docker `--network none` separate-UID probe, with a dedicated ext4 bind-mounted lab and read-only binary, fixture, and script: exit 0. It covered root parent, exact/stale/extend, deletion of an unextended tail, rollback and replacement, restart, project inode/source binding, corrupt/missing state, and unauthorized peer/write.
* A23 playbook, harness, and signed-review `unittest`: 4 + 7 + 16 tests passed.
* Witness binary clippy with `-D warnings`, Rust format check, whitespace check, and Python syntax check: exit 0.

The Docker UID probe is a process and permission test on ext4, not a physical power-loss test.
