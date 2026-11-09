# Rust toolchain revalidation

Date: 29 October 2026. Baseline: `docs/specs/release-0.1.txt`.

## Scope and environment

Revalidated the existing Rust CLI/core workspace using Ubuntu 26.04 LTS under WSL2, Linux kernel `6.6.87.2-microsoft-standard-WSL2`, x86_64. The Windows Rust executables remain subject to Windows Application Control; no security policy was changed. Ubuntu compilation is development evidence only. The user deferred actual Omarchy execution, so this does not establish target Omarchy or release acceptance.

The existing WSL `stable` toolchain was checked first: Cargo 1.95.0 and rustc 1.95.0, with rustfmt/clippy installed. This is older than the workspace `rust-version = "1.99"`. Running Cargo in the repository activated its existing `rust-toolchain.toml` pin and successfully installed Rust 1.99.1 with cargo, rustc, rust-std, rustfmt, and clippy. No pin or global configuration was changed.

- `cargo 1.99.1 (5f94df478 2026-08-27)`
- `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
- Linker/compiler available at `/usr/bin/cc`.
- Build artifacts: `/mnt/c/work/archy/.tools/cargo-target`, outside the repository.

## Commands and evidence

Commands ran in `/mnt/c/work/archy/repository` through `wsl -d Ubuntu -- bash -lc`. Build/test/clippy/run commands set `CARGO_TARGET_DIR=/mnt/c/work/archy/.tools/cargo-target`.

| Command | Result |
| --- | --- |
| `cargo generate-lockfile` | Exit 0; resolved 52 packages and generated the previously absent `Cargo.lock`. |
| `cargo build --locked --workspace` | Exit 0; both `archy-core` and `archy-cli` compiled. |
| `cargo test --locked --workspace` | Exit 1; CLI integration tests: 2 passed; core validation integration tests: 6 passed, 1 failed. Unit test binaries contained 0 tests. Execution stopped at the failing integration target. |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Exit 0; no warnings. |
| `cargo fmt --all -- --check` | Exit 1; reports formatting differences across existing Rust source/test files. No formatting edits were applied. |
| `cargo run --locked --quiet -p archy-cli -- validate --project examples/courtyard-housing/project.yaml --json` | Exit 0; returned normalized SI project JSON with 20 by 35 m site, 3 storeys, 3.2 m floor height, 900 m² target, 12 m maximum height, and 3 m setback. |

## Current blockers

`translated_rectangle_coordinates_are_preserved` fails at `crates/archy-core/tests/validation.rs:40`: actual first point `[0.0, 0.0]`, expected `[10.0, -5.0]`. Inspection confirms the included YAML example has 24 CRLF endings and zero LF-only endings. The test uses a literal LF-only multi-line `replace("width_m: 20\n  depth_m: 35", ...)`, which is absent from these file bytes; therefore the replacement is a no-op. This is a test portability defect, not evidence that the translated polygon input reached the validator. No product/test source was edited in this revalidation task.

The formatter check is a separate outstanding gate. Clippy and successful compilation do not imply formatting or test success. Both failures must be resolved and their commands rerun before claiming the workspace checks pass.

`Cargo.lock` is newly generated and ready for review/addition by the coordinating agent; it was not committed or staged here. Existing CI also requires the lockfile to be tracked. No Blender, IfcOpenShell, Omarchy execution, complete golden journey, or release acceptance was attempted.

## Handoff

Changed files: newly generated `Cargo.lock` and this report only. Next steps: fix the newline-sensitive test and apply formatting within an authorized implementation task, rerun workspace tests/format/clippy, and preserve target Omarchy/backend acceptance as a separate deferred gate.
