# A09 scratch capability receiver review, round 1

The intended result is a bounded, quota-checked directory capability for later artifact capture, backed by a private service tmpfs. **Verdict: ISSUES.** The receiver and WSL probes pass their existing checks, but the privacy check does not verify the documented `mode=0700` setting.

## P2. A world-writable tmpfs passes the privacy probe

`tests/security/systemd_scratch_fd_probe.py` requests `mode=0700`, but neither its pre-token checks nor `check_quota()` in `crates/archy-adapters/src/isolation.rs` inspect the received directory's permission bits. The Rust receiver checks directory type and owner, tmpfs magic, byte and inode capacities, and `nosuid,nodev,noexec`. An owned directory can also have mode `0777`.

I copied both probe scripts to a disposable temporary directory, changed the copied systemd probe's only `mode=0700` to `mode=0777`, and ran it with its normal UUID-scoped transient user unit. The exact adversarial command was:

```sh
/usr/bin/python3 - <<'PY'
import pathlib, tempfile, subprocess, shutil
source = pathlib.Path('tests/security')
with tempfile.TemporaryDirectory(prefix='archy-a09-mode-probe-') as d:
    root = pathlib.Path(d)
    shutil.copyfile(source/'scratch_fd_probe.py', root/'scratch_fd_probe.py')
    text = (source/'systemd_scratch_fd_probe.py').read_text()
    assert text.count('mode=0700') == 1
    (root/'systemd_scratch_fd_probe.py').write_text(text.replace('mode=0700', 'mode=0777'))
    result = subprocess.run(['/usr/bin/python3', str(root/'systemd_scratch_fd_probe.py')], capture_output=True, text=True, timeout=35)
    print('modified requested mode=0777')
    print('exit', result.returncode)
    print('stdout', result.stdout.strip())
    print('stderr', result.stderr.strip())
PY
```

Observed output began `modified requested mode=0777`, `exit 0`, and `stdout {"byte_capacity": 65536, ... "status": "pass", ...}` with empty stderr. The expected result for a probe that proves the documented private mode is refusal before the `GO` token. Add a check of the received FD's mode, and assert the mode in the service probe before token release. A private mount namespace limits who can see this directory, so this finding does **not** demonstrate host access or a production escape. It shows that the current check would not detect a weakened mount mode.

## Verified scope

The reviewed worktree is `/home/butahlecoq/work/archy/.worktrees/a09-supervisor` at base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`. Live SHA-256 values matched the frozen handoffs for `isolation.rs` (`b8abf0c6c3a4dc05598468e8bfa7ad255a30d528df4da8f72c471ddb8243d14c`), `tests/isolation.rs` (`e19af53386dbd11273c50fc9597a344e1b7a6e3693e3006b8808f91d8eff3594`), adapter `lib.rs` (`99586a26df00fc2af7846c9399a51c9944869eef621c740f26358f3076f7b614`), adapter `Cargo.toml` (`ba4552770e011dcf62b1d8b2cf2063ad5bc1ad22fa7ba167472e4fac34394929`), `Cargo.lock` (`13db31ddb6926b096eb0e16714ae0a8c83dac68bc6b9e472733f22934035dc40`), and both Python probes (`94be127b1039b3dadc1a1b2e888414bdc96a69641b252182b237fbcfac14d423`, `de05027f065b4fb7653fe90858d0688a65dbb0e0e33c894792fb4788a6621765`).

I ran `cargo test --locked -p archy-adapters --test isolation` with `RUSTUP_AUTO_INSTALL=0`, offline Cargo, two jobs, and `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a09-review`. It passed three integration tests. The focused `--lib isolation::tests::exact_control_frame_transfers_one_cloexec_directory_before_quota_check` test passed. `/usr/bin/python3 tests/security/scratch_fd_probe.py` passed with 65,536 bytes, eight inodes, and real byte and inode `ENOSPC`. `/usr/bin/python3 tests/security/systemd_scratch_fd_probe.py` passed with the documented mount and cgroup settings, a retained FD read after service exit, and an absent cgroup after collection. A final check found no matching temporary paths or `archy-scratch-fd-*.service` units.

Source inspection found no accepted extra descriptor, truncated ancillary message, non-seqpacket channel, non-directory FD, or non-tmpfs fallback. `RecvAncillaryBuffer` owns received FDs until validation, so rejection drops them. The fixed frame is not peer or job authentication, as the handoff states. A positive end-to-end call to the **Rust** `receive_quota_checked_scratch()` from the transient service, service identity binding, writer teardown, production containment, and Omarchy target acceptance remain unverified. I made no product-source change.

Pstack's **Boundary Discipline** kept the finding at the descriptor policy and pre-token check. **Prove It Works** led to the disposable `mode=0777` run instead of treating the hardcoded mount option as proof.
