# Blender development worker

This GPL-3.0-or-later Python worker accepts one bounded `blender-worker-v1`
request. `protocol.py` uses only the standard library. `worker.py` must run
inside the pinned Blender archive in `versions.json`. A trusted caller supplies
absolute request and response file paths and a fresh private output directory.
`output_dir` in the request is a logical project-relative destination only; the
worker writes fixed basenames beside the response file. The worker does not
implement filesystem isolation, process cancellation, quotas or publication.
Blender stores mesh coordinates and transforms at float32 precision. The worker
checks every constructed world vertex against the Task02 1e-6 m budget before
export; coordinates this runtime cannot represent return an explicit
`BACKEND_UNAVAILABLE` capacity error without changing the source variant.

Run the stdlib validation tests:

```sh
python3 -m unittest discover -s tests/backend/blender -p 'test_protocol.py' -v
```

Run real WSL integration with `ARCHY_BLENDER_BIN` set to the verified archive's
`blender` executable:

```sh
ARCHY_BLENDER_BIN=/absolute/path/to/blender python3 -m unittest discover -s tests/backend/blender -p 'test_blender_integration.py' -v
```

`--disable-autoexec` and `--offline-mode` are Blender flags, not an OS sandbox.
The generated `.blend` is trusted derived output. Neither output checks nor a
successful render establish design feasibility, professional compliance or
approval. Omarchy execution and production A09 orchestration remain separate.
