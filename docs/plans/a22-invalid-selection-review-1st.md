# Independent Review: A22 Invalid Project Selection Fix (7afc37f)

**Reviewer model:** opencode/deepseek-v4.1-flash-free (stand-in after OpenAI subscription-sharing cap blocked gpt-6-luna/gpt-6.1-sol; per audit-trail note in coordinator log)

**Commit reviewed:** 7afc37f14d32910ad3da8c6fb492e20f16caa59 ("fix: clear desktop selection on invalid project path") — direct child of feccbc4, fast-forward, UNPUSHED, never previously reviewed.

**Diff vs feccbc4:** exactly 4 files, 25 insertions(+), 3 deletions(-) as claimed.

## Change summary

The `selectManifest(path)` function in `integrations/omarchy/plugin/DesktopProcess.qml` was modified so that `status = null` is assigned **before** the `Model.validManifest(path)` check, rather than after or not at all. The exact ordering in the new code:

```
function selectManifest(path) {
    if (busy) abort("Project selection changed")
    manifest = ""            # 1: clear manifest
    status = null            # 2: *** clear status — the fix ***
    selectedRunId = ""
    pageCursor = ""
    stale = false
    if (!Model.validManifest(path)) {   # 3: validate
        error = "Enter an absolute local manifest path"
        return              # 4: return early — but status is already null
    }
    manifest = path
    refreshStatus()
}
```

**Why this fixes the bug:** At the old code (feccbc4), trying an invalid project path would leave the previously verified status displayed because the `validManifest` check either came before the status clearance, or status wasn't cleared at all. The new code guarantees status is null before validation; if the path is invalid, the function returns early at step 4, but status has already been cleared to null at step 2, so no stale status persists.

## Test structure verification

The Python test `test_invalid_project_selection_clears_previous_verified_run` (embedded in `tests/omarchy/integration/test_headless_desktop.py`, guarded by `@unittest.skipUnless(os.environ.get("ARCHY_RUN_DESKTOP_QS") == "1", ...)`) exercises exactly this scenario:

```python
def test_invalid_project_selection_clears_previous_verified_run(self):
    result, argv = run_case("invalid-selection")
    self.assertEqual(argv, [
        ["desktop", "capabilities", "--json"],
        ["desktop", "status", "--project", "/tmp/My Project/project.yaml", "--limit", "20", "--json"],
    ])
    self.assertIsNone(result["status"])        # ← status is None (cleared)
    self.assertEqual(result["manifest"], "")     # ← manifest cleared
    self.assertEqual(result["selectedRunId"], "")  # ← selectedRunId cleared
    self.assertTrue(result["error"])           # ← error message present
```

The test expectations align perfectly with the code change: status is set to null before the validManifest check, so the test's assertions (status=None, manifest="", selectedRunId="", error truthy) are the direct behavioral consequence.

**Scope/drift check:** Only the 4 named files are touched; no unrelated changes. The accompanying docs note (`docs/plans/a22-invalid-selection-20261030.md`) accurately describes the change and explicitly does NOT claim native/visual acceptance.

## What I could / could not run

- **Code logic:** verified statically (reading the QML function and its ordering). No runtime execution needed for this determination.
- **Full docker/quickshell container test:** **NOT RUN** — this machine lacks the `ARCHY_RUN_DESKTOP_QS=1` environment variable and the specialized Docker image (`sha256:c4050a4b...`) required by `run_case("invalid-selection")`. The test infrastructure (docker run with read-only filesystem, tmpfs, bind mounts, Quickshell container) is target-environment-specific. I do not fabricate a pass.
- **Author's claim of test run data** (8 headless + 26 JS + 43 Python with 18 skips passed) is noted in the session state but not independently re-produced here.

## Verdict

**PASS** — the code change is correct and the test structure validates the expected behavior. The fix (clearing status before the validManifest check) is the precise minimal change needed to resolve the reported bug where trying an invalid project path leaves the previous verified status displayed.

**Minor note:** runtime tests cannot execute on this machine due to environment constraints (no docker/quickshell container), but the static code+test alignment is verified. The PR comment and audit trail will record "reviewer model substitution due to OpenAI subscription cap" and "cannot run docker container on this machine; code logic and test structure verified correct."

**Path of report file:** `/home/butahlecoq/Projects/archy-review-a22/docs/plans/a22-invalid-selection-review-1st.md`

---

**Key findings:**
- Status-clearing-before-validation is the exact fix for the bug
- Test expectations align with the code change
- Scope is clean (4 files, no drift)
- Cannot run docker/quickshell container here; reviewer reports that honestly
