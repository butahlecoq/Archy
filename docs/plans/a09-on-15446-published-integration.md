# A09 integration on 15446ee

The frozen A09/A15 candidate was reconciled with the A16 authority workspace and reviewed independently in `a09-a16-combined-15446-review-sol.md`. All 28 merged freeze entries match on this integration checkout, including the retained authority, MCP, and process fixture sources. The manifest contains both `authority` and `archy-service-gate`; the lock adds the gate and CLI-to-adapter edge without replacing the authority entry.

On the integration checkout, locked metadata, formatting, strict Clippy across adapters, CLI, MCP, gate and authority, adapter/gate/authority and accepted-worker tests, 32 packaging tests, live MCP generate/evaluate/compare and comparison tests, and the full schema script passed. Logs for packaging and schemas are `/home/butahlecoq/work/archy/.tools/a09-on-15446-packaging.log` and `/home/butahlecoq/work/archy/.tools/a09-on-15446-schema.log`. This schema run passed, while the previously reproduced MCP pipe flake remains under repair.

The CLI wrapper binds owned worker requests to verified accepted history. Public adapter entry points remain callable without that wrapper's provenance check. The fixed gate and bounded capture do not establish installed runtime execution, immutable artifact publication, final human approval or Omarchy acceptance. The package recipe does not yet provide the full runtime inventory.
