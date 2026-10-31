# Independent review: A15 first read slice plus A21 notice coverage

29 October 2026. **Scoped merge verdict: acceptable as the exact first-slice integration patch on `0cfcf916`, subject to the limits below.** I found no integration regression in the checked source, live MCP paths, or locked notice stage graph. This is not approval of full A15, final packaging, or Omarchy runtime.

## Identity and source comparison

I read `AGENTS.md`, `a15-a21-integration-handoff.md`, the original A15 `mcp-first-slice-handoff.md` and round-six review, and the A21 `mcp-notice-regression-review-sol.md` and its four source files. The target worktree HEAD is `0cfcf9164da20e6f22691b6ab92f305dc20fb679`, the specified base. `d356133b7311bb05104837be6a356ee3574fe581` (A09) is its ancestor. No source file was edited for this review.

`sha256sum /home/butahlecoq/work/archy/.tools/patches/a15-a21-on-a18-0cfcf916.patch` and `git diff --cached --binary | sha256sum` both produced `a2a89c2e4ae00883854399d80af090f415dff537dc2bc2fe9694447612809ac0`. `git apply --check --reverse` for that patch exited 0 in this worktree, establishing that the patch describes the 36 staged paths at this base. `git -c core.whitespace=cr-at-eol diff --cached --check` exited 0. The handoff and this review are untracked documents outside the patch. The default whitespace rule would flag the frozen CRLF lines in `Cargo.toml`.

The A15 freeze manifest SHA-256 is `eb22a4240529c03a7d37121d39ec23eabb984ee7ab898c7ba76329f8c7e55106`. I compared its 31 entries against both the original A15 worktree and this integration by SHA-256: 28 match byte for byte; the original matches the manifest in all 31 cases. The three intentional integrated differences are `crates/archy-core/src/storage.rs` `dbf53fd8b4ae0d870be30e3c837e0769765aacc5171ea3fa1aef49eee2c1dcfd`, `crates/archy-cli/src/lib.rs` `bc55331f81cefb5de9432c747016ff7674ca9dce9f87faec49f1a4ca02327916`, and `scripts/check-schemas.py` `068a243e55ea6e8c5988613957f9ffd6e988f7866a38d115006778a3e203fd15`. Integrated `Cargo.toml` and `Cargo.lock` retain the frozen A15 hashes `51f3cb607ef314327f30551d33c0d244c6c0e9fd8c07e76f4e7897bfe2db2c6` and `6866671914d12c1e0a39e3b72505be5056d24d1464823618c866096f2fc67fde`.

The A21 source worktree's ordered four-file `git diff --binary -- packaging/arch/notices.py packaging/arch/package.py tests/packaging/test_notices.py tests/packaging/test_package.py` hashes to `82983a8dba3288302dd96001b7b04e719ead16d2109e9156d601d14123231727`, as in Sol's review. Integrated file hashes match that review: `notices.py` `5c56fae3c62882c2090c29fa31fd8b1e778324f372ff9aaaf51c0db770a8dc5b`, `package.py` `8971fd67acc613c4a0fca715bc4ed7965bed96770788ee3fc9124642ea4eb856`, `test_notices.py` `9b9f14cf377f301f97584108d665df564514c1fcc870c42acf880d3a19d368a1`, and `test_package.py` `d66dbe19d80a9b4acec94d0203ad3269b17d20d20e938741be8d5b97906659b6`.

The shared-file reconciliation retains A09's storage implementation and A14's public `compare` export while adding `BoundProjectRead`; `desktop_status` uses the bound reader and its replacement tests remain. `scripts/check-schemas.py` still invokes A14's 50-case decision schema probe and A18's `tests/demo` suite, and now compares nine MCP input/output schema pairs and invokes both live stdio suites. The A09 ancestor and the unchanged base portions of storage and CLI were checked against `git show 0cfcf916:<path>`. This is source and selected behavior evidence, not a re-run of every upstream acceptance gate.

## Executed checks

All Cargo-backed commands used `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2` and a target under `/home/butahlecoq/work/archy/.tools/targets/`; Python used `PYTHONDONTWRITEBYTECODE=1` or `-B`. Commands were run from this integration worktree.

- `python3 -B -m unittest discover -s tests/packaging -q`, with `CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/targets/a15-a21-sol-review-20261029`: exit 0, **24 tests**, zero failures.
- `python3 -B crates/archy-mcp/tests/stdio_contract.py` and `python3 -B crates/archy-mcp/tests/parse_errors.py`, with the existing same-worktree target `/home/butahlecoq/work/archy/.tools/targets/a15-a21-integration-20261029`: both exit 0. The actual binary covered modern/legacy discovery, exact nine schemas, three verified reads, six typed `BACKEND_UNAVAILABLE` prerequisites, bounds, paging, stale cursor, corrupt/symlink history, source replacement, historical revision zero, parse errors, depth/framing, backpressure, EOF, and unchanged project inventories.
- `node crates/archy-mcp/tests/unicode_regex.js`: exit 0; eight Unicode fields.
- `python3 scripts/check-schemas.py`, with the unique review target above: exit 0. It ran A14's 50-case decision probe, compared all nine MCP input/output schema pairs with SDK descriptors, built fresh CLI and MCP binaries, reran both live stdio suites, and passed all six A18 demo tests. The remaining source schemas, fixtures, CLI outputs, doctor and desktop capability/status checks also passed as reported by the command. Doctor's target runtime acceptance remains deferred by that check.

