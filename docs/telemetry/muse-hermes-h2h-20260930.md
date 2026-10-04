# Muse and Hermes head-to-heads, 2026-09-30

Both run the full polyglot-v1 set: 136 tasks (48 JS, 34 Python, 30 Rust, 24 C++). They use the same Prime Verifiers 0.3.1 cell as the sol, Grok, DeepSeek and GLM head-to-heads: one attempt per task, a 600 s wall clock, a fresh HOME for each rollout, grading however the agent exits, and `TMPDIR=/work/tmp`.

## Muse: angelX 0.1.91 vs Muse Code

- **angelX:** pin `angel-0.1.91-ee7c626-09c6d9e7`, in the kit's Muse cell at minimal on the contributor tier. Muse gets its own reasoning back every hop and sends a prompt cache key. Both fixes are on `dev` as d4c000d and 776e374.
- **Muse Code 1.4.2-R4684.1,** as it ships: `muse exec --json --yolo`, at its shipped effort (high) with its own delegation, on `muse-spark-1.3-contributor`. Its usage comes from its own session logs.
- Both arms ran side by side from 02:17Z.

| | angelX 0.1.91 | Muse Code |
|---|---:|---:|
| solved | **136/136** | **136/136** |
| agent time | **50.8 min** | 165.7 min |
| median task | **18.6 s** | 58.9 s |
| p90 task | **39.6 s** | 129.3 s |
| model calls | **748** (5.5/task) | 1075 (7.9/task) |
| input tokens | **6.70 M** | 25.86 M |
| uncached input | **1.64 M** | 4.47 M |
| cached share | 76% | 83% |
| output tokens | **196k** | 715k |
| cost, contributor list price | **$0.21** | $0.63 |

- **Headline:** both solved all 136. angelX used 69% less agent time, 74% fewer input tokens and a third of the cost. It was faster on 130 of 136 tasks and in every language.

| language | angelX: solved / min / input M | Muse Code |
|---|---:|---:|
| C++ | 24/24 / **10.4** / **1.13** | 24/24 / 39.7 / 6.36 |
| JavaScript | 48/48 / **16.5** / **2.54** | 48/48 / 45.0 / 8.29 |
| Python | 34/34 / **12.0** / **1.67** | 34/34 / 34.5 / 5.19 |
| Rust | 30/30 / **11.9** / **1.36** | 30/30 / 46.6 / 6.03 |

- **Parked arms, not in the result:**
  - c6793df stopped after 51 tasks; it had no reasoning replay.
  - 325cdf9 stopped after 12 tasks; it had replay but no cache key.
- **Mulligan:** Muse Code was at 109/136 when systemd-oomd killed the bench window at 22:42 CDT. py-tree-building was in flight, so it was rerun from #110 at 22:54 CDT.
  - Muse Code's sheet spans `seed0-20261001T011112Z` (1–109) and `seed0-20261001T035433Z` (110–136).
  - `full-report.md` and `full.json` in `/work/artifacts/musecode-h2h-20260930/` hold the full numbers.

## DeepSeek: Hermes Agent v0.18.2

- **Hermes Agent v0.18.2** on DeepSeek V4.1 Flash, in the kit's DeepSeek cell (`run-cell.sh deepseek hermes`), through the same interception proxy as angelX.
- **angelX:** the hints-on c6793df sheet from the dsh head-to-head (`dsh-h2h-hints/deepseek/angelx/merged-c6793df`). It ran earlier the same evening, not side by side. Hermes ran from 21:36 to 23:15 CDT.

| | angelX 0.1.91 | Hermes v0.18.2 |
|---|---:|---:|
| solved | **136/136** | 132/136 |
| agent time | **25.7 min** | 71.2 min |
| median task | **8.8 s** | 25.1 s |
| model calls | **737** (5.4/task) | 1729 (12.7/task) |
| input tokens | **7.28 M** | 26.92 M |
| uncached input | **1.38 M** | 2.43 M |
| cached share | 81% | 91% |
| output tokens | **187k** | 421k |
| cost, off-peak list price | **$0.34** | $0.69 |
| cost, peak list price | **$0.67** | $1.38 |

- **Headline:** angelX solved 136 to Hermes's 132. It used 64% less agent time, 73% fewer input tokens and about half the cost.

| language | angelX: solved / min / input M | Hermes |
|---|---:|---:|
| C++ | 24/24 / **5.8** / **0.99** | 24/24 / 14.9 / 3.30 |
| JavaScript | 48/48 / **9.0** / **2.84** | 48/48 / 13.5 / 4.24 |
| Python | 34/34 / **5.5** / **1.75** | 32/34 / 25.7 / 11.04 |
| Rust | 30/30 / **5.5** / **1.70** | 28/30 / 17.1 / 8.33 |

- **Misses:** all four are real; integrity passed, and the tests failed.
  - rust-bowling, py-transpose and py-wordy each stopped at exactly 91 calls, which looks like Hermes's shipped turn budget.
  - py-transpose ended on a change Hermes said it had not verified.
  - py-wordy described its fix without applying it.
  - rust-doubly-linked-list failed after 20 calls.
- **Mulligan:** task #105, rust-variable-length-quantity, was in flight when systemd-oomd killed the bench window at 22:42 CDT. It reran first and solved in 29 s.
  - Free swap fell about 1.4 GB/s from 22:38 in that window, a minute after the task started. It's the likely runaway, but the clean rerun leaves that unproven.
  - Resumed cells now run in their own `systemd-run` scope with `MemoryMax=24G`.
  - The merged sheet is `hermes-deepseek/deepseek/hermes/merged-20261001/`, and its `SOURCES.txt` records which run each task came from.
  - `full-report.md` and `full.json` in `/work/artifacts/hermes-deepseek-20261001/` hold the full numbers.
