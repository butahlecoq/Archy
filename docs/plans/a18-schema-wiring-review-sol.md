# A18 schema-check wiring: independent review — 29 October 2026

**Verdict:** The narrow CI adjustment is sound. The workflow is byte-identical to `HEAD` (`ae6e6a6923fad5a4aae343cea67fcf40f62fd43e`); its existing schema step now runs the six reviewed demo tests after all existing schema and decision checks. This is evidence for the partial CLI journey only, with the release limits in `a18-integration-review-sol.md` unchanged.

## Patch and provenance

`git diff HEAD -- scripts/check-schemas.py` shows only `import sys` and a final `subprocess.run([sys.executable, '-m', 'unittest', 'discover', '-s', 'tests/demo', '-v'], cwd=ROOT, env=dict(os.environ, ARCHY_BIN=str(binary)), check=True)`. The preceding `tests/decision_contract_schema.py` invocation remains in place. `git diff HEAD --exit-code -- .github/workflows/ci.yml` exited 0; `git diff --check HEAD` exited 0.

SHA-256 (`sha256sum`):

| File | Current | Prior reviewed snapshot / HEAD |
| --- | --- | --- |
| `.github/workflows/ci.yml` | `1576af01331414791ceffe0de17177a949c692672451194deaaeac1da8a46ee4` | HEAD: same; prior reviewed snapshot: `a0088e3a90852d80b1a2c72f4fdb5e0640f242077b58f81a9edd941da3fd4859` |
| `scripts/check-schemas.py` | `25601bdacae68da6bb61ff0a9e8325930ca788ed6d7ebf7b18cb4a0671a78726` | prior reviewed snapshot and HEAD: `0de9567628ed44809c551eedc48ee3ab80e2ca9a4f59930574ba58cfe0a1c496` |
| `scripts/demo.py` | `2cc20930027a10e6a8fd33b13764caa052a55fb8b8c421cba27767d3603b9fb3` | same |
| `tests/demo/test_demo.py` | `b7aa540e205df1b2b268c1bd890ac7b688c93c7b33e561ca62cd1096ba5eb6b4` | same |
| `tests/demo/review_shim.py` | `e21e60d4a2899e9c86b3d10e03b0d7a6ea7b49189bdaca8c7d315c2c87fa7c17` | same |
| `CONTRIBUTING.md`, `docs/cli.md`, `README.md` | `2961bf7a7b95ab63f9d02c7c110a96c6eaad159c407b211194e2663d1032286d`, `417ef8ab15dac6e3bd92d76a5885f0e1b0f708019c982824bd06a4d3521bb0af`, `09e2216e8560dc2adc6eaa2720f01f83238e9890e482d64abdcb59bde7bb3d61` | all same |

## Reproduction

From `/tmp/archy-ci-repair`:

```sh
CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a02-integration-7b53-fresh python3 scripts/check-schemas.py > /tmp/a18-schema-wiring-check.log 2>&1
```

Exit **0**. Cargo metadata with the same `CARGO_TARGET_DIR` resolved that exact target directory; the executable `debug/archy` existed (SHA-256 `84af1113674209951ea847b37b841de6a232bb9df3643916adb4202543b744ff`). The script reuses that metadata-derived binary for CLI schema checks and passes its absolute path as `ARCHY_BIN`. `sys.executable` uses the running Python interpreter; the existing binary selection adds `.exe` on Windows. The existing script assumes a host `debug` build, so cross-target layouts are outside this check; CI runs on Ubuntu.

The log (`6f12425604159d28a7aac8456cafd445ab90d5ef32ceb3736b56f3fffffdaab0`) records all 16 schema files, both brief fixtures, nine error-code fixtures, 26 negative CLI fixtures, doctor and desktop checks, **50 decision contract cases passed**, then **6 demo tests, all OK**. `ARCHY_BIN` is nonempty, so the tests' `skipUnless(ARCHY, ...)` condition is satisfied; no skip appears. Both the decision invocation and new demo invocation use `check=True`, so a nonzero child status raises and fails the schema step. Local Python had `jsonschema` 4.19.2; CI installs 4.26.0. This was a local combined run, not a hosted Actions run.

No source edits, commits, pushes, installs, child agents, or other worktree edits were made. The untracked `docs/plans/active-state.md` was left untouched.
