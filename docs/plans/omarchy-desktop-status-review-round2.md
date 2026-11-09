# A22 read-only desktop status review, round 2

**Verdict: scoped PASS; no verified findings.** I independently reviewed the frozen tracked and untracked A22 source at base `d4b6ca835a6af8577ff25f5966bdd9ea690e9b84`. The round-one selected-run mismatch is rejected by the actual native QML component. This clears the reviewed development-profile gate for publication; installed Omarchy target acceptance remains deferred.

## Intent and result

The panel must read one explicitly selected local project's H2 verified status through fixed CLI arguments, show the correct selected run, and grant no H3 open or launch authority. `DesktopProcess.qml` now computes `requestedRunId` once, uses it in both `statusArgv()` and `newProbe()`, and `DesktopModel.js` validates the returned status against that probe. With no `--run`, the selected summary must equal the first run's complete summary or be null for an empty page. With `--run`, the selected ID must equal the requested ID and its complete summary must match the page entry if that run is on the page. An explicit off-page run is allowed: `crates/archy-cli/src/desktop_status.rs` searches all accepted receipts for the requested ID, independently of the page slice.

The original round-one QML reproduction was rerun by replacing only the disposable `accepted_status.json` fixture read via `unittest.mock.patch`, as in round one. Its status argv was `["desktop","status","--project","/tmp/My Project/project.yaml","--limit","20","--json"]`; the malformed response selected page run 2. Actual QML output was `{"accepted":false,"error":"desktop status unavailable"}`. This reverses round one's observed acceptance. A fresh direct probe through `newProbe`/`append`/`complete` printed `wrongDefaultAccepted:false`, `wrongExplicitAccepted:false`, `rightExplicitAccepted:true`, `offPageExplicitAccepted:true`, `emptyDefaultAccepted:true`.

## Commands and observed checks

All commands ran from `/home/butahlecoq/work/archy/.worktrees/a22-desktop` against the hashes below.

| Command | Result |
| --- | --- |
| `node --test tests/omarchy/integration/doctor_model.test.js tests/omarchy/integration/desktop_model.test.js` | Exit 0; 14 passed. Covers the selected-run rules, fixed typed argv, malformed shapes, byte and array limits, stderr/non-ASCII refusal, and stale generation. |
| `python3 -m unittest discover -s tests/omarchy/integration -p 'test_*.py' -v` | Exit 0; 4 static contracts passed, 15 opt-in tests skipped by design. |
| `ARCHY_RUN_DESKTOP_QS=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_desktop.py -k NativeDesktop -v` | Exit 0; 7 passed. Actual `DesktopProcess.qml` rejected wrong default and explicit selections. Fixed argv with a spaced path, stale-cursor recovery, malformed/flood/stderr refusal, capability refusal, and manifest switch during a live child passed. |
| `ARCHY_RUN_DESKTOP_REAL=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_desktop.py -k PublishedDesktop -v` | Exit 0; 1 passed. Frozen published CLI capabilities and genesis status passed through actual QML. |
| `ARCHY_RUN_HEADLESS_QS=1 python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -k valid_reports -v` | Exit 0; 1 passed, covering native doctor exit 0/1 reports. |
| `ARCHY_RUN_REAL_DOCTOR=1 ARCHY_TEST_DOCTOR_BINARY=/home/butahlecoq/work/archy/.tools/targets/reviewed-publication/debug/archy python3 -m unittest discover -s tests/omarchy/integration -p test_headless_doctor.py -k RealDoctorTransport -v` | Exit 0; 2 passed with frozen published binary. |
| `git diff --check` | Exit 0. |

I also called `run_case('switch-manifest')`, `run_case('stale-cursor')`, and `run_case('wrong-explicit')` directly. The changed-manifest race emitted capability, old-project status, then new-project status argv; final `manifest` was `/tmp/Second Project/project.yaml`, `stale:false`, `error:""`, and busy events were `[true,false,true,false,true,false]`. The stale cursor case emitted a `--cursor` request followed by an explicit first-page request without `--cursor`; final page was fresh. The wrong-explicit case sent `--run 62040078-5b31-46af-95f6-07205fdac442`, kept the prior verified page showing `860e2341-e139-44b1-a49d-2a5e2ceedd23`, and reported `stale:true` with an error. These observations exercise the QML request state, not merely the model helper.

