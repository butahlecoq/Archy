# A11/A12 IFC worker development handoff

Date: 2026-10-29. Scope: `workers/ifc/**`, `tests/backend/ifc/**`, this
handoff and ADR0004 only. No Rust, Cargo, CLI, storage, commit, push or agent
changes. This is WSL native development evidence, not Omarchy acceptance.

The original evidence and pending-review state below are retained as the
pre-review snapshot. The round1 addendum at the end records fixes to the four
findings in `ifc-review-final.md`, newer test counts, and Carson rereview status.

## Implemented boundary

- Installed package entry point `python -I -m archy_ifc_worker` accepts an exact
  1 MiB bounded request and only a canonical Task02 serialized Variant. It
  recomputes footprint ordering/coordinates and the Task02 SHA-256 variant ID.
  Unknown/duplicate fields, summary/approval claims, nonfinite values,
  unsupported capacities and unsafe logical paths fail.
- One IFC4 Project→Site→Building→Storeys aggregation, one contained
  `USERDEFINED` proxy per storey, one closed rectangle/L profile and one +Z
  Body/SweptSolid extrusion per proxy. Explicit metre/square-metre/cubic-metre
  SI units, building origin XY, storey Z and proxy relative identity. North is
  metadata. All IfcRoot GUIDs are stable UUIDv5 values independent of run ID.
- `Archy_SchematicMassing` stores source digest/references, project/variant/run
  IDs, versions, coordinate/method/tolerance text, UTC export time and DRAFT.
  Analytic custom quantities use explicit units and methods. The output is a
  schematic mass, with no professional approval or detailed BIM semantics.
- `model.ifc` is reopened in a fresh process and checked for schema, units,
  hierarchy, GUID uniqueness, exact profile vertices, placement, quantities,
  draft provenance, tessellated world bounds and volume. The official
  `ifcopenshell.validate ... --rules --json` command is run separately and its
  exit and findings recorded. `validation.json` and `response.json` are fixed
  basenames; response is last. Response artifact hashes are evidence for A09 to
  recheck, not publication authority.

## Exact local runtime provenance

- Official CPython 3.12.16 XZ source URL:
  `https://www.python.org/ftp/python/3.12.16/Python-3.12.16.tar.xz`.
  Downloaded bytes SHA-256:
  `b5796915dc45ff745cfb4e990a7e7c93d6f546328a102ce31448d69ebc37575c`,
  matching the official release page. Built with GCC 15.2.0 under
  `/home/butahlecoq/work/archy/.tools/ifc-python/cpython-3.12.16` and venv
  `/home/butahlecoq/work/archy/.tools/ifc-venv`; `sys.version` reports 3.12.16.
  The `uv python install 3.12.16` attempt exited 2, “No download found”, so
  the verified source was built instead. The first source build lacked zlib and
  OpenSSL development headers and `make install` exited 2. The successful
  build used headers extracted under `.tools/ifc-build/deps` from Ubuntu
  `zlib1g-dev` 1:1.3.dfsg+really1.3.1-1ubuntu3.2 (deb SHA-256
  `2e4663ab8d018aff8351ee949e53971911b2aca3491eefde61e82111ee83cbc3`)
  and `libssl-dev` 3.5.5-1ubuntu3.7 (deb SHA-256
  `d1cf6cafb141490b7020e392964b1f901835d60cb88c3a68ef0f95ef5e930de4`).
  Compilation linked the existing system zlib 1.3.1 and OpenSSL 3.5.5 shared
  libraries, without system package installation. `import ssl,zlib` succeeded.
- OS: Ubuntu Resolute WSL2, Linux 6.6.87.2, x86-64, glibc 2.43. Installed
  IfcOpenShell 0.9.0.post1 native wrapper reports `0.9.0post1+b5e509b`;
  NumPy 2.5.4 and Shapely 2.2.0. Native OpenCascade version is not exposed by
  the inspected wrapper API. The interpreter omits optional `_sqlite3`; the
  tested paths run, but IfcOpenShell prints `No SQL support: No module named
  '_sqlite3'`. SQLite-backed features are untested.