`BoundProjectRead::open` pins one project source and root; `snapshot` checks root identity, source identity and source bytes before reading accepted history. The server checks the bound project ID, limits concurrent reads to four, and routes six unfinished tools to typed prerequisites. The live tests verify passive behavior for the exercised paths, including source replacement and corrupt history; they do not establish immunity to every concurrent in-place mutation or prove a real Omarchy client.

## Locked notice graph and stage probes

The shared `normal_registry_graph` in `package.py` is used by both notice admission and stage graph calculation. Stage computes offline, locked Cargo metadata filtered to `x86_64-unknown-linux-gnu`, traverses normal dependencies from `archy-cli`, and compares the inventory's unique `(name, version, source_sha256)` triples to that graph and `Cargo.lock` before destination writes. It also compares the supplied bundle digest to the inventory and copies the captured validated bytes.

I ran `python3 -B packaging/arch/notices.py --source . --vendor /tmp/archy-core-arch-source/vendor --output-dir /home/butahlecoq/work/archy/.tools/targets/a15-a21-sol-review-20261029/stale-admission`: exit 1 at absent `chrono-0.4.45`, with zero output files. The same command using `/home/butahlecoq/work/archy/.tools/targets/a15-a21-integration-20261029-vendor` and `/home/butahlecoq/work/archy/.tools/targets/a15-a21-sol-review-20261029/fresh-admission` exited 0. Independent reading of the fresh inventory, lock and vendor checksum manifests found 91 unique crate rows, 169 vendor notice texts, and zero source/text hash mismatches. This did not reauthenticate seven primary texts against upstream Git or the three Rust standard-library texts.

I invoked the real `packaging/arch/package.py stage` twice with the exact arguments below, substituting `old89`/`fresh91` for `LABEL` and the corresponding retained/admitted path for `PAIR`. The CLI binary was built from this integration worktree in the retained target. The Python probe used `subprocess.run` with these arguments and checked exit status, destination files, inventory, all staged modes and hashes, and both notice bytes.

```sh
REVIEW_TARGET=/home/butahlecoq/work/archy/.tools/targets/a15-a21-sol-review-20261029
PAIR=/home/butahlecoq/work/archy/.tools/targets/a21-packaging-notices-admitted # old89
# PAIR=$REVIEW_TARGET/fresh-admission # fresh91
LABEL=old89
python3 -B packaging/arch/package.py stage --source . \
  --binary /home/butahlecoq/work/archy/.tools/targets/a15-a21-integration-20261029/debug/archy \
  --manifest packaging/arch/files.txt --dest "$REVIEW_TARGET/stage-$LABEL" \
  --inventory "$REVIEW_TARGET/stage-$LABEL.json" \
  --notices-file "$PAIR/THIRD-PARTY-NOTICES.txt" \
  --notice-inventory-file "$PAIR/third-party-notices.json"
```

The retained old pair has 89 rows, inventory SHA-256 `e4d95486832a64364e38fe4b565369ad8d075b6529edbf4d84973e69110cffd6` and bundle SHA-256 `ea7019a4ff8e69a9b0d49c4e54ab5869c1c2a0726997c7c9e30cd425402da710`. Stage exited 1 with `notice dependency coverage mismatch`, zero destination files, and no stage inventory. The fresh pair has 91 rows, inventory SHA-256 `215d1f061bfd35362cdd0ffaf7a596c434d1e3bd30355f62bac7a249efaa07bd` and bundle SHA-256 `bb7333062a0d0885d0cc4bde6370374d72760fe515f0a2513cce79fc2dd09184`. Stage exited 0 and wrote 19 manifest paths. Every staged path's mode and SHA-256 matched the stage inventory, and both staged notice files matched the admitted pair byte for byte.

The stage graph check establishes locked Linux dependency-row coverage. Stage does not independently authenticate notice content: a substituted bundle with a corresponding inventory digest and unchanged crate triples can pass, as the A21 Sol review demonstrated. The frozen first slice also inherits the pinned SDK's parseable-invalid-request `-32600` response without an `id` member; the syntax adapter's explicit `id:null` applies to `-32700` parse errors. These are limits to carry into follow-on review.

This verdict applies only to the exact patch and base identified above. Live compare, the separate packaging gate patch, full A15/approval behavior, Arch `makepkg` or pacman, licensing sufficiency, and Omarchy runtime remain separate work. Re-run the locked graph, notice admission, stage and MCP checks after any lockfile or overlapping-source merge.
