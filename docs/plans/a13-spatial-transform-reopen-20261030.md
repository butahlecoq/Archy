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
