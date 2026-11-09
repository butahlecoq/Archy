# A17 edit staging interruption slice

Baseline: published `c72f5309a681f36f56cb05e3932486bb7410a3f0`.

## Plan and rationale

The core staging SIGKILL test covers a generic `generate` transaction. The CLI edit/restore test covers completed operations. This slice joins those boundaries: accept a real CLI generation, begin the exact edit request through the published `ProjectStore` API in a separate writer, stage edit bytes, terminate that writer at a test-owned barrier before commit, then retry through the real CLI with the same key and revision. A second edit back to the original parameters verifies restore remains possible after recovery.

The parent compares every accepted run file and revision hash before interruption, after interruption, and after retry. It checks the staged UUID is not accepted, the prior run is still trusted, the edit publishes once, same-key CLI replay returns its receipt, and the restored variant matches the original ID. No production hook or dependency is needed. A scoped child guard kills and reaps the writer if an assertion unwinds; a private parent channel bounds the writer if its test parent dies.

Risk: the test stages through core APIs rather than interrupting inside CLI `publish`, so it proves cross-layer request identity and recovery at the staging boundary, not CLI cancellation at an arbitrary instruction. This is a process-kill test on local ext4, not power-loss or Omarchy acceptance.

## Observed fault matrix

The focused test ran with `TMPDIR=/home/butahlecoq/work/archy/.t`, `CARGO_BUILD_JOBS=2`, and `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-edit-c72`. `df -T` identified `/home/butahlecoq/work/archy` as ext4 on `/dev/sdd`. The process writer uses the same filesystem for the project fixture and staging directory. The published baseline had no source changes in the worktree when this test was written.

| Point | Fault/observation | Accepted state and retry evidence |
|---|---|---|
| Before edit | CLI sample accepted revision 1 | `verify_run` read original variant; 11 accepted files inventoried |
| After `PendingRun::write("params.json", ...)`, before commit | Child published an atomic test readiness marker, then parent observed staged bytes and killed/reaped the child with signal 9 | `read_history` stayed at revision 1; staged run `1a9ce739-5f2b-4fc7-98f4-d80c854ae275` was rejected as unaccepted; `desktop status --json` listed only the original run; original `verify_run` still succeeded; all 11 hashes matched |
| CLI retry with same key, revision, edit file and source run | Normal `archy generate --from-run ... --variant ... --params edit.json --idempotency-key interrupted-edit --expected-project-revision 1 --json` | Revision 2 accepted with original run/variant provenance; repeated identical CLI call returned equal receipt; desktop status listed exactly two accepted runs |
| Restore from edited run | Normal CLI edit with original parameters, new key and expected revision 2 | Revision 3 accepted; variant ID equaled the original; original and edited inventories stayed unchanged; history contained exactly three receipts; interrupted staging remained for diagnosis |

One captured invocation produced this complete accepted revision 1 inventory. The test compares the whole map within each invocation before interruption, after interruption, after edit retry, and after restore. Run IDs and envelope hashes vary between invocations.

| Accepted file (under `runs/c337fc8f-cf54-4399-874c-5592a65f6f8c/` unless shown) | SHA-256 before and after SIGKILL |
|---|---|
| `.archy/revisions/00000000000000000001.json` | `4b72079ee1260545fd34f8af9cb1f348c456556abb43c3d3a88fe4463bc41677` |
| `evaluations/variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b.json` | `982c7549f080a4c7f7d0cfea14bead54dae2e34f5c70056316b240ba301414eb` |
| `events.jsonl` | `c62e1c6631feb70bfccb8a584b1e390c2cc716d953c1e3254695574615b5a617` |
| `generation.json` | `eaadf5930928d77fccdae7b2851a898f459c67a234189167767f0a1b420aa474` |
| `input.json` | `c2cf36b61fec408ab41a33936f47519a2aad9168e73dc42ce44cc7a2764b79b7` |
| `manifest.json` | `7bedda0a1359ea7e7e4ca161ad0e70e3101862fd5b94f4343a21e611049e806c` |
| `normalized.json` | `a8408595759f456105022d97271d3a41d47923499500b7dc43d302151b6ecac2` |
| `profile.json` | `47edd09dd0d2cc0b0d6bc00511e1caea6e3c468c56a441f81d3fe1b94b72c297` |
| `report.json` | `7060cf852c6c3a18730b2d945baa23cb9195f36f0a0ee0e3d1e3396618cd79c1` |
| `report.md` | `834f4c804f3521340d3e5676eadfc9a0153f173b62e634923784e25c80f424b7` |
| `variants/variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b.json` | `916627457ccab67c9aad01832af638933334c2fbff6dc5d915300c655075b8c9` |