- The exact proposed Linux x86-64 IfcOpenShell wheel is 54,947,812 bytes;
  downloaded bytes SHA-256
  `39fb69217fca787264ada575e10b46b2c038c96f85810bc5275b6b505fa91f7e`,
  matching the exact PyPI JSON entry. The installed runtime uses a 15-package
  full transitive hash lock at `workers/ifc/requirements.lock`, SHA-256
  `2218b2dc0ab2376a9f63b7868cc3bbff6c98972dd8973c021bb393c89ad985d1`.
  `pytest==9.0.0` is a runtime validation dependency: the EXPRESS rule executor
  imports `_pytest`. No extras or Blender Python are used.

Selected Linux x86-64 wheel SHA-256 values (all bytes downloaded under
workspace `.tools/ifc-wheels`; the lock also records PyPI's other platform
hashes):

| Package | Version | Selected wheel SHA-256 |
| --- | --- | --- |
| ifcopenshell | 0.9.0.post1 | `39fb69217fca787264ada575e10b46b2c038c96f85810bc5275b6b505fa91f7e` |
| iniconfig | 2.3.1 | `9121e2c1fdb355232495be3194c8dfe87ccc2d5dee45947b78e68f499790d7a7` |
| isodate | 0.7.2 | `28009937d8031054830160fce6d409ed342816b543597cece116d966c6d99e15` |
| lark | 1.3.1 | `c629b661023a014c37da873b4ff58a817398d12635d3bbb2c5a03be7fe5d1e12` |
| numpy | 2.5.4 | `fbde6962867ee75b48b0ee29b2b9372ec5d617799dbaf38e82dc0596f2f7738a` |
| packaging | 26.3 | `d7193f7c8e4e93f444fde0262bf90af30e16fa0ad0ad44cb553c87339b23cd1c` |
| pluggy | 1.7.0 | `7dd7b0d8832ba3cb632c306926ded123429211b83641b35dc5c41ad2d34f9bec` |
| pygments | 2.21.0 | `2363c69b61c4a97c838da3b130dcd6468f4848992b21a82f2a63ec34377137d9` |
| pyparsing | 3.3.3 | `ece8c00a69cf01b45d0b1dedabb469c90d8caf996d4fda40f147627a122849a4` |
| pytest | 9.0.0 | `e5ccdf10b0bac554970ee88fc1a4ad0ee5d221f8ef22321f9b7e4584e19d7f96` |
| python-dateutil | 3.0.0 | `3c79630202e4152c6add678446984236efe4f57e84d8e892aab0637bab82e9fd` |
| shapely | 2.2.0 | `650a5f4d8a8e3c96982079d8c99b6ddbe6602bbd1e34c75c2b95dbc0d28ac997` |
| six | 1.17.0 | `4721f391ed90541fddacab5acf947aa0d3dc7d27b2e1e8eda2be8970586c3274` |
| typing-extensions | 4.16.0 | `481caa481374e813c1b176ada14e97f1f67a4539ce9cfeb3f350d78d6370c2e8` |
| tzdata | 2026.6 | `0a10eec6e13ef16b627fabed9a5f2ba61a62b1c67f327305f4e7e412a70b4aec` |

Installed metadata licenses and distribution caveats are inventoried in
`workers/ifc/README.md`. Bundled native/third-party notice review remains a
distribution gate; original worker MIT does not relicense IfcOpenShell LGPL.

## Test evidence and preserved failures

Exact completed verification commands (all exit 0 on the final source except
the documented initial red runs):

