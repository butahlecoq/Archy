# Blender child `PATH` failure: issue #21

## Reproduction

With mise's Python directory first on `PATH`, Blender's embedded Python failed to import the standard-library `math` module:

```text
env PATH=/home/butahlecoq/.local/share/mise/installs/python/3.14.8/bin:/usr/bin /usr/bin/blender -b --factory-startup --python-exit-code 7 --python-expr 'import sys,math; print(sys.prefix)'
```

The command exited with status 7. Supplying the system path to the Blender process avoids the failure:

```text
env PATH=/usr/bin:/bin /usr/bin/blender -b --factory-startup --python-exit-code 7 --python-expr 'import sys,math; print(sys.prefix)'
```

## Fix and verification evidence

The full Archy workflow now isolates its Blender child process with `PATH=/usr/bin:/bin` and removes `PYTHONHOME` and `PYTHONPATH`, so the caller's mise environment does not contaminate Blender's embedded Python. The workflow was run to `/tmp/archy-path-fixed` and verified successfully.

Standalone Blender examples in `docs/massing-operations.md` use `env -u PYTHONHOME -u PYTHONPATH PATH=/usr/bin:/bin blender ... --python-exit-code 1` so embedded script errors produce a failing command status.

## Review status

The fix is implemented and independently reviewed. Confirmation from the user's existing foot session remains pending; the reproduced mise-PATH failure passes locally.

## Final verification

The final implementation uses `os.defpath` for the child PATH and removes all
`PYTHON*` overrides. The standalone examples use `/usr/bin:/bin` explicitly.

```bash
env PATH=/home/butahlecoq/.local/share/mise/installs/python/3.14.8/bin:/usr/bin .venv/bin/python -m archy.workflow examples/massing-brief.yaml /tmp/archy-study --blender /usr/bin/blender
.venv/bin/python -m archy.verify /tmp/archy-study
```

Both commands exited 0. Workflow output is retained in
`/tmp/archy-mise-repro.log`. Verification printed
`Archy workflow artifacts verified`, covering manifest hashes and sizes,
comparison consistency, geometry inspection, and IFC verification.
The bundle is at `/tmp/archy-study`.

```bash
env PATH=/usr/bin:/bin /usr/bin/blender --background --factory-startup --python-exit-code 7 --python-expr 'raise RuntimeError("archy deliberate failure propagation check")'
```

This deliberate error exited 7, confirming script exceptions propagate.
Output is retained in `/tmp/archy-blender-failure.log`.
Blender reported 5.2.2 LTS, build `d13f752e3b9c`.
