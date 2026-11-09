# Package notice integration checkpoint

The A21 notice source was copied from the independently reviewed worktree based on `d4b6ca8` onto integration head `634a114`. All 20 source and notice hashes in `package-notices-review-round2.md` matched before staging. The copied files matched the reviewed bytes. The locked CLI graph remained unchanged at this integration point.

On the integration copy, `python3 -B -m unittest discover -s tests/packaging -q` passed 23 tests. Offline `notices.py` admission using the existing vendor directory exited 0. An initial coordinator attempt passed a nonexistent child as `--output-dir` and failed before writing. The corrected attempt used an existing empty temporary directory and succeeded.

The staged patch passes `git diff --cached --check -- . ':(exclude)packaging/arch/notices-sources/**'`. The unscoped check reports whitespace in the verbatim Rust `COPYRIGHT-library.html` notice. Its original bytes and reviewed SHA-256 are preserved for provenance and notice accuracy.

This validates notice admission and package staging source. It does not establish a new Arch package, complete linked-code or legal distribution audit, pacman transaction, or actual Omarchy acceptance.
