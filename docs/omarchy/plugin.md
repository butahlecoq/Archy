# Archy Omarchy status plugin

`integrations/omarchy/plugin/` is a standalone Omarchy shell panel root. It contains the manifest and every referenced file. The doctor and read-only project status slices have no installer, configuration edit, menu entry, hook, or runtime installation step.

The panel invokes `/usr/bin/archy doctor --json` with a fixed argument vector. On the reviewed Quickshell 0.3.2 / Qt 6.12 profile, a normal exit 0 or 1 can display one bounded, ASCII-escaped schema 1 report with empty stderr. The panel shows host, supported baseline, capability prerequisites, check reasons/remedies, and deferred target gates. It rejects non-ASCII stdout, stderr output, malformed or extra JSON, failed or cancelled processes, and oversized output. A previous report is marked stale after a failed refresh. Doctor readiness and capability prerequisites do not authorize project or launch actions.

This completion rule is limited to the trusted packaged `/usr/bin/archy doctor --json` producer: it writes one ASCII JSON document, flushes before normal exit 0/1, writes no stderr on those exits, and does not leave another process holding its pipes. The rule relies on the reviewed Quickshell/Qt source ordering and native development-profile evidence; it is not a generic process-EOF guarantee. Installed binary identity and Omarchy target compatibility still require validation.

The panel also reads H1/H2 desktop capabilities and status. You explicitly enter an absolute local project manifest path; the plugin does not scan home, remember a project, or accept a path from the summon payload. It shows the current project ID/revision/source digest, newest-first verified accepted runs, aggregate required-check result, one selected run's verified artifact IDs/kinds/byte lengths/hashes, and `backend_jobs: unknown`. A next-page cursor can be used for another 20 runs; if it is stale, “Refresh first page” starts again without reusing the cursor. These are metadata views only. Report, preview, and Blender opening remain disabled until the H3 checked opener exists. Only an empty payload or `{}` opens the panel.

Desktop probes use the fixed `/usr/bin/archy desktop capabilities --json` and `/usr/bin/archy desktop status --project <explicit absolute manifest> --limit 20 [--run <verified run ID>] [--cursor <typed cursor>] --json` forms. They accept only bounded ASCII v1 JSON and normal exit 0 with empty stderr. An invalid/changed selection cancels the old request. No CLI status data supplies an executable command, project action, job start, or approval grant.

The manifest/UI source profile is Omarchy v4.0.6; the native doctor transport was tested with Quickshell 0.3.2 / Qt 6.12 in an isolated Arch container on WSL. This repository has no installed Omarchy shell on the host. Target validation of packaged binary identity, manifest discovery, QML imports, focus, close/toggle synchronization, scaling, theme behavior, and real installed `/usr/bin/archy` execution remains pending. Do not treat the development profile as installed compatibility.

Developer contract checks:

```sh
python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py'
node --test tests/omarchy/integration/doctor_model.test.js
node --test tests/omarchy/integration/desktop_model.test.js
ARCHY_CHECK_UPSTREAM=1 python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py'
```

The last command fetches the pinned v4.0.6 gallery and registry source and checks SHA-256 hashes. These tests do not run the QML plugin.

An opt-in generic Quickshell harness runs the real `DoctorProcess.qml` inside the pinned Arch container: `ARCHY_RUN_HEADLESS_QS=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -v`. Valid-report and failure-matrix tests pass on the reviewed profile. It provides no Omarchy Main/theme or installed-target acceptance.

`ARCHY_RUN_REAL_DOCTOR=1 ARCHY_TEST_DOCTOR_BINARY=/absolute/path/to/archy python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -k RealDoctorTransport -v` maps a selected test binary read-only to the fixed container command path. In this worktree, use `/home/butahlecoq/work/archy/.tools/targets/reviewed-publication/debug/archy`. This checks the ASCII doctor wire through generic Quickshell; it is not an Arch package or Omarchy target test.

For desktop status, run `ARCHY_RUN_DESKTOP_QS=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_desktop.py -k NativeDesktop -v`. The frozen published CLI is used only when `ARCHY_RUN_DESKTOP_REAL=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_desktop.py -k PublishedDesktop -v` is selected. Both are generic headless Quickshell container checks; neither runs Omarchy Main/theme or validates an installed target.
