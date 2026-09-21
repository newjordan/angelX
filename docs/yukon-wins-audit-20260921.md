# AngelX field results on Yukon — audit of the wins
From: toymaker.claude.angelx-film · 2026-09-21 · for Atlas

Source: `yukon submissions <benchmark> --all --json` over the 12 benchmarks listed on 2026-09-21.
Solver handle: `newjordan`. A "record" is a promoted submission, i.e. the board's new best when it landed.
The director states these were set with the AngelX harness.

## Totals
- 142 records across 8 benchmarks (2,603 submissions).

## First places
| Benchmark | Field | Records | Result |
|---|---|---|---|
| eigenlabs/flock-challenge-multi/x86 | systems / Rust optimization | 65 | most records on the board (21 record-setters); holds the board best, 1,651,266.98, set 2026-09-16 |
| eigenlabs/flock-challenge | systems / Rust optimization | 44 | most records on the board (43 record-setters, 70 solvers) |
| davidtai/qwen38-125b-a6b-cuda-v1 | LLM inference speed (Qwen 3.8 125B, CUDA) | 5 | took first place five times, 12–15 Sep (2.18x to 2.50x); fastest validated run 2.753x on 21 Sep |
| proximity-prize irs-reduction-threshold-upper | Lean / proof bounds | 5 | most records on the board |

## Other records
- gpsanant/ecdsafail-challenge (quantum ECDSA point-addition circuit, 136 solvers): 19 records; moved the qubit-Toffoli score from 2.23B to 1.60B during June 2026.
- eigenlabs/eip8200-challenges/ripemd160: 2 records. quantum-safe-bitcoin pinning: 1. irs-reduction-threshold-lower: 1.

## How the director wants this used
- The figures above are background. Marketing uses evergreen wording with no board names, ranks, dates or exact numbers, and the boards are not re-pulled.
- Do not claim an all-time rank: active competition stopped about three weeks ago; recent activity is harness testing.
- Approved direction for public wording:
  - "Field proven. 100+ records on public research leaderboards."
  - "First-place results in kernel optimization, LLM inference, and cryptography research."
  - Plate form: `FIELD PROVEN · kernel · inference · cryptography research` / `RECORDS · 100+ on public leaderboards`
- Closed benchmarks are not in the CLI listing, so the true totals may be higher.
