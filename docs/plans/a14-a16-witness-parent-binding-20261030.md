# A14/A16 witness parent binding handoff — 30 October 2026

Base: exact witness port `4f864b02cb5c3d2fb98f0482fbc2af5d0ac9e1b9`. This change is confined to the new `a14-a16-witness-parent-binding-20261030` worktree.

## Plan and binding

1. Reproduce the same-source-inode/new-project-parent attack under separate Docker UIDs.
2. Persist a versioned path identity digest and a separate lock-structure identity digest in root-owned `binding.json`; recheck both at each witness transition.
3. Refuse v1 bindings without migration; test the unchanged path, restart, source, lock, symlink, and bind cases.

The project path digest hashes the root-through-source descriptor chain in the same order and little-endian `(dev, ino, statx mount ID)` encoding as the independent `authority/src/protected_project.rs` in `a14-a16-bus-a9-integration-20261030-codex`. The lock digest hashes `.archy`, `staging`, `revisions`, and `lock`, opened relative to the pinned project directory with `openat2` beneath/no-symlink checks and owner/mode/link checks. The v2 binding bytes seed the v2 witness domain. Missing or unknown version fields fail on load; an old state is never silently rebound.

The root-owned state and root-owned non-writable journal parent are trusted anchors. A broker-owned history remains broker-mutable. The witness observes and extends records that survive its scan; it cannot attest that a deleted, unextended record existed or that the broker obeyed its advisory lock after a scan. `EXACT` and `EXTENDED` are journal observations, not human approval or an attestation of unobserved records.

## Validation for independent review

The regression Docker probe failed before the source fix with `AssertionError: EXACT 0` after moving the same source inode under a new broker-owned parent and lock. After the fix, the probe passed with separate root/broker/agent UIDs, including refusal of the replacement and old unbound binding, successful restoration and restart, source mutation/replacement, lock replacement, symlinked `.archy`, and a self bind mount with a changed mount ID. The probe also confirms an unextended deleted tail can return `EXACT` and an extended deleted record is denied. The bind run used disposable Docker with `--cap-add SYS_ADMIN --security-opt seccomp=unconfined --network none` and a private ext4 target lab. One earlier bind run failed the existing wrong-UID socket assertion without diagnostic output; a fresh subsequent run passed. This is not a physical power-loss or target provisioning test.

`cargo +1.99.1 test --locked --offline -p archy-authority` passed (2 library, 1 review, 13 boundary, 8 journal boundary tests; other binary/doc suites empty). `cargo +1.99.1 fmt --check --all` and `git diff --check` passed. Cargo used offline mode, `CARGO_BUILD_JOBS=2`, `TMPDIR=/home/butahlecoq/work/archy`, and a unique ext4 target directory. No service, push, child agent, or approval action was used.

Minimal follow-on: an independent reviewer should inspect the exact frozen commit and rerun the Docker probe on a fresh ext4 lab, especially the lock acquisition race and the unchanged unextended-record boundary. This handoff makes no approval or deployment claim.
