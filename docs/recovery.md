# Recover an interrupted local run

Archy accepts a run only when its numbered revision record is published. A failed or killed writer can leave an unaccepted directory under `.archy/staging`. A run directory published before its revision can also remain as an orphan. The store retains these directories for diagnosis; it does not remove them during retry.

To inspect accepted history, use the public `ProjectStore::read_history` API or `archy desktop status --project PROJECT --json`. The reader checks the accepted revision chain and artifact hashes. It does not treat staging as accepted. Keep the project file, `.archy`, and `runs` together when backing up or moving a project.

If a mutating call reported an uncertain commit, retry the **same operation** with the **same idempotency key, expected revision, source input, and destination**. For CLI generation, an example is:

```sh
archy generate --project project.yaml --count 3 --seed 42 \
  --idempotency-key example-run --expected-project-revision 1 --json
```

Use the original key and arguments in an actual recovery. If a revision already exists for that request, Archy returns its original receipt. If a complete run was published without its revision, Archy verifies and accepts that orphan once. If the process stopped during staging, a retry creates a fresh staged run; the incomplete directory remains for diagnosis. A changed request or a new key with an old expected revision returns `REVISION_CONFLICT`. If accepted history fails verification, stop mutation and preserve the files for investigation. Do not edit a numbered revision or accepted run to force progress.

The recovery integration test in `crates/archy-core/tests/recovery_staging.rs` kills a child process after it writes a staged artifact. It checks that the earlier accepted manifest, artifact, and revision hashes are unchanged, then retries and observes one new revision. The retry uses the same request but writes different artifact bytes, so this test does not establish deterministic artifact equality. The test covers process termination on this Linux host. It does not test sudden power loss, network filesystem durability, backend cancellation, or MCP retries.

The same test file also covers a lost receipt after commit: a child commits a second revision, aborts before delivering its receipt to the parent, and the parent reopens the project. An identical retry returns the accepted receipt without adding a revision or changing accepted bytes. This tests a completed commit followed by process death; it does not place the death inside the revision publication or sync sequence.

The edit interruption test in `crates/archy-cli/tests/edit_interruption.rs` stages an edit through the public store API, kills the writer before commit, and retries that exact edit through the CLI. The CLI status reader reports only the earlier accepted run after the kill. The retry publishes one child edit; identical CLI replay returns its receipt; a later edit restores the original variant parameters while keeping prior accepted files intact. Its fault matrix and complete captured hash inventory are in `docs/plans/a17-edit-interruption-c72.md`. The interrupted writer stages through the core API, so this does not test cancellation inside CLI artifact publication.

The IFC export unit regression injects a worker cancellation after empty staging begins. It checks that the error returns the generated idempotency key and expected revision, marks the attempt retriable, removes empty staging, and preserves accepted bytes and revision. This is a local runner test; it does not exercise the installed service or IfcOpenShell.

If the test runner itself is killed, its private `archy-recovery-staging-*` temporary directory can remain. The writer exits when its parent channel closes. A surviving test or the operator who created an isolated temporary root can inspect and remove that owned root after the writer stops. Archy does not run a global cleanup service for these fixtures.
