# Archy

Archy is an architectural workspace for Omarchy. This partial release 0.1 exposes local `init`, `validate`, `generate`, `evaluate` and `compare` commands. Blender and IFC workers exist as separate components, but no production supervised `render` or `export-ifc` CLI command is available yet. The one-command demo stops at that boundary and A18 remains open.

The [release specification](docs/specs/release-0.1.txt) supersedes earlier proposals. Structured manifests are authoritative; Blender scenes and IFC are derived artifacts. Validation does not certify legal or professional compliance.

Contributors can follow the [source quickstart](docs/contributor-quickstart.md) for a repeatable offline development check and a handoff template. It distinguishes this source demo from an Omarchy installation.

## Development and use

The current source CLI help is the command authority. Build locally with a unique target, then run the disposable demo (Python 3 standard library only):

```sh
export CARGO_TARGET_DIR=/path/on/ssd/archy-a18-target
cargo build --locked -p archy-cli
python3 scripts/demo.py --archy "$CARGO_TARGET_DIR/debug/archy"
```

The script prints and retains its fresh project path. To choose the location, add `--project-dir /path/to/absent-directory`; existing paths are refused. It runs real `init`, `validate`, seeded `generate`, `evaluate --all`, and `compare` JSON/Markdown. Accepted runs stay under `runs/`; comparison copies are `comparison.json` and `comparison.md`. It exits **2** after comparison, with the project intact: this CLI lacks `render`, and the driver has no reviewed backend receipt or artifact contract. `--export-variant ID` records selection intent but does not export. This script never records human approval. See [CLI and demo limits](docs/cli.md) and [A18 integration evidence](docs/plans/a18-demo-integration-handoff.md).

Install pinned Rust 1.99.1 with rustfmt/clippy, then:

```sh
cargo build --locked --workspace
cargo run --locked -p archy-cli -- init --name courtyard-demo --dir ./courtyard-demo
cargo run --locked -p archy-cli -- validate --project ./courtyard-demo/project.yaml --json
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python -m pip install jsonschema==4.26.0
python scripts/check-schemas.py
```

`init` requires an absent target with an existing parent. It creates project.yaml and a local schema, refusing existing files, directories or symlinks. A write failure can leave a partial new directory for inspection; recovery never removes user files. `validate --project` accepts a YAML/JSON manifest file, not a directory. `--json` emits one normalized object or error envelope on stdout; failures exit nonzero. Source bytes remain unchanged.

## Input contract

Required fields: schema_version (`0.1.0`), project_id (1–128 ASCII letters/digits/underscore/hyphen), units (`m`, `mm`, `cm`, `ft`), site, brief and constraints (possibly empty). Brief requires positive integer storeys, floor-to-floor height and target GFA. Site requires width/depth or a closed axis-aligned rectangular polygon. Polygon position is retained; canonical winding starts at minimum X/Y in right-handed XY, Z-up coordinates.

Generic dimensions use global units; constraints may override units. Explicit `_m`/`_m2` fields always mean SI, regardless of global or constraint units. Supplying both representations fails. Null dimensions mean absent. All dimensions must be finite and positive. North defaults to zero and normalizes modulo 360. Target GFA is soft unless target_gfa_hard=true. Hard setbacks and site/storey capacity must be consistent; candidate height feasibility belongs to evaluation. Length tolerance is 1e-6 m; area comparison tolerance is separately 1e-6 m².

Constraints require unique id, supported kind (`max_height`, `min_edge_setback`), positive dimension and nonempty source. Missing authority becomes `assumption`; supplied authority remains an unverified user declaration. Optional `preferences` is a string list preserved without invented metrics. Generation defaults to count=3, seed=42 and rectangle/l_shape; this slice validates declarations only.

Unknown/duplicate fields fail. YAML anchors, aliases, merge keys and unsupported tags fail. Input limits are 1 MiB, depth 32, and bounded nodes/events/scalars. Text resembling numbers/booleans requires YAML quotes. Errors include stable code, field path and retriable flag; syntax errors use `$` where no field is available, with parser locations in the message. Generated schemas describe structure; core validation additionally enforces cross-field dimensions, finite arithmetic and geometry.

Omarchy is the target runtime. WSL/Ubuntu checks are development evidence; actual Omarchy acceptance is deferred. See [research](docs/research/omarchy-and-reuse.md), [dependencies](docs/dependencies.md), [license status](docs/license-status.md) and [handoff](docs/plans/task-01-handoff.md). Original core uses MIT; backend/third-party obligations remain separate.