```sh
uv pip compile workers/ifc/requirements.in --python-version 3.12 --python-platform x86_64-manylinux_2_31 --generate-hashes --no-emit-index-url --output-file workers/ifc/requirements.lock
uv pip sync workers/ifc/requirements.lock --python /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python --require-hashes --find-links /home/butahlecoq/work/archy/.tools/ifc-build
/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python -m pip download --require-hashes --only-binary=:all: -r workers/ifc/requirements.lock -d /home/butahlecoq/work/archy/.tools/ifc-wheels --find-links /home/butahlecoq/work/archy/.tools/ifc-build --quiet
uv pip install ./workers/ifc --python /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python --no-deps --reinstall
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/backend/ifc -p test_protocol.py -v
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python -m unittest discover -s tests/backend/ifc -p 'test_*.py' -v
/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python -I -m ifcopenshell.validate /home/butahlecoq/work/archy/.tools/ifc-failures/cli1/runs/run-1/ifc/model.ifc --rules --json
cargo test --locked --workspace
```

- `uv pip compile workers/ifc/requirements.in --python-version 3.12
  --python-platform x86_64-manylinux_2_31 --generate-hashes
  --no-emit-index-url --output-file workers/ifc/requirements.lock`: exit 0,
  15 packages. `uv pip sync ... --require-hashes --find-links
  /home/butahlecoq/work/archy/.tools/ifc-build`: exit 0; 15 packages installed.
  `pip download --require-hashes --only-binary=:all: -r
  workers/ifc/requirements.lock`: exit 0, selected wheel hashes listed above.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s
  tests/backend/ifc -p test_protocol.py -v`: exit 0, 6/6 import-free tests.
- `PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python
  -m unittest discover -s tests/backend/ifc -p 'test_*.py' -v`: exit 0,
  14 tests, 13 passed and one expected skip of the missing-backend case in the
  installed backend venv. The native fixture tests measure translated rectangle area
  200 m²/GFA 600 m², translated L area 75 m²/GFA 225 m², 9.6 m height,
  Task02 serialized L, three proxies/storeys, stable IDs, identical repeated
  semantic inventories and DRAFT. Negative tests cover unknown/duplicate JSON,
  fake approval/summary, tampered identity/footprint, nonfinite/negative zero,
  capacity, traversal, occupied/symlink destination, corrupt IFC, wrong units,
  placement, profile, GUID, quantity value and quantity unit. No failing design
  checks are rewritten.
- Manual `python -I -m ifcopenshell.validate .../model.ifc --rules --json`:
  exit 0, “No validation issues found.” It prints the optional SQL warning
  above. This validator is not a proof of geometry or professional validity.
- `cargo test --locked --workspace`: exit 0 on the current shared tree; all
  Rust unit, integration and documentation test targets reported 0 failures.
  No Rust source was changed by this slice.
- Early failed CLI outputs and requests are retained under workspace
  `.tools/ifc-failures/cli1` and `cli2`; they record the initial aggregation
  count check and native stdout contaminating the fresh-process JSON channel.
  The latter is fixed by routing native import diagnostics to stderr in the
  verifier subprocess. No prior artifact was overwritten.

## Remaining independent gates

Independent review is **required and pending**. No reviewer/agent was
dispatched, per the explicit no-agents instruction. A11/A12 WSL development
tests therefore do not constitute final acceptance. Rust validation,
evaluation recomputation, design-manifest integration and supervisor invocation
are future bounded work outside this owned scope. A09 sandbox/artifact
acceptance, A05 publication, A13 independent engine plus visual comparison,
distribution notice audit, and clean native Omarchy execution remain external
gates. No A09/A05/A13 pass is claimed.

## Round1 fixes after `ifc-review-final.md`

The independent final review is preserved at [ifc-review-final.md](ifc-review-final.md).
Its four findings were reproduced or confirmed before changes and addressed
within the IFC worker paths. No review text was rewritten.

| Finding | Reproduction and correction |
| --- | --- |
| P1 lexical request escape | `test_request_read_rejects_sibling_traversal_and_symlinks` exited 1 before the fix: `/project/../secret.json` was read without a `ProtocolError`. `input_bytes` now rejects noncanonical absolute path components, checks lexical containment, and walks from `/` through directory descriptors with `O_NOFOLLOW`, opening the regular request file relative to the final descriptor with `O_NOFOLLOW`. It caps reads at 1 MiB + 1 byte. The sibling sentinel and file/directory/root symlink cases are checked. The same test exits 0 after the fix. |
| P1 active project units | `test_active_project_units_reject_missing_wrong_and_duplicate_assignments` exited 1 with three subtest failures before the fix: missing, incomplete and duplicate `IfcProject.UnitsInContext` assignments all passed reopen inspection. The verifier now requires exactly three *assigned* SI units, one each for length, area and volume, with metre/square-metre/cubic-metre names and no prefixes. The three cases exit 0 after the fix. |
| P2 native child bounds | The initial `test_subprocess.py` run exited 1 because the bounded runner and failure retention did not exist; the oversized verifier request returned an unstructured traceback. Trusted child calls now have a 30-second deadline, 64 KiB per stream, bounded buffered reads, a new process group and group kill on timeout/output overflow, including descendants holding pipes. The verifier child reads at most 1 MiB + 1 byte. Timeout returns `BACKEND_TIMEOUT`; output overflow/invalid child output returns `ARTIFACT_INVALID`. `validation.json` retains bounded diagnostics on failure and `response.json` still carries no success artifacts. Real noisy, sleeping and descendant-held-pipe process tests, child request cap, and error-retention tests pass. A09 remains the external whole-worker supervisor and artifact authority. |
| P2 relation identity/build provenance | Before the fix, relationship GUIDs used class iteration indexes and the build-system required version ranges. `test_relation_guids_follow_endpoints_and_property_roles_not_creation_order` initially exited 1 because no role-based assignment API existed. GUIDs now derive from typed parent/child endpoint names or property/quantity set names, with sorted related endpoint roles and no class index; recreating containment and property relations in reverse order preserves their GUID inventory, while changing an endpoint changes its GUID. Build backend requirements are exact in `pyproject.toml`; the separate five-package build graph is hash-locked and installed into a dedicated workspace build venv. Runtime lock versions are unchanged. |

Actual negative artifacts were preserved under workspace
`/home/butahlecoq/work/archy/.tools/ifc-round1-evidence/`:
`sibling-secret.json`, `request.json`, and
`active-units-{missing,wrong,duplicate}.ifc`. Against the fixed verifier, the
three IFC files fail with respectively “project has no active unit assignment”
and two “project active units must be exact SI length/area/volume” errors; the
lexical sibling request fails `PERMISSION_DENIED` and the sentinel bytes remain
`outside-secret`. The original early CLI failure artifacts remain in
`.tools/ifc-failures/cli1` and `cli2`.

### Build and runtime provenance after round1

The existing runtime lock remains **15 packages**, SHA-256
`2218b2dc0ab2376a9f63b7868cc3bbff6c98972dd8973c021bb393c89ad985d1`.
Its CPython 3.12.16, IfcOpenShell 0.9.0.post1 and pytest 9.0.0 versions were
rechecked in the workspace venv; all 15 installed versions match the lock.
No runtime version was changed. `pytest==9.0.0`, already present in the
runtime lock for EXPRESS validation, is now also declared as a direct package
runtime dependency.

The new **build** lock is separate: `workers/ifc/build-requirements.lock`,
SHA-256 `079eec1deec846c9beaaa1402e1409151dbf70df85e00a61ec354a4776e19288`.
It contains five exact pinned/hashing entries; all five installed versions in
`.tools/ifc-build-venv` match. Selected downloaded build-wheel SHA-256 values:

| Build package | Version | Selected wheel SHA-256 |
| --- | --- | --- |
| build | 1.6.1 | `ecd351a4be9d35a9eaaba244a7687143c9c7d4aea6ac964e7e7ddab20cbcf4e7` |
| packaging | 26.3 | `d7193f7c8e4e93f444fde0262bf90af30e16fa0ad0ad44cb553c87339b23cd1c` |
| pyproject-hooks | 1.3.3 | `5fc53fdac9f7bd63fbcdc868fb5f90b4784d78a53a3d3388cd738b807441a20b` |
| setuptools | 80.10.2 | `95b30ddfb717250edb492926c92b5221f7ef3fbcc2b07579bcd4a27da21d0173` |
| wheel | 0.45.1 | `708e7481cc80179af0e556bbf0cc00b8444c7321e2700b8d8580231d13017248` |

The worker source was copied into workspace `.tools/ifc-round1-final/src` for
the no-isolation build, leaving generated build files out of the repository.
The resulting wheel
`.tools/ifc-round1-final/dist/archy_ifc_worker-0.1.0-py3-none-any.whl`
has SHA-256 `41a982629f3a911731b475049ce9f2e3adbc47df7a0e05bb8e8e9772e233bc3e`.
It was installed with `--no-deps --reinstall` into the existing workspace IFC
venv. Installed Python module bytes match the source; both build and runtime
venvs passed `pip check`. Build-package license metadata is inventoried in
`workers/ifc/README.md`; distribution/native notice audit remains open.

### Final local verification for round1

Commands and observed results on this source snapshot:

```sh
uv pip compile workers/ifc/build-requirements.in --python-version 3.12 --generate-hashes --no-emit-index-url --output-file workers/ifc/build-requirements.lock
# exit 0; resolved 5 packages
uv pip sync workers/ifc/build-requirements.lock --python /home/butahlecoq/work/archy/.tools/ifc-build-venv/bin/python --require-hashes
# exit 0; installed 5 locked build packages
/home/butahlecoq/work/archy/.tools/ifc-build-venv/bin/python -m pip download --require-hashes --only-binary=:all: -r workers/ifc/build-requirements.lock -d /home/butahlecoq/work/archy/.tools/ifc-build-wheels --quiet
# exit 0; selected hashes above
/home/butahlecoq/work/archy/.tools/ifc-build-venv/bin/python -m build --wheel --no-isolation --outdir /home/butahlecoq/work/archy/.tools/ifc-round1-final/dist /home/butahlecoq/work/archy/.tools/ifc-round1-final/src
# exit 0; wheel hash above
uv pip install /home/butahlecoq/work/archy/.tools/ifc-round1-final/dist/archy_ifc_worker-0.1.0-py3-none-any.whl --python /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python --no-deps --reinstall
# exit 0; worker package replaced, runtime dependencies unchanged
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/backend/ifc -p test_protocol.py -v
# exit 0; 7 passed
PYTHONDONTWRITEBYTECODE=1 /home/butahlecoq/work/archy/.tools/ifc-venv/bin/python -m unittest discover -s tests/backend/ifc -p 'test_*.py' -v
# exit 0; 24 run, 23 passed, one expected skip (missing-backend case in backend venv)
/home/butahlecoq/work/archy/.tools/ifc-build-venv/bin/python -m pip check
# exit 0; no broken requirements
/home/butahlecoq/work/archy/.tools/ifc-venv/bin/python -m pip check
# exit 0; no broken requirements
cargo test --locked --workspace
# exit 0; all current shared-tree Rust test and doc-test targets reported 0 failures
```

The native suite still emits the known optional `_sqlite3` SQL-support notice;
this does not establish SQL-backed IfcOpenShell functionality. All tests above
are WSL development evidence. **Carson independent rereview is pending** on
this fixed snapshot. Rust integration, A09 sandbox/artifact acceptance, A05
publication, A13 second-engine verification, distribution audit, and Omarchy
target execution remain external gates; none is claimed passing.
