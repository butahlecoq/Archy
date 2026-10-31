# Lost receipt integration

The independently reviewed receipt-loss test was merged from `a5a61c8` onto
`7728da0`. Its test file also received the later recovery test race fixes in
`7b53b10`; a three-way merge preserved both sets of changes. The reviewed
source hash matched before merging. The merged test file has SHA-256
`a8bf8dd7321f42d8cb45aa45a6a65bb102adbbea9f506d5278392dbd7434359e`.

On the combined tree, the recovery suite passed all 11 tests, followed by 50
parallel runs. Targeted strict Clippy, formatting, and diff checks passed.

The child aborts after `commit()` returns and before delivering its receipt.
This exercises replay after a lost receipt; it does not prove interruption
inside publication, power-loss durability, backend cancellation, or Omarchy
acceptance. See `a17-recovery-receipt-loss-review.md` for independent evidence.
