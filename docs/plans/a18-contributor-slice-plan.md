# A18 contributor handoff slice plan — 30 October 2026

Base: published `1e2313fe0bf20f69f246836334adb761276d2390`. Issue #18 and `docs/specs/release-0.1.txt` were read before this plan.

## Scope

Give a new contributor one source-development path from a locked build to the existing partial demo, an exact environment/version record, and a session handoff template. Link it from the contributor entry point. Keep all runtime, package, desktop, IFC, authority, renderer, and existing demo-driver code untouched. This is a partial A18 slice; it does not establish installation or release acceptance.

## Checks and risks

- Run an offline locked CLI build, the six existing real-binary demo tests, and a fresh one-command demo. Verify its expected exit 2, retained comparison and runs, and absent render/IFC outputs.
- Check documented commands and links against the published tree; inspect the final diff for unvalidated installation claims and scope leaks.
- Risk: WSL evidence can be mistaken for Omarchy acceptance. State environment and deferred gates next to the commands and in the handoff template.
- Risk: a developer may treat demo exit 2 as a failure of generation or as approval. Explain the exact completed steps and the explicit unavailable boundary.

No new dependency. Existing six behavior tests already cover the demo and collisions; this documentation-only slice adds no implementation test that merely mirrors prose. Independent review remains required after patch freeze.
