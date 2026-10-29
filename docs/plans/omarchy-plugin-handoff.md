# A22 S1 plugin handoff

## Scope and short plan

Implement the standalone `archy.status` manifest, a summoned floating doctor panel, bounded fixed-argv process transport, and a reusable JS schema/probe model. Verify the model with Node and the manifest/API contract with Python. Keep S2 installation and H1–H3 action contracts separate.

## Changed paths

`integrations/omarchy/plugin/{manifest.json,Main.qml,DoctorProcess.qml,DoctorModel.js}`; `tests/omarchy/integration/{test_plugin_contract.py,doctor_model.test.js,fixtures/upstream-v4.0.6.json}`; `docs/omarchy/plugin.md`; this handoff. No Rust, Cargo, CLI, backend, runtime installation, or user configuration changes.

## Risk and dependency rationale

Only stock QML, Omarchy shell UI imports, and Node/Python standard libraries are used. No npm dependency is added. The report validator rejects unknown schema/status, missing/extra shape fields, excessive array/string sizes, and malformed JSON. Each stream is capped at 64 KiB before accumulation; the process has a 20-second deadline and TERM/KILL sequence. A generation and single-slot session reject stale completions and duplicate refresh. On failure, any retained report is visibly stale. Project actions remain unavailable; no doctor status grants readiness, approval, or launch authority.

Gauss's P1 review found that close then reopen before child exit lost the open-triggered refresh. The session now queues one reopen request while the cancelled child owns the slot, launches it after reaping, and ignores late callbacks from the cancelled generation without reporting overflow. Failed-to-start is handled through `runningChanged` because the pinned source does not emit `exited` for that case.

At the lifecycle rereview stage, QML passed literal `false` for stream completion. That historical blocker was addressed by Hume's reviewed trusted-producer rule and the native matrix recorded below. Installed Omarchy lifecycle and target acceptance remain untested; this slice does not close OM-A22-01.

## Upstream source evidence

