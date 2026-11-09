# A13 spatial transform reopen slice

Base: `1c292054f9409cd27f7f0d5a2ab42a83a9722441`. WSL development only.

The fresh-process IfcOpenShell verifier checks storey and proxy world positions, but does not check the site or building world transforms. Moving the site and offsetting the building relative placement can preserve the mass geometry and existing measured positions while changing the spatial hierarchy's actual transforms. The independent web-ifc reader already checks these parent transforms.

Plan: add a native regression that writes this contradictory IFC and demonstrates acceptance, then require canonical site and building world transforms. Keep the current coordinate tolerance and native-child deadlines. Re-run the pinned Python worker checks and independent Node reader checks. No new dependency is needed. This does not establish target Omarchy acceptance, general IFC certification, or independent review.

## Result and handoff

- The new native mutation test failed before the fix with `AssertionError: ArtifactError not raised`. It moves the site to `(1, 0, 0)` and offsets the building by `(-1, 0, 0)`, leaving storey/proxy world coordinates and the tessellated mass unchanged.
- The verifier now checks site world position `(0, 0, 0)`, building world position `(origin_x, origin_y, 0)`, and identity orientation for all four spatial/product levels. It retains the existing `1e-6 m` coordinate tolerance and the worker's 30-second native-child deadlines. The independent web-ifc reader already checked the parent transforms; a new cross-engine mutation test records that rejection.
- `PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python -m unittest discover -s tests/backend/ifc -p 'test_*.py' -v`: exit 0, 25 tests, 24 passed and one expected missing-backend skip. The native interpreter reported CPython 3.12.16 and IfcOpenShell 0.9.0.post1. Its optional SQLite warning remained unchanged.
- `npm test` from `tests/backend/ifc-reader`: exit 0, 39 passed, 0 failed, 0 skipped, Node 22.22.2 with the existing pinned web-ifc installation. A temporary link to the preexisting module directory was created only in this worktree and removed after testing. The reader, bridge, STEP scope, fixtures, dependency lock, and timeout values were not changed.
- `git diff --check`: exit 0. No host services or other worktrees were changed. The tests' temporary IFC models were deleted by their existing temporary-directory cleanup; retained repository fixtures were not modified.

Independent review is pending on this frozen slice. Remaining A13/release gates include actual accepted-artifact publication and target Omarchy execution/visual interoperability. This WSL evidence does not close those gates or grant approval.

## Placement presence follow-on plan

ADR 0004 specifies the site's identity placement and the building's origin placement; the published selected-export profile and independent reader require their placement references. The writer supplies both. The IfcOpenShell placement utility treats a missing placement as identity, so the worker can accept an absent site placement or an absent building placement at the origin. Test these cases with real exported IFC and require an explicit placement at the existing worker check. This is an Archy export-profile requirement, not a statement that IFC generally forbids missing placements. Verify the independent reader separately; retain existing tolerances and deadlines.

The focused native regression failed on both mutations before the fix (`ArtifactError not raised`). The unchanged framed web-ifc reader rejected absent site and building references for both rectangle and L fixtures with `ARTIFACT_INVALID`; each unchanged fixture passed. The worker now rejects missing site and building placements before asking IfcOpenShell to calculate their transforms. It leaves the existing transform tolerance and process deadlines unchanged.

Validation on WSL:

- `PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python -m unittest discover -s tests/backend/ifc -p 'test_*.py' -v`: exit 0, 26 tests, 25 passed and one expected missing-backend skip. The test imports the worktree verifier directly; the shared environment's installed worker is not replaced.
- A private venv at `/var/tmp/archy-a13-placement-78Wmjc/venv` contains a copy of this worktree's worker and reads dependencies from the existing pinned environment. Its `python -I -m archy_ifc_worker` produced a successful rectangle export. Its separate `python -I -m archy_ifc_worker.verify` children rejected copies with missing site and building placements, each with exit 20 and the corresponding missing-placement message. The valid model, request, mutated copies and child reports remain at `/var/tmp/archy-a13-placement-78Wmjc/evidence`; the valid model SHA-256 is `cc440f218b6d1772a628038449a079f3cf07234079d83295ddf9394ebd93bb6e`.
- `node --test --test-name-pattern='framed reader requires explicit site and building placements' test/reader.test.cjs`: exit 0, one focused test passed with web-ifc 0.0.78 and Node 22.22.2. It uses the actual framed verifier and retains the existing process timeout.

The follow-on requires independent review. PR head `77afee3` adds A20 documentation on top of the original base and must be retained when these A13 commits are integrated.

After the compatibility reviewer observed the four edits in the `49f89e3` review worktree, the user directed that worktree be left untouched. Its four modified files were copied byte for byte into a new worktree based on `49f89e3`; their copied snapshot is Git tree `b77d3b2155d03c20fc31201a5933a22d8942330b`. The shared review worktree retains those edits. The private worker copy used above has the same verifier SHA-256 as the new worktree: `9cc09fdae0693766cfefb7a5f2ff09086994d5555be8b2fa8050500727161c3a`.

The later draft head is `3ad404af5ce4753cea02be86d4c54f1aa2d9a7b4`: it retains the `77afee3` A20 supplement and repairs the A23 matrix documentation. Both must be preserved during later integration. The native suite was rerun from the new worktree with the same result, 25 passed and one expected skip. The prior full Node run in the review worktree was interrupted when isolation was requested and is not counted as a completed check.

In the new worktree, `npm test` from `tests/backend/ifc-reader` exited 0 with 40 passed and no skips. The local module link used for this run was removed from the new worktree afterward; no source reader bytes or timeout settings changed.
