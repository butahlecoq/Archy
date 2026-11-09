# A16 journal encoded-size fix: independent round 2 review

Date: 2026-10-29. Worktree: `/home/butahlecoq/work/archy/.worktrees/a16-human-decision-f1e2477-20261029` at `f1e2477047dbafbacfba3229daddab0ca58ebe02`. Read `AGENTS.md`, the first independent review, the fix handoff and hash manifest, the journal source and tests, and the daemon/transport denial path. This report is the only source-tree edit. No source change, commit, push, install, child agent or other-worktree operation was made.

## Verdict and findings

**The serialized-size defect is fixed for this scoped journal primitive.** `append` serializes the candidate record and rejects `bytes.len() > 65,536` with `InvalidInput` before hashing or creating a pending file. This matches `scan`'s maximum file length, while the 16 KiB raw-payload limit remains. I found no new blocking defect in the fix. The journal is still a preparation primitive, not an approval mechanism; this verdict does not satisfy the A16 human-decision acceptance gate.

An independent probe using the public `Journal` API established the boundary with 10,000 NUL bytes and ASCII filler under the raw-payload limit. The NULs expand to six JSON bytes each. With key `edge`, 5,399 filler bytes produced an exactly 65,536-byte event: append returned sequence 1, disk size was 65,536, and `read_verified` after a fresh `Journal::open` returned the identical payload. One further ASCII byte made 65,537 encoded bytes: append returned `InvalidInput`, with zero files in a fresh journal and an empty verified read. Repeating the rejection after a committed event left one file; the next successful append received sequence 2. Exact replay returned the original sequence and hash. A separate maximum-escape 16,384-NUL payload was rejected with zero files and a clean reopen. Probe source `/tmp/a16_round2_probe_sol.rs` has SHA-256 `aa5c0b0fd380bf87ffec20ab983df2fd33f3b54d66be8108e3f8ccd8078d0c98`; it used disposable mode-0700 directories and non-decision records.

The same probe placed a correctly named, mode-0600 partial file in the journal directory. `open` removed it, then accepted a sequence-1 record and reopened cleanly. A malformed pending name caused `open` to fail; removing that file restored the valid record. The focused unit test also injected a post-rename directory-sync failure and verified that exact replay acknowledges the event only after a successful sync. This establishes the exercised recovery control flow, not physical power-loss durability.

Other focused tests passed for exact-key replay/conflict, tampered bytes and chain rejection, symlink refusal, owner/mode checks, and twelve concurrent openers with no gaps. Source inspection confirms a mutex and directory `flock` around scan/append, create-exclusive mode-0600 pending files, `RENAME_NOREPLACE`, file and directory syncs, a 1,024-event cap, and fail-closed scan of names, ownership, mode, sequence, hashes, predecessor links, duplicate keys and payload bounds. A self-consistent history rewritten by a broker/root actor or rollback of a valid tail is outside this chain's protection; there is no independently protected monotonic anchor.

The actual production packet handler, `accept_and_handle` → `handle_connection`, was exercised by the real seqpacket socket boundary test. Both agent and human `OP_DECIDE` requests returned `STATUS_DENIED`; the test also checked the peer-UID policy and read-only human proposal access. Source inspection confirms `authorityd` opens the journal for startup verification only and has no production `append` call. The socket fixture uses one local UID for protocol coverage, which `Identities::distinct()` rejects for production. This was not a native installed-daemon session or successful authorization test.

## Frozen identity and checks

`sha256sum -c docs/plans/a16-journal-encoded-size-fix.sha256` passed for all three frozen artifacts:

| Artifact | SHA-256 |
| --- | --- |
| `authority/src/journal.rs` | `bd82ddb9e3de443fcd21e9cf8037c60f9638a540291cc6117157e772e4e6b655` |
| `authority/tests/journal_boundary.rs` | `c8f8121c1a81bbbca9b8b5ffb4f21971f86cf6c803a354911e4536798e60060a` |
| `docs/plans/a16-journal-encoded-size-fix.patch` | `d609fa3c88ff5b9cd629f05f30c04aaf3a1bf0ba4d26011170bcd4897b03025f` |

`git apply --reverse --check docs/plans/a16-journal-encoded-size-fix.patch` passed. The 18 unchanged entries of `a16-human-decision-source.sha256` also passed; its two journal hashes intentionally describe the pre-fix state.

Checks used `CARGO_TARGET_DIR=$PWD/.cargo-target-a16-round2-sol` on the workspace SSD, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true` and `RUSTUP_AUTO_INSTALL=0` for Cargo:

| Check | Result |
| --- | --- |
| `cargo test -p archy-authority --locked --offline` | Exit 0: 2 library, 13 socket boundary, 8 journal boundary, 1 display test; doc tests 0 |
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy -p archy-authority --all-targets --locked --offline -- -D warnings` | Exit 0, no warnings |
| Independent compiled journal probe | Exit 0; exact-size, rejection, readback, reopen, sequence, and pending recovery assertions passed |

## Combined A16 scope and limits

The complete A16 branch provides bounded proposal submission and read-only review plus this broker-private opaque journal. It does **not** provide a decision-record writer, bind a journal record to a trusted displayed snapshot or protected run history, or make an approval. Neither socket can commit `OP_DECIDE`. There is no native distinct-UID/systemd activation proof, live UID/session/logind check, fresh Polkit challenge, or final protected-evidence recheck. This WSL2 review did not run native Arch/Omarchy deployment, installed packaging, a human-positive path, or a power-cut test. Exports must remain `DRAFT`; no approved IFC status is authorized. Integration with the later `ff7f6bc` head and overlapping `Cargo.lock` remains a separate gate, with the exact boundary regression to be rerun on the integrated tree.
