# A15 read-only comparison integration on f1e2477

The comparison follow-on patch has SHA-256 `1eaeb6591e80624992dbd67049baebd5053a75ff0d3bddbf607c0074eddaa9b2`. All 11 frozen source paths matched the integration checkout and independent Sol review. The original feature review and merged integration review both passed their live protocol checks.

The combined integration checkout passed workspace formatting, strict CLI and MCP Clippy, CLI comparison and MCP tests, the schema and live stdio suites, 50 decision-contract cases, six demo cases, and 24 packaging tests. Exact output is retained outside this tree in `.tools/a15-compare-on-f1-root.log`. The integration author also passed full workspace tests and reproduced notice admission against the locked graph.

`compare_variants` returns verified comparison documents with rankings within each selected generation/edit run. It does not rank across runs. It is a fixed-project read and conveys no mutation or approval authority. Generation, evaluation, rendering, export and proposal MCP operations remain open.
