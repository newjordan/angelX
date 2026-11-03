# Muse Pareto baseline on current dev — 2026-10-26

Lane 1 of `docs/META-HACKATHON-2026.md`: a fresh, small, paired-ready
baseline for Muse Spark in Angel on the hackathon branch's base,
before any optimization candidate. Small on purpose (16 of 136 tasks):
this pins the method and the order of magnitude cheaply; a full-136
run is for a candidate that first wins this subset.

## Protocol

- Binary: release `angel` built from a clean detached worktree of
  `be287282d` (private `dev`, the hackathon branch's base), bound with
  `ANGEL_BUILD_SOURCE_SHA256=2fa9489766ede01e59ecedf1413a5139950bc42712d12a3bf1f2c3fe3f8c8bb2`
  (`scripts/check/cockpit-source-digest.sh`). `--build-info` reads the
  same digest back. A binary built from a dirty tree reports `unbound`
  and the harness refuses it — measured runs are source-pinned.
- Cell: `scripts/bench/polyglot/run-polyglot-muse-low.sh`, Prime
  Verifiers 0.3.1, `muse-spark-1.3`, reasoning low, temperature 1.0,
  600 s wall, seed 0, one attempt, fresh HOME. Kit:
  `~/angel_tests/angelX-bench/polyglot-20260921`.
- Tasks: first 4 per language of `tasks-polyglot-v1.json`
  (`tasks-subset16.json` beside it): js-affine-cipher, js-alphametics,
  js-beer-song, js-binary, py-affine-cipher, py-beer-song, py-book-store,
  py-bottle-song, rust-accumulate, rust-acronym, rust-alphametics,
  rust-book-store, cpp-all-your-base, cpp-allergies, cpp-bank-account,
  cpp-binary-search-tree.
- Summary: `scripts/bench/polyglot/summarize-polyglot-loop.py` on the run
  dir. Run: `runs/muse/angelx/seed0-20261026T182122Z`.

## Result

| Metric (16 tasks) | Value |
|---|---:|
| Solved | **16 / 16** (usage complete 16/16) |
| Task wall (sum / median) | 174.7 s / 7.82 s |
| Model time (sum) | 126.9 s |
| Model calls | 68 (4.25 per task) |
| Tool calls | 55 |
| Input tokens | 689,646 |
| Cached input tokens | 512,884 (**74.4 % cached**) |
| Output tokens | 26,411 |
| Reasoning tokens | 10,942 (~161 per call) |

Per-solved-task: 43.1k input tokens, 10.9 s wall. The cached share is
in line with the 2026-09-30 full-set run's 76 % (`muse-hermes-h2h`),
so the prompt-cache fix is holding on current dev.

![Input tokens per task against median wall per task: angelX with Muse Spark on 2026-09-23, 2026-09-30 and 2026-10-26, Muse Code with Muse Spark, and the September DeepSeek harness comparison. The October point sits below-left of both September angelX points.](../images/bench/muse-pareto-20261026.png)

Chart: `scripts/bench/charts/muse_pareto.py` (points transcribed only
from the receipts named on the chart).

Dollars are deliberately not headline here: September's $0.21 was
contributor-billing actuals; converting these tokens at list prices
would invent a number no bill shows. Tokens, calls and wall are the
currency-free Pareto axes; the receipts keep the raw counts.

## Negative: effort `none` through this runner

A paired `MUSE_EFFORT=none` cell on the same binary and subset scored
0/16 in 4.6 s total with zero model time and zero errors: each rollout
"completes" in ~0.3 s without a model call. `minimal` is not in the
Verifiers effort enum at all. The mechanism, read in the code: this
runner's sampling is imposed by the interception proxy from the
Verifiers-level effort, and the same value is exported into Angel's
seat env. Angel's Meta seat accepts
`minimal|low|medium|high|xhigh` (`META_REASONING_LEVELS`,
`cockpit/src/agent/club/discovery.rs`) and its calibrated default for
Muse Spark is already `minimal` (`docs/telemetry/model-calibration.toml`,
2026-09-30) — which this runner overrides to low. So the baseline
above is Angel *forced up* to low, and minimal cannot be expressed
through this runner today; measuring it needs the calibration runner
(the September night cells) or a runner change decoupling the Angel
seat env from the proxy's imposed sampling, then a committed clean
tree to stay promotable. Recorded as Lane 1's first candidate, not
silently dropped. (Baseline reasoning averaged ~161 tokens/call
against calibration's 409 at low — this alphabetical subset is much
easier than September's 12 slowest tasks; do not read an effort
conclusion into that gap.)

## Next candidates (paired against this sheet)

1. Minimal-effort seat via the runner fix above (Wonder list item 4).
2. A Muse Wonder row: same subset, Wonder at low (Wonder list item 1)
   — needs the operator's word for the spend and a `muse-wonder` cell.
3. Cache-share guard: alert when cached share drops per cell
   (Wonder list item 3); this sheet's 74.4 % is the reference.
