# CI escaped pipeholder integration

GitHub Actions PR run 40945971854 failed the escaped-session pipeholder test because both captured pipes reached EOF. The exact scheduling path in that run was not recorded. Independent local probes reproduced two plausible ordering problems: delayed startup consumed the capture deadline, and delaying the escaped child's session setup allowed the original group to be signaled too early.

The revised test fixture gates the capture deadline until the escaped child has entered its own session and the direct group leader is known to have exited without being reaped. The independent second review approved this scope after forced startup delays, cleanup failures and parallel load probes. The implementation is test-only.

On integration HEAD 8e36e11, the frozen process.rs hash was a519321c480723e09eb2e417c5ac10c7d231373c5159ff0407653c31d66197ee. Workspace formatting, strict Clippy for adapters and MCP, both package tests, and scripts/check-schemas.py passed in a combined run that also included a provisional MCP test change. That provisional change was separately rejected and is not included in this commit. GitHub results for this commit remain to be checked; a separate MCP backpressure test flake is under repair.