## Limits

The kill occurs at the test-owned barrier after a core transaction stages edit bytes. The actual CLI retry and restore use the normal binary, but the interrupted process does not execute CLI artifact publication. This does not prove interruption inside revision rename or sync, sudden power loss, persistence after a kernel crash, or durability on network/other filesystems. It does not exercise Blender, IFC, MCP, backend cancellation, GPU/visual output, or Omarchy runtime acceptance. No backend success is inferred from these core and CLI results. No production source or sync ordering changed. If the test runner itself is killed, its private `archy-edit-interruption-*` fixture can remain; the writer exits when its parent socket closes, and the operator can inspect then remove that owned fixture. There is no cleanup service.

## Validation and handoff

Commands from this worktree used `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a17-edit-c72 TMPDIR=/home/butahlecoq/work/archy/.t RUSTUP_AUTO_INSTALL=0` for Cargo invocations:

```sh
cargo test --locked --offline -p archy-core --test recovery_staging -p archy-cli --test core_workflow
cargo test --locked --offline -p archy-cli --test edit_interruption -- --nocapture
cargo test --locked --offline -p archy-cli --test edit_interruption
cargo clippy --locked --offline -p archy-cli --test edit_interruption -- -D warnings
rustfmt --check --edition 2024 crates/archy-cli/tests/edit_interruption.rs
cargo fmt --all -- --check
git diff --check
```

All exited 0. The baseline command reported 23 `core_workflow` and 11 `recovery_staging` passing; the new integration command reported 2 passing (including its standalone no-op helper). The final focused test and Clippy passed after the last assertion change. The test invokes the actual `archy` binary for generation, desktop status, edit retry, replay, and restore. No backend worker was run. Independent review and the subsequent correction are recorded below.

## Request identity review correction

Independent review of `b7eab3000ecd84664b7d990046e1551a7bac989d` found that matching CLI arguments and a hand-built request did not establish that the interrupted request and the accepted retry had the same fingerprint. The earlier captured inventory and fault observations remain evidence for their stated run; that run did not compare fingerprints.

The staging writer now computes the v1 fingerprint from the deserialized `RunRequest` it passes to `begin` and includes it in the atomic readiness marker. After the real CLI retry, the parent reads revision 2, checks its receipt identifies the CLI result, and compares its persisted `request_sha256` with the writer's fingerprint. This test computes the fingerprint using the storage format's domain prefix and serialized full request, without adding a production hook. In the focused passing run, both values were `4cda830274d85cd910247c4e7e8f22df0744602887a926941995724048277d17`. The run also passed the existing SIGKILL, status, replay, restore, history and inventory assertions.

For a negative probe, temporarily append one ASCII space to `request.request_bytes` in the writer immediately after deserialization, before computing its fingerprint and calling `begin`. With that local mutation, the focused test reached SIGKILL and CLI revision 2, then exited 101 at the new comparison: staged fingerprint `911014017d36654a1cbaae9c39e6452e313d2f78c3d7d4488398dfbe973ef081`, accepted fingerprint `dca51401b01c36df2dca2f456c3da21cf73de7de2a218e9e1b11bf96dd2a418f`. The mutation was removed. UUID-dependent request hashes vary between runs. This establishes detection of an actual difference in the staged request bytes, while leaving the interruption boundary and backend and power-loss limits above unchanged.

With the same Cargo environment as above, `cargo test --locked --offline -p archy-cli --test edit_interruption -- --nocapture` passed 2 tests after removing the mutation. `cargo clippy --locked --offline -p archy-cli --test edit_interruption -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` also passed. The final test file SHA-256 was `d74b674d67711ed9837c78d848bae90dc770c82600c45824971050f512355888`; `Cargo.lock` was `fa404005e3938b38e399d949a7fff6dae842b24342f3d05ebc8f1e781e3cf92f`. No production source changed.
