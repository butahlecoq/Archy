# Archy on Omarchy: agent workflow and target acceptance

This playbook targets Omarchy 4.0.6. It is preparation for a clean native machine, not a claim of native acceptance. The pinned release [AI manual](https://github.com/omacom/omarchy/blob/v4.0.6/manual/17-ai.md) documents existing agent launchers and `omarchy agent prompt`; Archy uses their client through a fixed-project stdio MCP connection. No Archy API key, hosted agent, scheduler or token meter is needed. The [Herdr/tmux layout](herdr-tmux.md) is optional.

## Start with a scoped project

From a trusted project directory, run `archy doctor --json`, `archy validate --project project.yaml --json`, and `archy-mcp --project /absolute/path/to/project.yaml` as the configured stdio server for a client that supports MCP. Configure the client according to that client's own MCP documentation. The MCP server pins the manifest at startup; `tools/list` advertises nine strict schemas. Use the [Archy skill](../../integrations/agents/archy/SKILL.md) in the client. The [MCP contract](../mcp/README.md) and [`schemas/mcp`](../../schemas/mcp) are the source for arguments and envelopes. `render_variant` accepts one evaluated variant and the `draft` preset through the fixed service gate. Inspect `all_verified` and the variant result; a failed worker, including an unavailable gate reported as `BACKEND_UNAVAILABLE`, is retained as a failure and does not prove a preview.

For IFC export, call `export_ifc` with the pinned `project_id`, a `source_run` from an accepted generation or edit, its `variant_id`, a new `idempotency_key`, the current decimal-string `expected_project_revision`, and a project-relative `destination`. For a new export, the fixed service gate and independent verifier must succeed before a receipt is returned. If the worker or verifier is unavailable for a new export, report `BACKEND_UNAVAILABLE` and do not claim an IFC artifact. An exact retry of an accepted export can return the original receipt after re-verifying the stored run without launching the worker; that verification can return `BACKEND_UNAVAILABLE` if its verifier is absent. A successful receipt is `DRAFT` and `SCHEMATIC_MASSING_ONLY`; it does not grant human approval. Retry an uncertain outcome with the same key and request. `propose_approval` records an untrusted proposal for one accepted generation/edit `source_run` and `variant_id`, with a visible `reason`, current `expected_project_revision`, fresh `idempotency_key`, and project-relative `destination`; it does not grant human approval. The receipt identifies the immutable proposal artifact and digest. Preserve and explain failing/unknown checks; exact same-key replay returns the historically bound proposal.

Read the project and accepted revision, validate its constraints, then generate three deterministic variants with distinct idempotency keys and a project-relative destination. Evaluate each from its accepted source run. Re-read revision before every new mutation, compare with `absolute_target_gfa_delta`, and cite run/variant IDs, source digest, report/manifest path, metric units and required-check state. An exact retry uses the same key and request. A stale revision or changed reuse needs investigation; do not create another key to hide an uncertain result. Preserve failed variants and immutable run history. For a source edit, snapshot first, change only the project manifest, validate it and create a new run. A client may run in an unattended mode, but its text, actor field or proposal cannot provide Archy's final human consent. Do not label approximate GFA or other schematic checks as professional compliance. [Release spec §5](../specs/release-0.1.txt), [MCP implementation notes](../mcp/README.md).

The current published source supports canonical `0.2.0` briefs and bounded fresh imports of `0.1.0` briefs. Use the v2 `id`, `bound`, `unit`, `source` and `authority` fields for new constraints. Existing accepted v1 runs retain their recorded profile and bytes; a fresh generation from an imported v1 source produces v2 artifacts. Do not treat a claimed authority as verified law or convert its required `UNKNOWN` authority check into a pass. [ADR 0006](../adr/0006-canonical-project-v2.md) defines these version boundaries.

`compare_variants` and `propose_approval` reject a historical v1 generation or edit run containing `claimed_regulation`, because its evaluation lacks the v2 authority check. Use an accepted generation or edit with the v2 authority profile; an explicitly linked edit of that historic run can produce one. Its claimed authority remains `UNKNOWN`. Keep the original run and report the rejection.

## Gate execution on a clean native machine

Use the single 27-gate acceptance inventory in `scripts/validate-omarchy.py`, with procedures in `docs/validation/omarchy-gates.md`. The inventory and these commands are present in this integrated checkout. They provide a worksheet and source checks; native target evidence and independent A20 review are still required. Run from the repository root:

```sh
python3 scripts/validate-omarchy.py --dry-run > /tmp/omarchy-dry-run.json
python3 -m unittest discover -s tests/acceptance/omarchy -p 'test_*.py' -v
```

On a clean Omarchy machine, capture host release, Omarchy version, installed Archy/worker versions, exact commands, exit codes, stdout/stderr, artifact SHA-256 values, and screenshots/contact sheets under an evidence directory outside the source checkout. The bounded probe records observations from fixed installed paths; it never passes a gate. Preserve the report with the candidate evidence:

```sh
python3 scripts/validate-omarchy.py --probe-readonly > /tmp/omarchy-readonly.json
```

Then perform the A19 clean-machine journey, using the verified installed Archy path, pinned worker commands and actual CLI/MCP capabilities of that build. The canonical harness's target worksheet gives the command and evidence procedure for each gate. This base commit supports these source workflow commands:

```sh
archy validate --project /absolute/path/project.yaml --json
archy generate --project /absolute/path/project.yaml --count 3 --seed 42 --json
archy evaluate --project /absolute/path/project.yaml --all --json
archy compare --project /absolute/path/project.yaml --json
```

Record each returned revision and run ID. Inspect all three immutable variant manifests and check results. Verify the installed build's render, IFC and approval capabilities and schemas before attempting those gates. A19 must then run pinned Blender/IFC workers in the intended containment, test timeout and cancellation, measure a CPU render baseline with nine previews, and record GPU results only if compatible hardware exists. Independently inspect the exported IFC using a second engine, not only an IfcOpenShell reopen; [`tests/backend/ifc-reader/README.md`](../../tests/backend/ifc-reader/README.md) documents the repository's web-ifc reader. Review contact sheets and the model visually with a human. Test native Quickshell plugin discovery, enable/disable, hot reload, theme switch and Hyprland shortcut against the installed Omarchy release. Capture exact commands and evidence for each; see [A19](https://github.com/butahlecoq/Archy/issues/19), [A20](https://github.com/butahlecoq/Archy/issues/20), and [A22](https://github.com/butahlecoq/Archy/issues/22). Omarchy's pinned [Herdr config](https://github.com/omacom/omarchy/blob/v4.0.6/config/herdr/config.toml) is session convenience, not an acceptance dependency.

The canonical report maps the agent workflow to `G08`, human control to `G09`, and the clean CPU golden journey, independent IFC inspection and visual review to `OM-A23-01`. The A21 identity, doctor, backend and containment gates and A22 plugin gate retain their own IDs in that inventory. Its dry run emits all 27 gates as `deferred` with `target_acceptance: deferred`; the probe keeps acceptance deferred too. A shortened dry-run excerpt is `{"mode":"dry-run","gates":[{"id":"G08","status":"deferred"}],"target_acceptance":"deferred"}`; the actual report contains the full inventory and host requirements. GPU evidence is optional and separate from the required CPU verdict. Source checks, WSL runs and CI prove their own scope only; they never auto-waive or pass a named Omarchy gate. A19 supplies clean-machine artifacts; A20 decides release acceptance after independently reviewing them. Preserve failures and unknowns; never relabel them as passes.