Omarchy v4.0.6 tree `237f1b4071a0bff2296c6d740ba2b9fd695ebfa0`; pinned gallery panel SHA-256 `ee7e0477865b8ca92d86847d9558d139379d1882d8ffc508a1051a92a07dbd1b`; registry SHA-256 `8466127c53037b80b582544610b0bdf78a86764e4f037a3b9ce360d107359803`. See fixture and [upstream manual](https://github.com/omacom/omarchy/blob/v4.0.6/manual/32-shell-plugins.md), [gallery panel](https://github.com/omacom/omarchy/blob/v4.0.6/shell/plugins/dev-gallery/GalleryPanel.qml), and [registry](https://github.com/omacom/omarchy/blob/v4.0.6/shell/services/PluginRegistry.qml).

Quickshell v0.2.1 tag `a1a150fab00a93ea983aaca5df55304bc837f51b`: [Process source](https://github.com/quickshell-mirror/quickshell/blob/v0.2.1/src/io/process.cpp) SHA-256 `61af633eb2b0b58872a2d7c57f06d5781332e971d243e01f93b6871b9587c98a` calls `streamEnded` before `exited`, but does not explicitly drain QProcess read buffers in `onFinished`; [SplitParser source](https://github.com/quickshell-mirror/quickshell/blob/v0.2.1/src/io/datastream.cpp) SHA-256 `73edb2a6bc1c9472696adbf87d8041753b53550835fb6655f888f057f3aff7cb` converts each arbitrary byte chunk to a string with an empty marker and has no stream-finished QML signal. [Qt QProcess documentation](https://doc.qt.io/qt-6/qprocess.html) says read buffers remain available after `finished`. Thus `exitStatus` cannot stand in for observed stream completion, and split UTF-8 can yield replacement characters; the model rejects them. This source evidence is an inference about risk, not an installed-runtime result.

## Initial S1 validation evidence (before container harness)

- `node --test tests/omarchy/integration/doctor_model.test.js`: 7 passed, 0 failed, exit 0. Real subprocess fixture includes close/reopen before exit, a queued replacement child, and late cancelled-generation callbacks. Direct model checks cover exit 0/1/2, malformed/oversized output, UTF-8 replacement rejection, stale completion, teardown, and duplicate refresh.
- `python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py'`: 4 passed, exit 0. Manifest bad IDs/kinds/paths/missing files/symlinks and static QML contract checked.
- `ARCHY_CHECK_UPSTREAM=1 python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py'`: 4 passed, exit 0; pinned source hashes matched.
- `qmlscene`, `quickshell`, `omarchy-shell`, and `qmlformat`: absent from host PATH. Generic QML execution in the later container harness is recorded below; no actual Omarchy execution is claimed.

## Review request

Gauss accepted the scoped close/reopen fix in the round-1 review and retained useful report display as a blocker. Hume approved the exact completion rule in `omarchy-completion-rule-review.md`. Final scoped implementation review by Gauss is still required; no review is represented as complete.

## Generic headless Quickshell harness, post round 1

Gauss's lifecycle rereview at `omarchy-plugin-review-round1.md` accepts the original close/reopen P1 as addressed and retains the useful-report blocker. A new test-only harness copies the actual `DoctorProcess.qml` and `DoctorModel.js` into disposable container tmpfs, loads them with Quickshell, and mounts a read-only `/usr/bin/archy` fixture that accepts only `doctor --json`. It does not load Omarchy `Main.qml`, themes, or an installed target. Production command, environment, QML, CLI, and storage remain unchanged.

Test profile: immutable local image `sha256:c4050a4bcd4c1f640d8c77b9635bfa70f448da232f1e4b878ce2258c3998de05`, `quickshell 0.3.2-1`, `qt6-base 6.12.0-3`; offscreen/software rendering, no network, no capabilities, no privileged mode, source and fixture mounted read-only, disposable tmpfs home/runtime. The harness now runs as user `65534:65534`; focused rerun evidence follows below. This is **generic Quickshell on WSL via an Arch container**, not Omarchy execution or the originally researched v0.2.1 runtime profile.

`ARCHY_RUN_HEADLESS_QS=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -v` exited 1: 5 tests ran, 3 passed and 1 valid-report test failed for both exit 0 and exit 1. Each valid case produced `report:null` with `Doctor stream completion unverified; report unavailable`, demonstrating the literal `false` blocker through the real QML component. Other native cases pass: exit 2, malformed/truncated JSON, stdout/stderr flood, 20-second timeout, missing binary, close/reopen, and split Unicode. QML `busy` notifications were observed as `[true,false]` for a single request and `[true,false,true,false]` for close/reopen; the second child was actually launched. The test-only ordering probe observed the full expected ASCII fixture text before `exited` on this image, while split UTF-8 produced replacement characters. These observations are bounded to this image and fixture; they do not prove all installed Qt profiles or make `exitStatus` a stream-completion signal.

At that checkpoint, `python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py' -v` exited 0 with 11 tests discovered, 4 passed and 7 opt-in native tests skipped by default; the Node suite passed 7. Current results follow below.

Root reports the ASCII-escaped doctor JSON wire contract frozen and verified in the CLI, with persisted canonical bytes unchanged. At this historical checkpoint, report acceptance remained off; the reviewed implementation and retest are recorded below. The separate Omarchy target gate and Gauss S1 acceptance remain open.

## Read-only rebuilt doctor probe in the generic harness

After the fixture matrix, `ARCHY_RUN_REAL_DOCTOR=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -k RealDoctorTransport -v` exited 0 with 2 tests passing. The test mounted the frozen WSL-built `target/debug/archy` read-only at `/usr/bin/archy` in the same immutable container; binary SHA-256 `aa74b7d33eba48cef40ab6b588e4e30b9cd7f67004bff46b9ce4881732109426`, size 66,106,792 bytes. The command and QML environment remained fixed; no host home, network, capabilities, privileged mode, Omarchy shell, or runtime installation was used.

Before the completion rule, the test-only ordering probe observed `[stdout, exited]`, 6,778 ASCII stdout bytes, no stderr text, exit 1, and a parseable schema-1 report with 12 checks, `ready:false`, and `target_acceptance:deferred`. The actual `DoctorProcess.qml` showed `[busy:true,busy:false]` and kept `report:null` with its explicit completion-unverified message. This was evidence for that exact image and WSL-built binary, **not** Arch package ABI acceptance, general pipe-drain proof, or installed Omarchy behavior. Production display results after the rule follow below.

Root's separate direct container doctor probe, under user `65534` with the same read-only binary and no network or capabilities, reported exit 1, empty stderr, and 6,778 ASCII JSON bytes (SHA-256 `4821f9238db0e3307c8b359acab81e1788432556924f956226acb5b2e4c5e600`). It parsed schema 1, `os_id:arch`, `environment:wsl`, `ready:false`, and deferred target acceptance. This corroborates the wire and executable in the development image; the QML parser and lifecycle evidence is the separate harness above.

After switching the QML harness to user `65534:65534`, focused reruns passed for the real doctor transport (2/2), close/reopen (1/1), and parser ordering/Unicode loss (1/1). The valid-report QML test remained red for both exit 0 and 1 with the same explicit completion-unverified message. No production acceptance switch was changed.

## Hume-approved completion rule implementation; freeze for Gauss final review

Current scoped changes: `integrations/omarchy/plugin/{DoctorModel.js,DoctorProcess.qml}`; `tests/omarchy/integration/{doctor_model.test.js,test_plugin_contract.py,test_headless_doctor.py,fixtures/archy-doctor-fixture.sh}`; `docs/omarchy/plugin.md`; this handoff. The other plugin and harness files remain part of the S1 review set. No CLI, Cargo, backend, install, or user config file was edited in this slice.

The fixed packaged producer contract is `/usr/bin/archy doctor --json`: one bounded ASCII-escaped JSON report on stdout, flushed before a normal exit 0/1; empty stderr on those exits; no surviving child holding the pipes. Under the reviewed Quickshell 0.3.2 / Qt 6.12 source and native ordering evidence, `exited` follows the channel reads and parser endings for this trusted producer. This is a profile-bound rule, not a generic child-exit, EOF, or installed Omarchy guarantee. `DoctorModel.js` now rejects every non-ASCII stdout chunk before accumulation and rejects any stderr before report acceptance. `DoctorProcess.qml` accepts only normal exit 0/1 for the active generation while the panel is open and the probe is not failed/cancelled, with bounded stdout and empty stderr; the model then parses exactly one full schema-1 report. Fixed argv/environment, 64 KiB per-stream limit, 20-second deadline, TERM/KILL, stale-result discard, and close/reopen queue remain intact. No project, launch, or approval authority was added.

Source basis: [Hume's rule review](omarchy-completion-rule-review.md), [Quickshell 0.3.2 Process](https://github.com/quickshell-mirror/quickshell/blob/v0.3.2/src/io/process.cpp), and [Qt 6.12.0 QProcess](https://github.com/qt/qtbase/blob/v6.12.0/src/corelib/io/qprocess.cpp). The source ordering is combined with the exact-profile native fixture and real CLI observations; it does not identify an installed Omarchy runtime.

Fresh focused evidence:

- `node --test tests/omarchy/integration/doctor_model.test.js`: exit 0, 8 passed.
- `python3 -m unittest discover -s tests/omarchy/integration -p test_plugin_contract.py -v`: exit 0, 4 passed.
- `ARCHY_RUN_HEADLESS_QS=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -k HeadlessDoctor -v`: exit 0, 5 passed. Actual QML component now displays valid exit-0/1 reports; exit 2, crash, malformed/truncated/extra-document JSON, small/flood stderr, stdout flood, unsplit non-ASCII, split Unicode, timeout, start failure, and close/reopen remain unavailable or refresh correctly. Test-only ordering probe still verifies ASCII output before `exited` for this image.
- `ARCHY_RUN_REAL_DOCTOR=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -k RealDoctorTransport -v`: exit 0, 2 passed. The frozen WSL-built CLI report displays through the actual QML component, while the separate probe checks ASCII, fields, empty stderr, and event order. This is development-image evidence, not an Arch package ABI or installed Omarchy test.
- `ARCHY_CHECK_UPSTREAM=1 python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py' -v`: exit 0, 11 discovered, 4 passed and 7 opt-in runtime tests skipped; pinned Omarchy source hashes matched. `git diff --check` on owned paths: exit 0.

Freeze these implementation and test files for Gauss's final scoped review. OM-A22-01, packaged binary identity, installed runtime profile, theme/focus/toggle, and actual Omarchy execution remain deferred.
