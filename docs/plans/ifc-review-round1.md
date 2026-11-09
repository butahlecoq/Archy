# A11/A12 IFC worker round1 independent rereview

Date: 2026-10-29. Read-only rereview of the four findings in [ifc-review-final.md](ifc-review-final.md) against the frozen round1 worker source and tests. The original review is unchanged. No worker implementation, dependency installation, rebuild, commit, push, or agent delegation occurred in this rereview. All executed checks ran in WSL/native development.

## Finding dispositions

| Original finding | Rereview result |
| --- | --- |
| P1 lexical request escape | **Resolved for the tested boundary.** `input_bytes` rejects `.`/`..`, empty, NUL and noncanonical absolute components before containment; it opens each directory from `/` with `O_DIRECTORY|O_NOFOLLOW`, then opens a regular request with `O_NOFOLLOW|O_NONBLOCK` and a 1 MiB + 1 byte cap. An independent sibling-secret reproduction now returns `PERMISSION_DENIED` and leaves the sentinel as `outside-secret`. The author regression also covers root/directory/file symlinks. |
| P1 active project units | **Resolved for the tested IFC subset.** `verify.inspect` follows `IfcProject.UnitsInContext`, requires one assignment of exactly three active `IfcSIUnit` kinds, and checks metre/square-metre/cubic-metre with no prefixes. Independently removing that assignment from a freshly built model now raises `ArtifactError: project has no active unit assignment`. Author regressions cover missing, incomplete and duplicate assignments. |
| P2 native child bounds | **Resolved at the worker-internal child boundary.** `run_native_process` uses a 30 s monotonic deadline, 64 KiB cap per output stream, nonblocking selector reads, a dedicated process group and group kill on timeout/overflow, including a descendant that retains a pipe after its direct parent exits. The verifier request read caps at 1 MiB + 1 byte. Failure diagnostics are bounded and retained in `validation.json`; timeout maps to `BACKEND_TIMEOUT`, output overflow to `ARTIFACT_INVALID`. The real noisy, sleeping and descendant-held-pipe regressions pass. Whole-worker CPU/memory/artifact authority remains A09's gate. |
| P2 relation identity and build provenance | **Resolved for the stated deterministic role contract and documented build path.** Relationship GUID keys now include typed parent/child endpoint names or property/quantity-set roles with sorted children, not creation indexes. Reversing relation creation order preserves GUID inventory; changing an endpoint changes its GUID in the native test. Build requirements are exact in `pyproject.toml`, and the separate five-package hash lock has SHA-256 `079eec1deec846c9beaaa1402e1409151dbf70df85e00a61ec354a4776e19288`. The recorded no-isolation wheel build uses that locked build venv. The unchanged 15-package runtime lock hashes to `2218b2dc0ab2376a9f63b7868cc3bbff6c98972dd8973c021bb393c89ad985d1`. This does not prove an arbitrary isolated build uses hashes unless it follows the documented locked path. |

## Commands and observations

- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/backend/ifc -p test_protocol.py -v`: exit 0, 7 passed.
- `PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python -m unittest discover -s tests/backend/ifc -p 'test_*.py' -v`: exit 0, 24 run, 23 passed, one expected missing-backend skip. The venv emitted its known optional `_sqlite3` notice.
- `sha256sum workers/ifc/requirements.lock workers/ifc/build-requirements.lock`: exit 0, values above. Both build and runtime `python -m pip check` commands exited 0 with “No broken requirements found.”
- Direct temporary sibling-file traversal and active-unit removal reproductions: both exited 0 as test drivers and observed the expected rejection messages above. No repository artifact was edited for those reproductions.

The four original findings are closed for this WSL source snapshot. This rereview does not claim Rust orchestration integration, A09 sandbox/artifact acceptance, A05 immutable publication, independent A13 review/visual comparison, distribution notice audit, or actual Omarchy execution. The separate A13 web-ifc reader has native fixture measurements but is awaiting its own independent review.
