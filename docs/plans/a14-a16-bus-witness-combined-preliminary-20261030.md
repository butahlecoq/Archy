# A14/A16 human bus and root witness: preliminary combined candidate

30 October 2026. This is an integration candidate for a separate combined review, based on published `c72f5309a681f36f56cb05e3932486bb7410a3f0`. It combines reviewed human bus `7af8be00ae5872053cb60bf98c7191a089cc5014`, reviewed witness parent and lock binding `013564f916d804c31b05d399fd25037e7c0fe42e`, and the scoped independently reviewed oversized-frame follow-on `50883f6781bbf044f44ddbd1c62de030c0b0c37e`. The source patch for that follow-on has SHA-256 `04231d57ca0cea076f75cde26dd388261d3bcb76282ea76628d0f32a598b9c23`. Its review is `.tools/github-updates/a14-a16-bus-availability-review-sol.md` outside this worktree.

## Reconciliation

The witness branch merged with one text conflict: `authority/Cargo.toml`. The merged manifest keeps both binaries and one workspace `rustix` declaration with the `process` feature; the workspace already supplies `fs`. `Cargo.lock` merged without a text conflict and `cargo --locked --offline` checks it. `authority/src/journal.rs` received the witness's locked scan/extension method without changing the bus path. `authority/src/project_path.rs` now owns the byte-identical source path walker, source digest, descriptor identity, and root-through-source identity encoding formerly duplicated in `protected_project.rs` and `journal_witness/project.rs`. The witness retains its additional `.archy`, staging, revisions, and lock identity checks. The bus and witness still each recheck their own pinned object at their trust boundary.

The availability follow-on cherry-picked without conflicts. Its accepted D-Bus frame bound remains 64 KiB. The reviewed discard path handles complete oversized frames within the protocol ceiling and leaves the served connection available for later valid calls in its private-bus tests. It does not establish sustained flood fairness or native availability.

## Authority and limits

`Review` returns bounded observations of broker-owned current v2 or authentic historic v1 evidence after peer, session, path, and worker checks. `Confirm` and both seqpacket decision routes deny. The root witness is a separate surviving-record observation with `EXACT`, `STALE`, and `EXTENDED`; it does not grant an agency or human approval. The witness requires a root-owned, broker-nonwritable journal parent, root-owned state, and a pinned project source and lock structure. A broker can delete an unextended tail before observation, ignore advisory locks, or mutate after a scan. The held project lock is acquired by pathname and is not compared directly with the persisted lock inode; the witness rechecks around it. Physical power loss and privileged kernel/storage attacks are untested.

The bus unit and policy remain source only. Effective native system-bus policy, installed supervision, real logind races, sender flood fairness, packaging notices, and Omarchy acceptance remain gates. There is no trusted exact-byte affirmative display or approval route.

## Validation and freeze

See the exact commit and test results in the final freeze section below. No push, other worktree edit, host service operation, or child-agent work was performed.

The integration merge commit is `8f67519`; the reviewed availability follow-on cherry-pick is `1987e1f`. All six paths changed by that follow-on have the exact Git blob IDs from `50883f6781bbf044f44ddbd1c62de030c0b0c37e`. The new 12-path combined source manifest is `docs/plans/a14-a16-bus-witness-combined-source.sha256`; all entries pass `sha256sum -c`. The old bus and witness manifests remain historical snapshots of their pre-integration commits and are not combined-tree manifests.

Rust 1.99.1, `--locked --offline`, `CARGO_BUILD_JOBS=2`, `TMPDIR=/home/butahlecoq/work/archy`, and dedicated ext4 target `/home/butahlecoq/work/archy/.tools/targets/a14-a16-bus-witness-preliminary-20261030-codex` were used for Cargo checks.

- `cargo test -p archy-authority`: exit 0 after the availability cherry-pick. Six library, one human daemon, one review binary, thirteen socket boundary, one private-bus recovery, one real private-bus human boundary, eight journal boundary, ten protected project, and seven zbus receive-bound tests passed. The private bus exercises oversize discard, subsequent valid Review, and Confirm denial; logind is mocked.
- `cargo test -p archy-cli --test golden_artifacts --test accepted_worker --test desktop_readers --test core_workflow`: exit 0. Four golden, five worker, eight desktop, and twenty-three workflow tests passed. Golden and workflow cover authentic historic v1 and current v2 evidence, including migration and replay.
- The supplied `authority/tests/witness_separate_uid.py` passed twice in disposable `--network none` Arch Docker with distinct broker/agent UIDs and a dedicated ext4 bind-mounted target. It exercised root-owned journal parent rejection, source/ancestor and lock identity replacement, symlinks, self-bind mount denial, journal rollback/replacement, restart, unauthorized peer, and corrupt/missing state. Its own final line was `PASS: separate UIDs, root parent, ancestor/lock/symlink, bind=tested, unextended tail, rollback/replacement, restart, project inode/source, corrupt/missing state, unauthorized peer/write`.
- `cargo clippy -p archy-authority --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check` from c72, the 12-path combined manifest and four-path availability manifest passed.

These tests establish a preliminary combined candidate, not a native service, sustained flood fairness, human decision, or power-loss result. A fresh independent security and compatibility review must evaluate the frozen combined commit and its shared path extraction.

After the PR branch advanced, reviewed A17 commit `15108bad8a187ad838eca5b0c524836a2ebd4175` was merged without conflicts as `66cdb204ca6beaf402c624399c7879e53b0f7350`. Its three paths (`crates/archy-cli/tests/edit_interruption.rs`, `docs/plans/a17-edit-interruption-c72.md`, and `docs/recovery.md`) are preserved byte-for-byte. This adds a staged-edit interruption test and recovery documentation; it changes no production source or the reviewed A14/A16 bases. `cargo test --locked --offline -p archy-cli --test edit_interruption` passed both tests on the combined tree with the same ext4 target and Cargo environment.
