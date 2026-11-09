# Independent A10 render publication review

Date: 2026-10-30. Reviewed the uncommitted candidate at `ff7f6bc` in `.worktrees/a10-render-publication-20261029`. Read `AGENTS.md`, the release 0.1 specification, A10 plan/handoff, prior A09 supervisor reviews, and prior A10 fixture reviews. I changed no source, other worktree, installed file, commit, or remote state. Disposable reproductions and the unique Cargo target are under `.tools/targets/a10-render-publication-review-sol`.

## Verdict

**Do not accept A10 publication or the release gate yet.** The frozen reader fixture and fail-closed unavailable-gate path work, but two independently reproduced semantic/integration defects remain. No installed supervised success or Omarchy acceptance was observed.

### P1 — a render run breaks the advertised desktop status reader

`desktop_status.rs` `summary()` accepts only `generate`, `edit`, and `evaluate` (lines 135–140), while `desktop_status()` verifies and summarizes every receipt in its page (lines 195–211). Once `render` commits a valid failed run, both default status and `--run` on that run return `ARTIFACT_INVALID: unsupported run operation`. This prevents the advertised `project_status`/`run_status` reader from inspecting the newest project state. The omission is in this A10 candidate even though the same worktree exports `render` from the CLI.

Reproduction in `semantic-probe/`: run `archy generate --count 2 --seed 42`, `archy evaluate --all`, then `archy render --all` with `/usr/lib/archy/archy-service-gate` absent. Generation, evaluation, and render exit 0; the render record contains two `BACKEND_UNAVAILABLE` results. `archy desktop status --project project.json --run <render-id> --json` exits 1 with the unsupported-operation error **before any tampering**. The same default status path reaches `summary()` for the render receipt. Add render to the typed status operation/schema and report render outcome separately from design checks, then rerun status/schema contracts with a render head.

### P1 — consistent local rehash can erase a requested `--all` result on replay

`render.rs` binds `{"selector":"all"}` into the request digest, but `verify_render_run()` (`run_integrity.rs` lines 405–620) checks only that each *recorded* variant belongs to the evaluated parent. It does not reconstruct the selector from retained request bytes or require all evaluated variants for an `all` request. `RunManifest` retains `request_sha256`, not the selector bytes. The replay path therefore cannot prove that the accepted result inventory still corresponds to the accepted request.

Concrete reproduction: the direct CLI run above created two failed render variants. In the disposable project, I removed the second item from `render.json`, its outcome and artifact entries from `manifest.json`, recomputed the `render.json` artifact hash and manifest receipt hash in revision 3, and left the original request digest/idempotency key unchanged. Calling `archy render --all --run <evaluation-id> --idempotency-key <original-key> --expected-project-revision 2 --json` exited **0** and returned the original run ID with **one** variant. This is a semantic forgery after local rehash, not a claim that an unauthenticated local hash chain resists a user who can edit all project files. The important gap is that replay says the `all` result is valid even when the stored result omits a selected variant. Persist canonical selector bytes or another independently checkable selector binding, then compare the exact selected variant IDs and order with source evaluation on replay. Exercise both successful and failed variants.

The mutation can be repeated on a disposable two-variant render run as follows (the third revision is the render):

```python
import hashlib, json
from pathlib import Path
root = Path("<disposable-project-directory>")
run = root / "runs" / "<render-run-id>"
def compact(value): return json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode()
def sha(value): return hashlib.sha256(value).hexdigest()
record_path = run / "render.json"
record = json.loads(record_path.read_bytes())
removed = record["variants"].pop()
record_bytes = compact(record)
record_path.write_bytes(record_bytes)
manifest_path = run / "manifest.json"
manifest = json.loads(manifest_path.read_bytes())
manifest["outcomes"] = [x for x in manifest["outcomes"] if x["variant_id"] != removed["variant_id"]]
manifest["artifacts"] = [x for x in manifest["artifacts"] if removed["variant_id"] not in x["path"]]
for item in manifest["artifacts"]:
    if item["path"] == "render.json":
        item["sha256"], item["byte_len"] = sha(record_bytes), len(record_bytes)
manifest_bytes = compact(manifest)
manifest_path.write_bytes(manifest_bytes)
revision_path = root / ".archy/revisions/00000000000000000003.json"
revision = json.loads(revision_path.read_bytes())
revision["receipt"]["manifest_sha256"] = sha(manifest_bytes)
revision_path.write_bytes(compact(revision))
```

### P2 — zero exit status for an entirely failed render needs an explicit contract

In the same reproduction, the installed gate was absent, both variants were `failed` with `BACKEND_UNAVAILABLE`, and `archy render --json` exited **0**. `render()` returns an `Ok(RenderReceipt)` after persisting failures and `main.rs` maps all `Ok` values to exit success. Persisting a failed immutable run is correct, and the JSON does not claim `verified`; however, shell callers using exit status alone see success without any rendered output. Existing generation tests deliberately use zero exit for failed *design checks*, so this may be an intended command convention. Document and test the render-specific convention or return a distinct nonzero result while retaining the receipt. Do not present this as a verified render success.

## Positive evidence and limits

