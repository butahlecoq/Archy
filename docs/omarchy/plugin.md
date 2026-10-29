# Archy status plugin (A22 S1)

`integrations/omarchy/plugin/` is a standalone Omarchy shell panel root. It contains the manifest and every referenced file. S1 has no installer, configuration edit, menu entry, hook, or runtime installation step.

The panel invokes `/usr/bin/archy doctor --json` with a fixed argument vector. On the reviewed Quickshell 0.3.2 / Qt 6.12 profile, a normal exit 0 or 1 can display one bounded, ASCII-escaped schema 1 report with empty stderr. The panel shows host, supported baseline, capability prerequisites, check reasons/remedies, and deferred target gates. It rejects non-ASCII stdout, stderr output, malformed or extra JSON, failed or cancelled processes, and oversized output. A previous report is marked stale after a failed refresh. Doctor readiness and capability prerequisites do not authorize project or launch actions.

This completion rule is limited to the trusted packaged `/usr/bin/archy doctor --json` producer: it writes one ASCII JSON document, flushes before normal exit 0/1, writes no stderr on those exits, and does not leave another process holding its pipes. The rule relies on the reviewed Quickshell/Qt source ordering and native development-profile evidence; it is not a generic process-EOF guarantee. Installed binary identity and Omarchy target compatibility still require validation.

Project, run, report, preview, and Blender actions are unavailable until H1–H3 provide typed CLI contracts. S1 does not accept project paths, commands, or executable data from the summon payload. Only an empty payload or `{}` opens the panel.

The manifest/UI source profile is Omarchy v4.0.6; the native doctor transport was tested with Quickshell 0.3.2 / Qt 6.12 in an isolated Arch container on WSL. This repository has no installed Omarchy shell on the host. Target validation of packaged binary identity, manifest discovery, QML imports, focus, close/toggle synchronization, scaling, theme behavior, and real installed `/usr/bin/archy` execution remains pending. Do not treat the development profile as installed compatibility.

Developer contract checks:

```sh
python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py'
node --test tests/omarchy/integration/doctor_model.test.js
ARCHY_CHECK_UPSTREAM=1 python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py'
```

The last command fetches the pinned v4.0.6 gallery and registry source and checks SHA-256 hashes. These tests do not run the QML plugin.

An opt-in generic Quickshell harness runs the real `DoctorProcess.qml` inside the pinned Arch container: `ARCHY_RUN_HEADLESS_QS=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -v`. Valid-report and failure-matrix tests pass on the reviewed profile. It provides no Omarchy Main/theme or installed-target acceptance.

`ARCHY_RUN_REAL_DOCTOR=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -k RealDoctorTransport -v` additionally maps the locally rebuilt `target/debug/archy` read-only to the fixed container command path. This checks the actual ASCII doctor wire through generic Quickshell; it is not an Arch package or Omarchy target test.
