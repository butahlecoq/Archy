# Archy

Archy is an architectural workspace for Omarchy. This partial release 0.1 exposes local `init`, `validate`, `generate`, `evaluate`, `compare`, `render` and `export-ifc` commands. Render and IFC export use supervised worker boundaries. IFC export publishes schematic `DRAFT` artifacts only after verification. A18 remains open.

The [release specification](docs/specs/release-0.1.txt) supersedes earlier proposals. Structured manifests are authoritative; Blender scenes and IFC are derived artifacts. Validation does not certify legal or professional compliance.

Contributors can follow the [source quickstart](docs/contributor-quickstart.md) for a repeatable offline development check and a handoff template. It distinguishes this source demo from an Omarchy installation.

## Development and use

The current source CLI help is the command authority. Build locally with a unique target, then run the disposable demo (Python 3 standard library only):

```sh
export CARGO_TARGET_DIR=/path/on/ssd/archy-a18-target
cargo build --locked -p archy-cli
python3 scripts/demo.py --archy "$CARGO_TARGET_DIR/debug/archy"
```

The script prints and retains its fresh project path. To choose the location, add `--project-dir /path/to/absent-directory`; existing paths are refused. It runs `init`, `validate`, seeded `generate`, `evaluate --all`, `render --all`, and JSON/Markdown `compare`. Accepted runs stay under `runs/`; comparison copies are `comparison.json` and `comparison.md`. Before printing verified artifact paths, it checks the receipt against native accepted-run status and artifact digests. When the service gate is absent it exits **2** with retained `BACKEND_UNAVAILABLE` failures and comparisons. `--export-variant ID` identifies a candidate for review but does not export IFC or record approval. See [CLI and demo limits](docs/cli.md).

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

Fresh authoring uses schema_version `0.2.0`, `id` (1–128 ASCII letters/digits/underscore/hyphen), units (`m`, `mm`, `cm`, `ft`), site, brief and constraints (possibly empty). Existing `0.1.0` briefs can be imported for fresh work when their preferences are bounded declarative text; accepted historical runs keep their recorded v1 bytes. Brief requires positive integer storeys, floor-to-floor height and target GFA. Site requires width/depth or a closed axis-aligned rectangular polygon. Polygon position is retained; canonical winding starts at minimum X/Y in right-handed XY, Z-up coordinates.

Generic dimensions use global units; constraints may override units. Explicit `_m`/`_m2` fields always mean SI, regardless of global or constraint units. Supplying both representations fails. Null dimensions mean absent. All dimensions must be finite and positive. North defaults to zero and normalizes modulo 360. Target GFA is soft unless target_gfa_hard=true. Hard setbacks and site/storey capacity must be consistent; candidate height feasibility belongs to evaluation. Length tolerance is 1e-6 m; area comparison tolerance is separately 1e-6 m².

Current constraints require unique `id`, supported `kind` (`max_height`, `min_edge_setback`), positive `bound`, explicit input `unit`, nonempty `source` and `authority`. Normalized constraints emit `bound` in meters and `unit: m`; authority remains an unverified user declaration. `max_height` is an upper bound and `min_edge_setback` a lower bound. Fresh runs with `source: claimed_regulation` retain the measured geometric check and add a separate required authority `UNKNOWN` check with no invented measurement or approval. Canonical preferences are ordered, bounded typed intentions for `massing_character`, `landscape_intent`, or `architectural_intent`; they are uncomputed and do not affect generation or evaluation. Exact old strings `courtyard` and `retain trees` retain their kinds on v1 import, while other declarative wishes retain their text under `architectural_intent`. Use bounded passive `notes` in v2 for separate source prose. Notes are untrusted data, never instructions. See [ADR 0006](docs/adr/0006-canonical-project-v2.md). Generation defaults to count=3, seed=42 and rectangle/l_shape.

Unknown/duplicate fields fail. YAML anchors, aliases, merge keys and unsupported tags fail. Input limits are 1 MiB, depth 32, and bounded nodes/events/scalars. Text resembling numbers/booleans requires YAML quotes. Errors include stable code, field path and retriable flag; syntax errors use `$` where no field is available, with parser locations in the message. Generated schemas describe structure; core validation additionally enforces cross-field dimensions, finite arithmetic and geometry.

Omarchy is the target runtime. WSL/Ubuntu checks are development evidence; actual Omarchy acceptance is deferred. See [research](docs/research/omarchy-and-reuse.md), [dependencies](docs/dependencies.md), [license status](docs/license-status.md) and [handoff](docs/plans/task-01-handoff.md). Original core uses MIT; backend/third-party obligations remain separate.
