# A16 human-decision preparation: independent review

Date: 2026-10-29. Reviewed `/home/butahlecoq/work/archy/.worktrees/a16-human-decision-f1e2477-20261029` against `f1e2477047dbafbacfba3229daddab0ca58ebe02`. I read `AGENTS.md`, release specification §§4, 5 and 7, the A16 handoff/design/plan, the original broker trust/security reviews (`authority/REVIEW-SOL.md`, `REVIEW-SOL-ROUND2.md`, `REVIEW-SOL-ROUND3.md` in the original `a16-human-broker` worktree), authority source/tests/package fragments and the A14 decision contract. The original reviewed worktree was read only. This report is the only source-tree edit; no install, approval, commit, push, child agent or other-worktree write occurred.

## Scoped verdict

**One verified journal defect blocks using this primitive for future decisions.** The current daemon has no journal append or approval writer, and both seqpacket channels deny `OP_DECIDE`; this review found no present approval bypass. The slice remains suitable only as bounded, untrusted proposal ingress and read-only display. It does **not** meet the release §7 human-control acceptance gate, establish a deployed distinct-UID authority boundary, or authorize final IFC status. Exports remain `DRAFT`.

## Verified finding: accepted escaped payload makes the journal unusable

`authority/src/journal.rs` validates `payload.len() <= 16*1024` before serialization, but `scan()` rejects any encoded event above 64 KiB. JSON can encode a single NUL byte as six bytes. A standalone probe against the compiled `Journal` API appended 16,384 NUL characters, without altering source or creating a decision record:

```text
append=Ok(1); file_sizes=[98450]; read_verified=Err(... "invalid journal file"); reopen=Err(... "invalid journal file")
```

Probe source: `/tmp/a16_review_probe_sol.rs`, SHA-256 `ef7d5a56b0137118c2fb3f41b5e484ba1dcd4469fb702acdc180b372b9f70c1e`. It used a private mode-0700 disposable directory, called `append`, then called `read_verified` and `open` on the actual committed event. The result contradicts the journal's accepted-record invariant: a successful write can permanently block later reads, appends and daemon startup until manual repair. This is a **medium severity preparation defect**, not a current approval bypass, because `authorityd` opens/verifies the journal but never appends to it. Before attaching a decision writer, reject oversized *serialized* bytes before file creation and cover maximum-escape payloads with a direct round-trip test. A future bounded event type must also validate its own semantics and provenance before it reaches this opaque storage API.

## Boundary and durability assessment

- `append` holds a process mutex and Linux `flock` across pre-append directory sync, full scan, idempotency lookup, create-exclusive temporary file, file sync, `RENAME_NOREPLACE`, and directory sync. Separate openers in the focused suite produced twelve ordered records without gaps. Exact key/payload replay returns the existing sequence/hash; a changed payload conflicts. The post-rename sync failure test returns an uncertain error and requires a later successful directory sync before replay acknowledges the record. These tests exercise syscall-error control flow, not power loss or hardware flush behavior.
- Startup takes the directory lock, removes only correctly named private broker-owned pending files, syncs the directory, and verifies the chain. Unknown entries, malformed names/records, altered bytes, gaps, duplicate keys, non-regular files, wrong ownership/mode and final-component symlinks fail closed. Directory and event access use opened FDs and `openat` with `O_NOFOLLOW`; the production directory must be broker-owned mode 0700. Safe ancestors, effective state-directory ownership, one broker UID and no external same-UID writer are deployment assumptions. A concurrent process already holding broker UID can tamper despite the lock; this is not an adversarial lock.
- The hash chain detects an isolated changed record or missing middle entry. It cannot establish provenance or prevent rollback of a valid tail. I rewrote both records of a disposable two-record chain, recomputed their names and predecessor hashes, and `Journal::open` returned `['rewritten first', 'original second']`. Probe source `/tmp/a16_chain_probe_sol.rs` has SHA-256 `99474795fe4539db1df0a452201fe0c1c10d31d7fb20199a1a5ec72ff51e6bf3`. A broker/root actor able to rewrite the complete chain can likewise forge a self-consistent history; no independently protected monotonic anchor is present. This agrees with the design limitation and must remain explicit.
- The journal accepts opaque UTF-8, ASCII bounded idempotency keys and at most 1024 events. It does not parse a `DecisionRecord`, bind a proposal or verify an artifact, run, comparison, project revision, actor or timestamp. A14 evidence fields remain untrusted claims and existing project history can be agent-writable. The read-only proposal store protects bytes after submission, not the truth of their contents. No automatic decision can follow from a passing claimed check.

