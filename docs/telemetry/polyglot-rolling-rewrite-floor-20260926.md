# Rolling rewrite floor — 2026-09-26

## Finding (from the rust30 wire cohort)

Per-call wire logs (`artifacts/polyglot-loop-2026-09-26/rust30-default/run/wire/*.jsonl`,
204 model calls over 30 solved Rust tasks) show the provider prefix cache is
dropped on a fixed cadence, not randomly. Every task that reached a 12th model
call re-billed a large block of previously cached tokens at exactly that hop:

| task | call index | cached before | cached after | re-billed tokens |
| --- | ---: | ---: | ---: | ---: |
| ws/forth | 11 | 12,800 | 6,656 | 6,144 |
| ws/react | 11 | 11,776 | 7,680 | 4,096 |
| ws/xorcism | 11 | 10,752 | 6,656 | 4,096 |
| ws/two-bucket | 3 | 3,584 | 0 | 3,584 |
| ws/forth | 23 | 18,944 | 6,656 | 12,288 |

Total: 30,208 re-billed tokens, 7.9% of the run's 381,594 uncached input.
The cadence matches `ANGEL_ROLLING_REWRITE_HOPS` default 12 (rust30 used the
default env), and hop 23 of `ws/forth` is the second 12-hop boundary. The three
hop-12 tasks are exactly the three biggest per-task call-count regressions
versus the historical cell (rust-xorcism +7, rust-bowling +5, rust-forth +9).

The rewrite is *bounded*: cached input never fell below ~6.6k tokens after a
drop, i.e. the flushed passes (tool-argument shrink / inspection dedup / aging)
rewrite only the middle of the history, not the system prompt.

## Change

`cockpit/src/agent/harness/turn/mod.rs` gates the rolling cadence flush on a
new body-size floor, `ANGEL_ROLLING_REWRITE_MIN_TOKENS` (default 24,000
estimated tokens, capped at one sixth of the live compaction budget so a
small-window seat still flushes before its body balloons). Below the floor the
held rewrites stay held, so a cache-warm prefix is never traded for savings the
request body does not need. Compaction, prune and history-splice breakers still
flush immediately — the floor only suppresses the *cadence* flush.
`ANGEL_ROLLING_REWRITE_MIN_TOKENS=0` restores the old behavior.

The aging probe fixture (`tests/cockpit/harness/tests__tool_aging.rs`) pins the
floor off explicitly so it keeps exercising the cadence mechanism on its short
synthetic history.

## Verification (final, after the commit-squash rebuild)

- `cargo test --release --no-default-features --bin angel cache_stable` →
  **10 passed / 0 failed** (includes the rolling-boundary probe).
- `cargo test --release --no-default-features --bin angel tool_aging` →
  **42 passed / 0 failed**.
- `cargo test --release --no-default-features --bin angel harness::turn` →
  **36 passed / 0 failed**.
- `cargo test --release --no-default-features --bin angel compaction::` →
  **44 passed / 0 failed**.
- `cargo build --release --bin angel` → clean
  (sha256 2a9829148c2ae8064ca77866af01b738456ebab578edeb3a8c9446b510189532).
- Earlier green run before the comment consolidation:
  `context::` → 6 passed / 0 failed.

## Effect hypothesis (unmeasured)

On the rust30 shape, hop-12/24 drops cost ~30k re-billed input tokens
concentrated in the longest tasks. With the 24k-token floor those tasks
(hist_tok stays well under 24k through 24 calls in this cohort) would keep a
monotone cache prefix. A rerun of the rust30 slice should show: no
cached-input drops at hops 12/24, and reduced uncached input on
xorcism/react/forth. Not yet measured with a model-backed cell.