Source inspection confirmed native Qt `TextField` manifest input, page-limited run selection, fixed `/usr/bin/archy` argv, `clearEnvironment: true`, bounded ASCII output and empty-stderr acceptance, timeout/cancellation, and disabled H3 report/preview/Blender buttons. The H2 selection source and tests support off-page explicit selection. No reproducible finding survived. The review used interrogate's run-backed finding rule, Boundary Discipline at the CLI JSON boundary, Test Behavior for observable QML output, and Prove It Works for fresh artifact checks. No subagents were used, as requested.

## Exact source and runtime identity

The SHA-256 values below were captured before and after the review runs and were identical. Both tracked modifications and untracked source were included. The round-two document is the sole review output and is excluded from this source freeze.

| Path | SHA-256 |
| --- | --- |
| `docs/omarchy/plugin.md` | `b15d7fbe225260d198ba20a012e2e79513a28f841cc14c43132f4159a9945e18` |
| `integrations/omarchy/plugin/Main.qml` | `13bd99ec055c54c5c276d55e3329c48388e7478c54682c0d5d7d94dbc8583107` |
| `integrations/omarchy/plugin/DesktopModel.js` | `01d85b295716b78abd9df17da305e2632ccf3da1617268d73a3836fc3ae3ed3b` |
| `integrations/omarchy/plugin/DesktopProcess.qml` | `5242cb2944d97bd48dc394d1febd2ca6ec4ea4c67b73b3c1ca261cc41f24461d` |
| `tests/omarchy/integration/test_headless_doctor.py` | `512ed5800c21352ab2043150b980c79d020c6a6772727b652d5c1896f0695aa7` |
| `tests/omarchy/integration/test_plugin_contract.py` | `d53d9c0aeddf9825ad34c754b554b5fbc708bb54a715fa5884c8a9de0effa1e5` |
| `tests/omarchy/integration/desktop_model.test.js` | `e9ef40ad2d97bbb2a92907ea906a9029fb8b7fdb1c60d0904bd5b08a955ce05c` |
| `tests/omarchy/integration/fixtures/archy-desktop-fixture.sh` | `31f9d1d415ed3a7e4bd03003176dcd69e8a58476aecc17ad1971426893f32a19` |
| `tests/omarchy/integration/headless/desktop-shell.qml` | `085a87c7f23d195241bd08bcb85151e6e85774893909e4c29974acc64a83eefc` |
| `tests/omarchy/integration/test_headless_desktop.py` | `06a1c1f7b81e4f4e8b23c29b7e43e3dbaf851d6f534d1e380c2ee98af826d714` |
| `docs/plans/omarchy-desktop-status-handoff.md` | `cb0cf10c6103541f00a6ba879918eb2b84146f3274d890884e1231ee53fd7274` |
| `docs/plans/omarchy-desktop-status-review-round1.md` | `f0f7950dc5b87c7507ea1f7347dd0847fe95f938ddc0fe26eedb9e5eae606e3e` |

`docker image inspect` confirmed immutable image `sha256:c4050a4bcd4c1f640d8c77b9635bfa70f448da232f1e4b878ce2258c3998de05`. The published binary used for the two real-binary suites was `/home/butahlecoq/work/archy/.tools/targets/reviewed-publication/debug/archy`, SHA-256 `d203bdbae6f18aa28c6582e42119c07ab2ac9031ed8661ef0fdfa8f32fcf6ad8`, bind-mounted read-only to `/usr/bin/archy` inside the container.

## Limits

The generic Quickshell 0.3.2 / Qt 6.12 container instantiates `DesktopProcess.qml`, not Omarchy `Main.qml` and its theme. The published CLI probe covers genesis status only; accepted-run mismatch defense is exercised with fixture responses against actual QML, and the off-page rule is checked against H2 source and the model. No installed Arch package identity, actual Omarchy UI behavior, or target filesystem gate was tested. These target checks remain deferred. No product edit, commit, push, host install, or main-workspace edit was made.
