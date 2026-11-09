# Release backlog publication handoff — 29 October 2026

Scope: reconcile and publish the release backlog; no product changes, issue closure, merge or release publication. This worker did not change repository settings; the coordinator configured main protection separately and its settings were read back below.

## Decisions and changes

- GitHub authentication works with ADMIN permission. The earlier authentication blocker is historical. The founder selected MIT for original core code; the license file and separate worker/dependency distribution review remain A01/A20 obligations.
- A01–A20 retain the full specification. Refined field-level schemas and preference preservation, SI alias rules, immutable edited iterations, evaluation before rendering, dimensioned tolerances, protected-main evidence and benchmark observations.
- A21 covers doctor/mise/Arch packaging; A22 covers native Quickshell and reversible user configuration; A23 covers provider-neutral skills, optional Herdr/tmux and the deferred Omarchy harness. A20 depends on these platform tasks as well as A01–A19.
- R01–R03 separately track Month 2, Month 3 and later scope. All explicitly exclude themselves from release 0.1 and cannot replace required acceptance.
- `scripts/sync-backlog-issues.py` defaults to read-only dry run. Apply uses durable body-marker identity with title fallback across open/closed issues, refuses conflicting/duplicate IDs before writes, creates missing labels/issues, reconciles bodies via `--body-file`, links dependencies, and verifies remote identities/titles/bodies/labels before writing the issue map. It never closes/reopens issues or changes repository rules. Independent review caught title-only identity before commit; marker matching and conflict rejection now cover renamed titles.

## Validation and evidence

- Initial `gh issue list --repo butahlecoq/Archy --state all --limit 100 --json number,title,url,labels` returned `[]`.
- Initial `gh api repos/butahlecoq/Archy/branches/main/protection` returned HTTP 404, `Branch not protected`; rulesets count was `0`. After coordinator configuration, a fresh protection API read verified strict required `rust` status check (GitHub Actions app 15368), one approving review, stale-review dismissal, administrator enforcement and disabled force pushes/deletions. Reverify required checks when final CI evolves; source planning does not establish CI passage.
- `python -m py_compile scripts/sync-backlog-issues.py` passed.
- Initial `python scripts/sync-backlog-issues.py` identified exactly 26 creates: A01–A23 and R01–R03, with no existing stable IDs.
- The first apply created all issues but final body verification exposed CRLF/LF normalization and an A01 edit made during publication. Body comparisons now normalize line endings; a reconciliation rerun updates A01 without duplicating issues.
- Final `python scripts/sync-backlog-issues.py --apply` passed: verified all 26 remote identities, titles, bodies and label sets and wrote [issue-map.md](issue-map.md). A following dry run returned `create: []`, with all 26 IDs reconciled. Release A01–A23 are issues #1–#23; future R01–R03 are #24–#26. All are open.
- This evidence concerns tracker content, not product tests, CI status or implementation acceptance.

## Remaining gates and next action

All authored tracker files are scoped for commit and push on `feat/release-0.1`. Product files being edited concurrently are excluded from this tracker commit. Independent review inspects backlog coverage, remote issue contents and dependency/identity behavior; no implementation issue is closed by publication.

Continue bounded implementation in dependency order. Real Omarchy installation, desktop behavior, isolation, GPU/visual and independent IFC acceptance remain deferred until a target machine exists. Windows/WSL/CI checks do not waive those gates. Final required-check configuration, dependency notices, exact backend pins and approval policy evidence remain owning-ticket responsibilities.