The 37 paths in `docs/plans/a10-render-publication-freeze.sha256` all passed `sha256sum -c` before and after review. The eight real fixture files also passed their separate SHA manifest before and after review. The direct Blender 4.5.15 LTS worker reproduction used the frozen variant and its original digest with a new request ID. It exited 0 and produced five files matching its response hashes. Its `model.glb` was byte identical to the fixture (`8711ea16…`); the three new PNGs differed in bytes but Pillow decoded each as 512×512 RGBA, and trimesh loaded three GLB geometry nodes. This directly exercises the worker, not the installed A09 gate or an accepted render publication. The fixture response binds the variant-file digest `916627…`, whereas current accepted render requests bind the project input digest; the frozen response cannot be used as an unmodified positive service capture.

Source inspection found fail-closed checks for a missing gate, fixed service argv and type-bound request, gate identity and cgroup policy before GO, teardown before scratch return, `openat2` capture with symlink/hardlink/nonregular refusal and size/hash checks, exact five-file success inventory, and a six-entry published inventory including response. The reader caps PNG input at 4 MiB, uses PNG decoder allocation limits and a fixed 512×512 RGB/RGBA decoded frame; GLB input is capped at 8 MiB with profile restrictions, transformed coordinates, sealed oriented edges, area/volume/bounds checks, and stored-byte reparse on verified replay. These are code and test observations. They do not prove that every valid parsed preview depicts the GLB, that `scene.blend` matches the GLB, or that the installed gate enforces its policy.

The accepted worker tests establish that the request is built from a verified historical run snapshot even after the live project source changes. Rendering an explicit old evaluation likewise binds its stored input digest; it does not assert that the old run matches the current project file. The render path has no approval write, and its verified/failed result is distinct from `required_checks_passed`. The focused test covers one idempotent failure replay and source-manifest preservation; it does not fault-inject render commit recovery, positive service capture, or mixed per-variant outcomes.

Parser risk remaining: GLB JSON is materialized as `serde_json::Value` and parsed again by `gltf` before the aggregate vertex/index bounds apply. The 8 MiB file cap limits source bytes, but no process-level peak-memory bound or adversarial near-cap JSON allocation test was shown. The geometry check counts a mesh primitive as one storey solid and verifies manifold edges, roof area and volume, but does not prove a single connected solid or exact roof coverage. A mesh containing a void plus a compensating overlapping closed component could preserve those aggregates and footprint corners; this is an analytical counterexample, **not a constructed passing fixture**. Add adversarial GLBs for duplicate components, holes/overlaps, transforms, and oversized JSON before claiming full semantic parity. The PNG foreground test only establishes nonblank decoded previews.

The stored failure branch retains a worker-response SHA string without the response bytes; replay validates its hex syntax and the recorded error artifact, not the historical worker error response. This does not create a verified render, but limits diagnostic provenance. Unknown extra physical files under an already accepted run are also outside the listed-artifact inventory check; the API exposes only listed artifacts, so this is an inventory/immutability limit rather than a demonstrated read spoof.

## Checks and release gates

- `cargo test --locked --offline -p archy-cli -p archy-adapters -p archy-service-gate -- --skip edit_history_beyond_sixty_four_runs_stays_readable_and_replayable`: exit 0 in the unique Cargo target. It includes the real fixture decode/corruption test, unavailable-gate publication/replay test, adapter capture tests, and gate unit tests. The 64-run history stress test was excluded; the pinned IFC/native systemd tests stayed ignored.
- `python3 -m unittest discover -s tests/packaging -p 'test_*.py' -v` with bytecode disabled: exit 0, 32 tests. `cargo metadata --offline --locked` through `locked_graph()` found `png 0.18.1`, `gltf 1.4.1`, `gltf-json 1.4.1`, `gltf-derive 1.4.1`, `fdeflate 0.3.7`, and `flate2 1.1.10` in the 106-crate normal CLI/gate registry graph. Synthetic tests reject missing/stale notice coverage. This worktree has no `vendor/`, so a real complete notice bundle and installed root-owned inventory were **not** produced.
- `cargo clippy --locked --offline -p archy-cli -p archy-adapters -p archy-service-gate --all-targets -- -D warnings`: exit 0 in the same unique target.
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-schemas.py` with the isolated Cargo environment: exit 0. Generated schemas and serializers, nine MCP contracts/live probes, 26 negative CLI fixtures, and six demo tests passed. Its desktop probe covers genesis, not a render run; this pass does not contradict the status defect above.
- `cargo fmt --all -- --check`, `git diff --check`, and both SHA manifest checks: exit 0.

`/usr/lib/archy/archy-service-gate` and `/usr/bin/blender` are absent on this WSL2 host; the local pinned Blender binary is not an installed policy gate. No positive service launch, cgroup teardown, root-owned runtime inventory, Omarchy run, or visual acceptance was demonstrated. Prior A09 round-three review likewise left the no-observed-cgroup path unverified. The A15/A16 integration head `15446ee` overlaps this candidate in `Cargo.toml`, `Cargo.lock`, `archy-cli/src/lib.rs`, and `project_read.rs`, and changes `archy-core/src/storage.rs`; this review did not merge or validate their combined semantics. Reconcile those files and rerun render replay, desktop status, notice, schema, and target service gates before integration acceptance.