## Decision and host-authority boundary

`authorityd.rs` retains its `Journal` only for startup verification. The sole production `append` call site is absent; `Journal::append` is a public generic library method, not a socket method or authenticated approval API. `handle_connection` routes only agent submit and human read; `(_, OP_DECIDE)` returns `STATUS_DENIED`. A direct probe through two real filesystem `AF_UNIX/SOCK_SEQPACKET` listeners and production `accept_and_handle` returned `agent decision response=[1]` and `human decision response=[1]`, where 1 is `STATUS_DENIED`. Probe source `/tmp/a16_socket_probe_sol.rs` has SHA-256 `8df1fa1409fb5eafd37d4bfd793618492bb5d34101ffeeee4a1ada785ea21a8f`. The disposable fixture mapped both channels to local UID 1000 only to exercise transport; `Identities::distinct()` rejected that configuration. It is not a distinct-host-UID or installed-daemon authorization proof.

The package declares separate agent and broker users, a group-restricted human socket, two named activation descriptors and a root-owned human enrollment file. The daemon rejects missing/overlapping/root identities and wrong service UID. These declarations do not prove effective systemd ordering, socket ownership/mode, group membership, whole-agent-tree confinement or native service startup. `SO_PEERCRED` describes the connecting peer and a forwarded connected FD can delegate read access; `getsockname` does not prove the listener path still refers to that socket. Neither read-only limitation grants consent. The required separate system-bus unique-sender method, live local logind session check, exact displayed snapshot binding, fresh non-cached Polkit `auth_self` challenge, protected history/evidence recomputation and final commit recheck are unimplemented. No test here simulates a successful approval.

## Reproducible checks and integration limits

All commands ran in the specified worktree with new SSD target `$PWD/.cargo-target-a16-review-sol`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, and `RUSTUP_AUTO_INSTALL=0` where applicable:

| Check | Result |
| --- | --- |
| `sha256sum -c docs/plans/a16-human-decision-source.sha256` | all 20 listed files OK before probes |
| `cargo test -p archy-authority --locked --offline` | exit 0: 2 unit, 13 socket boundary, 7 journal boundary, 1 display tests |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | exit 0 |
| Direct escaped-payload, real-socket and full-chain rewrite probes | outputs above |

The frozen patch `/home/butahlecoq/work/archy/.tools/a16-human-decision-f1e2477.patch` matched its adjacent SHA-256 and the requested identity `143663584d8f9707bc027ba6c9826714f4d67bf7d51d067459593bf81db40d13`. The source manifest gives per-file identities; it was not updated by this review. This branch remains on `f1e2477`, before the published `8e36e11` compare/MCP/schema-checker/lock changes. Integration must merge its `Cargo.lock` addition with the authority member and retain 8e comparison/schema semantics; separately versioned A02/A04 history and CI changes require their own conflict checks. Passing this focused build says nothing about that combined tree.

The host is WSL2, not native Arch/Omarchy. No real three-UID systemd activation, installed socket/group, D-Bus/logind/Polkit human-positive path, power-cut recovery, or target filesystem gate was run. Existing package staging omits the authority binaries and units. Those native and packaging/notice gates, plus the encoded-size defect and protected history integration, remain prerequisites to any decision writer or approved-state projection.
