# ADR 0003: Pinned Blender runtime for the bounded worker

Accepted for WSL development on 2026-10-29. The candidate is Blender 4.5.15
LTS Linux x64 from the official archive, with SHA-256 recorded in
`workers/blender/versions.json`. The bundled Python interpreter and bundled
glTF operator run the trusted worker. A separate GPL-3.0-or-later license file
applies to the bpy worker; Blender's own distribution notices remain distinct.

Rust must validate canonical variant identity and evaluation, map the logical
destination to private staging, launch a fixed argv, enforce OS containment and
process limits, then independently validate and publish outputs. The Python
worker performs structural/finite checks and constructs derived geometry. Its
response and hashes are evidence to verify, not authority. WSL smoke evidence
does not close the Omarchy acceptance gate or A09 artifact acceptance.
