# A18 independent review — 29 October 2026

**Decision:** The retained demo is credible evidence for the partial core workflow, but A18 and release 0.1 are **not accepted**. The actual CLI has no `render`, `export-ifc`, or `approve`; no GLB, PNG, IFC, or human decision was produced. The two findings below concern the driver when those commands become available. I made no source, other-worktree, or retained-project changes.

## Findings

1. **High — false successful export on a lying backend.** `scripts/demo.py:107-119` checks only process exit status for render and IFC export. A temporary shim delegated all core operations to the existing binary, advertised both missing commands, and returned exit 0 with `{}` for each without creating files. Running the driver with `--export-variant variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b` exited **0** and printed `Draft export complete`; a recursive search found **zero** GLB, PNG, or IFC files. Before connecting production commands, validate typed receipts, run/variant identity, expected files, hashes, and parseability. The release spec §§3.2 and 7 require these checks. This is a forward-path defect; the shipped binary cannot reach it yet.
2. **Medium — malformed evaluation receipt escapes the controlled failure path.** `receipt()` (`scripts/demo.py:33-40`) validates only `run_id` and that `variant_ids` is a list. `main()` then indexes `all_required_checks_passed` at line 104 outside that validation. A temporary shim delegated to the existing binary but removed that field from its successful evaluation JSON. The demo exited **1** with a `KeyError` traceback rather than `DEMO STOPPED`/exit 2; both runs were retained. Validate required field types, IDs and cardinality, and the evaluation's relationship to the generated run before use. The set comparison at line 96 also hides duplicate IDs.

The subprocess uses an argument vector (`subprocess.run(command, ...)`), not a shell; the shim probes did not expose shell interpolation. The new-path check refuses existing directories and symlinks, while `open("x")` protects comparison copies. Existing tests cover those collisions using the real binary. A path race between the driver's check and CLI `init` remains governed by the CLI's own occupied-target refusal. `invoke()` has no timeout; production backend timeout and containment remain release gates, not evidence supplied by this demo.

## Commands and evidence

From this worktree, with `/home/butahlecoq/work/archy/.cargo-target-a18/debug/archy` as the executable:

```sh
/home/butahlecoq/work/archy/.cargo-target-a18/debug/archy --help
ARCHY_BIN=/home/butahlecoq/work/archy/.cargo-target-a18/debug/archy python3 -m unittest discover -s tests/demo -v
sha256sum scripts/demo.py tests/demo/test_demo.py README.md CONTRIBUTING.md docs/cli.md docs/plans/a18-demo-transcript.txt docs/plans/a18-demo-artifacts.sha256
cd /var/tmp/archy-a18-demo-20261029-final
sha256sum -c /home/butahlecoq/work/archy/.worktrees/a18-demo-20261029/docs/plans/a18-demo-artifacts.sha256
```

`--help` listed `init`, `validate`, `generate`, `evaluate`, `compare`, `doctor`, and read-only `desktop`, with no backend or approval command. All **4/4** demo tests passed. The seven source/evidence SHA-256 values exactly matched the handoff. `sha256sum -c` passed **32/32** entries; a separate file enumeration found exactly those 32 files, with no unlisted files. The retained generation and evaluation runs each contain three variant and three evaluation JSON files. The transcript records zero exits for all core commands and exit 2 at the missing render boundary.

Adversarial commands used disposable directories under `/tmp`: `python3 scripts/demo.py --archy <temporary-shim> --project-dir <new-project> --export-variant variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b` (shim delegates core commands to the existing binary and returns `{}`/exit 0 for render and export), and the same command without selection with a shim that removes `all_required_checks_passed` from the real evaluation output. The respective observed exits were 0 with no backend files, and 1 with `KeyError`.

README and `docs/cli.md` accurately describe the current partial boundary and do not call fixture workers supervised production output. The full release spec additionally requires three previews/geometry exports, a checked draft IFC, human review control, clean Omarchy operation, containment, and resilience evidence. None is established by this Ubuntu WSL demo.
