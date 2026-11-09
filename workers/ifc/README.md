# IFC4 schematic worker (WSL development slice)

This MIT licensed worker maps one validated Task02 rectangle/L Variant to IFC4
spatial hierarchy and `IfcBuildingElementProxy` storey masses. It always records
`DRAFT`; it does not establish approval, design feasibility, professional
compliance, or BIM detail. The Python parser checks the Task02 parameter-derived
footprint, bitwise coordinates and canonical variant ID independently of Rust.
The Rust Task02 implementation remains the authoritative identity source.

## Runtime and invocation

The tested runtime is CPython 3.12.16 built from the verified official source
archive into workspace `.tools/ifc-python`, then a dedicated `.tools/ifc-venv`.
`requirements.lock` pins and hashes all 15 runtime packages for Linux x86-64;
`pytest` is needed because IfcOpenShell's EXPRESS rule executor imports
`_pytest` at runtime. The separate `build-requirements.lock` pins and hashes
the five packages used to build the original worker wheel (`build`,
`setuptools`, `wheel`, `packaging`, and `pyproject-hooks`). The wheel was built
in a separate workspace `.tools/ifc-build-venv` with `--no-isolation` after
hash-verified installation of that build lock. The build lock is not a second
runtime lock; the runtime versions remain those in `requirements.lock`.
Install the built worker wheel into the runtime venv without dependency
resolution, then use a
fixed argument vector:

```sh
/absolute/path/to/.tools/ifc-venv/bin/python -I -m archy_ifc_worker \
  --project-root /absolute/private/project-root \
  --request /absolute/private/project-root/ifc-request.json
```

The caller creates the request file read-only and the parent of the logical
`output_dir`. The worker creates that final directory only if absent, writes
`model.ifc` and `validation.json`, then writes `response.json` last. It never
overwrites an occupied destination. The response returns file hashes and sizes;
A09 must independently verify them before A05 publishes any artifact. The
worker is not an OS sandbox. A09 still owns no-follow path checks, quotas,
timeouts, cancellation, network/process isolation and acceptance. The request
has no import paths, executable options or arbitrary code. The worker itself
opens the request through canonical, descriptor-relative no-follow components;
its native verifier and EXPRESS validator have 30-second deadlines and 64 KiB
per-stream diagnostic caps. The verifier child reads at most 1 MiB + 1 byte of
the request. Internal failures retain bounded diagnostics in `validation.json`
and return no success artifacts. A09 remains responsible for whole-worker and
descendant containment, independent output quotas, and publication decisions.

Request fields are exact: `schema_version="ifc-worker-v1"`,
`operation="export_schematic_ifc"`, `request_id`, `project_id`, `variant_id`,
`run_id`, `variant` (the canonical Task02 serialized Variant), `site`
(`polygon_xy_m`, `north_angle_deg`), `source_digest` (SHA-256),
`source_artifacts` (up to 16 logical `{path,sha256}` references), and a
project-relative `output_dir`. A request is at most 1 MiB. The worker accepts
one 5/7-vertex closed rectangle/L footprint, 1–100 storeys and bounded finite
coordinates/heights. The logical path is validated but is not authority to
read source artifacts. Successful export does not change failed design checks.

The local unit and native tests run with:

```sh
python3 -m unittest discover -s tests/backend/ifc -p test_protocol.py -v
/absolute/path/to/.tools/ifc-venv/bin/python -m unittest discover -s tests/backend/ifc -p 'test_*.py' -v
```

The installed runtime emitted `No SQL support: No module named '_sqlite3'`.
The tested IFC export/reopen/tessellation and EXPRESS validation paths ran;
SQLite-backed IfcOpenShell features are outside this slice and untested.

## Dependency and license inventory

The installed wheel metadata reports: IfcOpenShell 0.9.0.post1
LGPL-3.0-or-later; isodate 0.7.2 BSD; lark 1.3.1 MIT; NumPy 2.5.4 with
BSD-3-Clause, 0BSD, MIT, Zlib and CC0 notices; pyparsing 3.3.3 MIT;
python-dateutil 3.0.0 BSD/Apache dual license; Shapely 2.2.0 BSD-3-Clause
and bundled GEOS notice; six 1.17.0 MIT; typing-extensions 4.16.0 PSF-2.0;
tzdata 2026.6 Apache-2.0; pytest 9.0.0 MIT; iniconfig 2.3.1 MIT;
packaging 26.3 Apache-2.0 OR BSD-2-Clause; pluggy 1.7.0 MIT; Pygments
2.21.0 BSD-2-Clause. These are metadata/installed-notice observations, not a
distribution audit of the IfcOpenShell native wheel or all bundled notices.
The separate build environment reports build 1.6.1 MIT, setuptools 80.10.2
MIT, wheel 0.45.1 MIT, pyproject-hooks 1.3.3 MIT, and packaging 26.3
Apache-2.0 OR BSD-2-Clause.
The IfcOpenShell wheel did not expose a top-level license file in installed
metadata; distribution must retain/resolve its upstream LGPL obligations and
native dependency notices before packaging. Python original glue is MIT; the
third-party runtime keeps its own licenses.

Native WSL evidence is recorded in `docs/plans/ifc-worker-handoff.md`. Omarchy
execution, independent A13 engine, A09 sandbox and A05 publication remain
separate gates.
