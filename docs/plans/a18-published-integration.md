# A18 partial demo integration on ae6e6a6

The reviewed driver and six tests were copied byte for byte from the independently reviewed snapshot. README and CLI documentation were updated to describe the actual stop at the unreviewed backend boundary; the existing CI schema-check step builds the CLI and runs the six demo tests. Frozen source hashes and the original real-run inventory are recorded in the integration handoff.

The combined integration checkout built `archy-cli` with its own SSD target and passed all six tests against that binary. The separate integration reviewer reproduced the demo build and test commands in a clean copy and exercised malformed receipts, absent artifacts, existing paths, symlinks, and comparison-file collisions. Both runs preserve accepted project history and return a controlled unavailable result at the render boundary.

No supervised render, IFC export, human decision, target Omarchy acceptance, or A18 completion is claimed.

The reviewed integration initially added a workflow step. The GitHub token lacks `workflow` scope, so that change was not pushed. Instead, the existing schema-check step invokes the same six tests against the already built CLI using Cargo's reported target directory. The earlier integration handoff and review describe their original snapshot, including that superseded workflow change.
