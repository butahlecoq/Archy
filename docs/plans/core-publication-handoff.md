# Reviewed workflow publication checkpoint

This publication adds persistent generate/evaluate/edit/restore, semantic run verification, JSON/Markdown reports and authentic five-revision golden evidence; nonmutating doctor and desktop readers; typed backend protocols and bounded artifact capture; optional pinned Blender/IFC workers and an independent IFC reader; package tooling and a native doctor panel. MCP and the production service/quota supervisor are separate work streams.

The isolated source tree was assembled from the scoped independent reviews named in the adjacent handoffs. The artifact error-provenance fix and observable-mutation fixture correction passed their independent rereviews. Native panel completion passed a separate source and real-component review on Quickshell 0.3.2 / Qt 6.12. The publication tree includes the Python protocol modules required by the Rust protocol regression.

Observed checks on this publication tree with Rust 1.99.1:

- Workspace format and strict locked all-targets Clippy passed.
- Locked workspace tests: 140 passed, zero failed, one optional native-backend integration test ignored.
- Fourteen source-derived schemas, actual serializers, 26 CLI negative fixtures, doctor and desktop probes passed.
- Package preservation/recipe tests: ten passed. Mise contract tests: nine passed.
- Panel model tests: eight passed. Python plugin contracts: four passed. The independent panel review records the real-component valid 0/1 test; the author's handoff records five native matrix tests and two real-CLI tests.

Exact local logs: `/tmp/archy-core-publication-gates-round4.log` and `/tmp/archy-core-publication-final-checks.log`. Earlier attempts exposed a full 3.9 GiB `/tmp` tmpfs, a timestamp assumption in a fixture, and omitted Python protocol source. Those were corrected before publication; failed observations remain in their earlier logs and handoffs. Generated build artifacts now use the workspace SSD.

Diff hygiene passed with generated golden `report.md` files and the verbatim Blender worker license excluded: Markdown's deliberate two-space line breaks and the preserved license EOF are not rewritten, because the golden bytes and hashes are acceptance evidence. No test, lint or schema gate was weakened.

Actual Omarchy install, desktop/theme/GPU, clean-target and filesystem acceptance remain deferred. Artifact capture proves byte consistency only and requires caller-proved whole-writer teardown; metadata alone cannot detect every same-tick write. The Arch package build is development evidence and its artifact remains private pending notice/license audit. There is no main merge, release, human design approval, production worker launch or completed-MCP claim in this checkpoint.
