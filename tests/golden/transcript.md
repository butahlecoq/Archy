# A05 live CLI capture transcript

Capture executed on Linux 6.6.87.2-microsoft-standard-WSL2 x86_64 with `rustc 1.99.1 (58cf8f9c3 2026-10-13)` and Cargo 1.99.1. The locally built `target/debug/archy` binary had SHA-256 `e14029acd0ccee02a426e128ab0501822f525245549546907b8000628c0fbb32`. `cargo build --locked -p archy-cli` exited 0 before capture. Source SHA-256 was `dbd0c3debe37cd7c725903d3492c991ae3451bf0ae8832be41c97b23f1a2b358`.

Two fresh temporary directories each received an exact byte copy of `examples/courtyard-housing/project.yaml`. The first was `/tmp/archy-a05-golden-aq3d35iq/first`; the second was `/tmp/archy-a05-golden-aq3d35iq/second`. Each command below used the built binary, the displayed arguments, `--project <directory>/project.yaml`, and `--json`. Exit status was 0 for all six commands; the complete parsed stdout receipts are in `evidence.json` under `capture_receipts`.

| Order | Directory | Command arguments | Revision | Run ID | Result |
| --- | --- | --- | ---: | --- | --- |
| 1 | first | `generate --count 3 --seed 42` | 1 | `c434a0b2-f7ea-47bb-9af1-fdfda414d906` | Three variants; all required geometric checks passed. |
| 2 | second | `generate --count 3 --seed 42` | 1 | `df8add66-435e-4652-948f-a48b9574255b` | Same three variant IDs and exact variant/evaluation/generation JSON bytes as first. |
| 3 | first | `evaluate --variant variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b --run c434a0b2-f7ea-47bb-9af1-fdfda414d906` | 2 | `8c9dfb50-2d67-4cd0-b667-0bc128a2cb3d` | Explicit one variant selection from immutable first run. |
| 4 | first | `evaluate --all` | 3 | `71f3dfdc-b88c-4d9d-9027-411efa54d380` | Default source resolved to first generation run; three evaluations. |
| 5 | first | `generate --from-run c434a0b2-f7ea-47bb-9af1-fdfda414d906 --variant variant-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b --params edit.json` | 4 | `62040078-5b31-46af-95f6-07205fdac442` | Width increased by 0.25 m; derived variant ID changed to `variant-82d8c3944ac04207169ab11c979643cefb53c150893f3414af32ff68918725ee`. |
| 6 | first | `generate --from-run 62040078-5b31-46af-95f6-07205fdac442 --variant variant-82d8c3944ac04207169ab11c979643cefb53c150893f3414af32ff68918725ee --params restore.json` | 5 | `860e2341-e139-44b1-a49d-2a5e2ceedd23` | Restored original parameters and canonical variant ID in a new run. |

The second generation used a distinct execution run ID, despite byte identical deterministic design and evaluation artifacts. The first project's five manifests, every artifact digest and length, and the second run's manifest inventory are in `evidence.json`. The first run's files still matched its manifest SHA inventory after evaluation, edit and restoration. The saved accepted project contains the source, revision links, manifests, inputs, normalized data, generation output, variants, evaluations, parameters, full reports and logs for all five revisions. All three generated variants passed the declared geometric checks; the report itself states that professional certification, daylight, cost and other analyses are uncomputed.

This transcript records actual execution IDs and measured output from this one capture. It does not assert that later runs reproduce UUIDs, idempotency keys, request digests, manifest digests, revision links or report bytes containing those IDs. The Cargo golden test verifies the saved chain through `verify_run` and tests the deterministic and execution dependent parts separately.
