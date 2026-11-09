# Initial `doctor` command plan

## Scope

Add a read-only local diagnostic at `.venv/bin/python -m archy doctor`, with
`--json` for machine-readable output. It reports Python and IfcOpenShell
availability and checks Blender by launching a bounded factory-startup probe.
The probe imports `bpy`, `math`, `hashlib`, and `subprocess` inside Blender and
reports Blender's embedded Python prefix. This demonstrates startup and import
availability in the embedded interpreter, not compatibility with Archy's
Python environment. Each check contains a name, status, detail, and an
optional remedy. Statuses are `available`, `unavailable`, or `deferred`. The
command returns exit status 1 when required Blender or IFC availability is
missing, and 0 otherwise.

This is a narrow availability report, not a compatibility matrix or a full
readiness assessment. Version compatibility, desktop containment, and
packaging readiness are explicitly deferred.

## Risks and controls

- Blender is an external process and can hang. Bound its probe with a timeout
  and report a useful unavailable result when that timeout expires.
- Use Blender's factory startup so the probe does not load the user's scene or
  preferences. Import only `bpy`, `math`, `hashlib`, and `subprocess`, and
  include the embedded Python prefix in the result for diagnosis.
- Keep the probe nonmutating: do not create project files or modify the
  environment while checking availability.
- Blender embeds Python and can inherit incompatible Python environment
  settings. Start it with a controlled environment that excludes
  `PYTHONHOME` and `PYTHONPATH` and uses a system executable path for `PATH`.
- Availability does not prove that installed versions work together. The
  reported embedded Python prefix is diagnostic only. Details and optional
  remedies must avoid implying compatibility or complete readiness.
- Deferred checks should remain explicit in output so callers do not interpret
  an incomplete probe as a clean full-system assessment.

## Verification

Ran `env PATH=/home/butahlecoq/.local/share/mise/installs/python/3.14.8/bin:/usr/bin .venv/bin/python -m archy doctor --json`, exit 0. Snapshot `/tmp/archy-doctor-runtime.json` reports Blender 5.2.2 LTS, embedded Python 3.14.8, prefix `/usr`, and IfcOpenShell 0.9.0.post1. Standard-library imports succeeded. `git diff --check` passed.

The controlled Python paths do not constitute full environment containment. Blender configuration and loader variables remain inherited and captured process output has no size cap. These remain limitations of this diagnostic; isolation is explicitly deferred.
