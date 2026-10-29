# Independent A22 rollback/security re-review

**Verdict: PASS for the frozen candidate and the exercised ext4 rollback paths.** The prior deterministic post-publication EIO no longer strands `install.json`; an ordinary retry succeeds. No new reproducible failure was found in the paths below. This is not Omarchy installation, graphical viewer acceptance, or current-head integration approval.

## Frozen scope

- Candidate worktree: `/home/butahlecoq/work/archy/.worktrees/a22-native-15446-20261030`, HEAD `15446eeaf316846936a093377041b8c1d2874092`.
- `sha256sum -c /home/butahlecoq/work/archy/.tools/a22-native-15446-20261030/rollback-fix-freeze.sha256` passed for the full patch, installer, installer tests, and rollback handoff. `git diff --binary HEAD | sha256sum` returned `46ef8ae6a18cf97d46d1a00323f5bc584674d11d8954832d896eae09669b43f6`, equal to the frozen full patch. `git diff --check HEAD` passed.
- Read the rollback handoff, `rereview-sol.md`, `independent-review-sol.md`, installer implementation/tests and viewer handoff implementation. No candidate source or test file was edited. The new independent probe is outside the candidate: `rollback-rereview-probe.py` beside this report.
- `findmnt -T .../rereview-ext4 -no FSTYPE` returned `ext4`. All mutable home, project and runtime targets were unique, disposable directories under that path. The candidate checkout was used only as source and build target.

## Reproduced transaction behavior

Run from the directory containing this report: `python3 rereview-ext4/repro.py` and `python3 rollback-rereview-probe.py`. Both exited 0. The original prior-review EIO probe now printed `post_state_rollback ... dest_absent= True state_present= False`; its normal retry was no longer refused. The independent probe additionally asserted that the retry installs and uninstalls.

The independent probe passed these cases using the real installer and real ext4 operations, with injected errors only at the named boundary:

| Boundary | Observed result |
| --- | --- |
| State hard-link publication returns EIO | Final plugin and state absent; retry installs. |
| Final plugin lookup returns EIO after state publication | Final plugin and state absent; retry installs and uninstalls. |
| Rollback `install.json` unlink returns EIO | Owned directory restored; state retained; next install is `already-installed`. |
| Destination occupied while restoring after state unlink EIO | Occupant intact, state retained, owned tree retained under one private `.archy-parked-*` name. Manual recovery is required. |
| Ownership state content changes before rollback | Changed state is retained and owned directory restored; no deletion of the changed record. |
| One parked-tree file unlink returns EIO after state removal | Final plugin and state absent, one private partial parked tree remains; retry installs. Manual cleanup of the parked tree may be needed. |

The original probe also passed: an occupied install destination returned `EEXIST` without modifying the unrelated victim; an uninstall inode swap preserved the state and restored the unrelated tree; an occupied uninstall restore preserved its backup and state; a symlinked `.config` ancestor was refused without writing through it. The existing post-publication symlink swap test returned failure and preserved the displaced owned directory and state for recovery. Ordinary preview, install, idempotent install, uninstall, neighboring plugin preservation, local edit refusal, and source symlink refusal passed in the installer suite. A separate injected uninstall state-unlink EIO restored the owned directory and state, left no `removed-*` backup, and permitted a later normal uninstall.

## Test and viewer evidence

- `TMPDIR=<ext4 rereview-ext4 path> python3 -m unittest tests/omarchy/integration/test_install.py tests/omarchy/integration/test_plugin_contract.py -v`: **19 passed**.
- `node --test tests/omarchy/integration/open_model.test.js`: **3 passed**.
- `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo test --locked -p archy-cli immediate_viewer_failure_and_bounded_success --lib`: **1 passed**, including immediate nonzero child exit and bounded handoff for a live child (the test checks under 800 ms against a one-second child).
- `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo build --locked -p archy-cli`: passed. With the rebuilt CLI, a freshly generated accepted report in a disposable ext4 project and a mode-0700 runtime, `desktop open --kind report --artifact report.md --json` returned exit **1**, `BACKEND_UNAVAILABLE`, and `viewer exited exit status: 3` with the real headless `xdg-open: no method available` diagnostic. No successful graphical display was tested.

## Limits of this PASS

The plugin directory and ownership state cannot be updated atomically together. The occupied restore and parked-tree deletion probes demonstrate the documented private leftovers and manual recovery cases. Directory handles, inode checks and exclusive renames prevent the tested overwrites, but arbitrary concurrent replacement after the last identity check remains outside this result. The viewer's 250 ms observation can detect an immediate failure; a process still alive past that window yields only a handoff receipt. No installed Omarchy shell, Quickshell UI, real graphical viewer success, or merged current-head tree was exercised.
