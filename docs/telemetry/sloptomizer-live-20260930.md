# Live Sloptomizer pilot — 2026-09-30

The local stress test found and fixed a signal gap: alternating failed receipts
did not trigger identical-evidence advice. The projection now keeps a separate
consecutive-verdict count, preserving every original receipt. Sparse `⚠⡫⠛`
notices explicitly allow changing failures to represent progress. Hypothesis,
expectation, model, or verdict changes reset this count. These notices introduce
no stop, forced pivot, model call, or tool restriction.

## Local measurements

Four sequential localhost runs used the frozen debug binary, a fresh arithmetic
fixture per run, and the existing scripted model with 20 alternating failure
cycles. Order: off/on, then on/off. All four completed with 63 requests, 62 tool
calls, 20 typed failed checks, and one final typed passing check. The scripted
model cannot respond to the advice, so these measure delivery and overhead only.

| Arm | Round 1 task ms | Round 2 task ms | Total request bytes, both rounds | Final request bytes |
| --- | ---: | ---: | ---: | ---: |
| Live memory off | 1,645 | 1,698 | 3,680,520 | 47,004 |
| Live memory on | 1,700 | 1,740 | 3,859,050 | 49,301 |

Median task time increased by 48.5 ms (2.9%); transmitted request bytes increased
by 4.9%. Each enabled run delivered one context card and three failure-run
notices; the disabled runs delivered none. Request count was unchanged. With
only two observations per arm, these timings do not establish a reliable latency
effect. Bytes are measured request payloads, not provider tokens or billed cost.

Between-request accounting has no recorded response-completion timestamps in
this stub. Its approximately 0.54-second harness subtotal uses the explicitly
labelled instantaneous-stub assumption; it is not observed provider latency.

The initial frozen candidate produced only one context card despite 20 failing
checks. Its earlier measurements are retained separately; they are not pooled
with the corrected candidate.

## GLM Flash comparison

All 24 planned live attempts completed after explicit operator approval, with
12/12 passing in each arm. Every attempt has a valid grading-integrity receipt;
there were no task timeouts, nonzero agent exits, missing scores, or trace
errors. All four evaluators exited normally. No attempt was excluded or rerun.

The enabled arm used fewer calls and tokens in this cohort. These are
descriptive observations, not demonstrated causal benefits of the advice:
five enabled attempts received no card, results varied substantially between
rounds, and the six selected tasks all reached the correctness ceiling.

| Metric, both rounds combined | Live memory off | Live memory on | On versus off |
| --- | ---: | ---: | ---: |
| Final grader passes | 12/12 | 12/12 | unchanged |
| Model calls | 140 | 117 | −16.4% |
| Tool calls | 137 | 118 | −13.9% |
| Input tokens, including cached input | 1,287,692 | 987,457 | −23.3% |
| Cached input tokens, a subset of input | 1,132,608 | 853,312 | −24.7% |
| Derived uncached input tokens | 155,084 | 134,145 | −13.5% |
| Output tokens, including reasoning | 40,542 | 27,908 | −31.2% |
| Reasoning tokens, a subset of output | 18,196 | 9,536 | −47.6% |
| Sum of task wall seconds | 1,906.1 | 1,091.5 | −42.7% |
| Attempts receiving a live-memory card | 0/12 | 7/12 | delivery observed |

Input, cached-input, output, reasoning, and timing coverage is complete for all
24 task traces. Uncached input is derived as input minus cache-read only because
all receipts declare included-cache accounting; the provider does not supply
that field directly here. Cached input and reasoning are subsets, not
additional tokens to add to the totals. These are usage measurements, not a
billed-cost estimate. Task wall totals use the recorded agent intervals and
exclude the separate final-grading intervals.

The matched first round used 74 off / 69 on calls and 964.6 off / 600.7 on task
seconds. The second round, executed on then off, used 66 off / 48 on calls and
941.5 off / 490.8 on task seconds. Both pairs have the same recorded contract
apart from the intended live-memory flag and isolated wire-output path.

| Task | Round 1 model calls, off → on | Round 2 model calls, off → on | On cards, round 1 / round 2 |
| --- | ---: | ---: | ---: |
| `js-forth` | 7 → 6 | 23 → 5 | 0 / 0 |
| `py-rest-api` | 8 → 5 | 14 → 11 | 0 / 1 |
| `rust-poker` | 25 → 16 | 4 → 5 | 2 / 1 |
| `cpp-space-age` | 8 → 5 | 5 → 5 | 0 / 0 |
| `py-react` | 10 → 20 | 15 → 10 | 1 / 1 |
| `py-transpose` | 16 → 17 | 5 → 12 | 3 / 3 |

The per-task regressions matter: React used 25 off versus 30 on calls in total;
transpose used 21 off versus 29 on. Twelve cards were observed across the seven
enabled attempts, deduplicated within each trace. They included two
identical-receipt warnings, two failure-run warnings, and one inconclusive
warning. Card counts come from unsampled input nodes in recorded provider-call
ancestry; model quotations and orphan nodes do not count. No disabled attempt
received a card. No contrast, unavailable-memory, or handoff card was observed.
Local typed checks remain separate from final grading; successful in-task
commands can still have an inconclusive verifier verdict.

The largest timing outlier stays in the totals. Second-round off-arm Forth
took 550.2 seconds and 23 calls, including 354.2 seconds in tools. Three public
executions account for 340.4 of those tool seconds: a Node test runner timed
out after 120.105 seconds; `timeout 100 npx jest ./* 2>&1 | tail -20` took
100.119 seconds and recorded exit 124; a debug-instrumented Node evaluation of
`Forth.evaluate('1 2 +')` was killed after 120.146 seconds. These are measured
process durations and exits; they do not establish the precise internal cause
of noncompletion. The task ultimately passed without a task timeout. Neither
enabled Forth attempt received advice, so this outlier cannot establish that
the advice prevented the slow behavior.

The frozen inputs and settings remained unchanged throughout the comparison:

- Model: `glm-5.3-flash`, route `glm-direct`, effort `low`, thinking configured as enabled.
- Endpoint: `https://api.z.ai/api/coding/paas/v4`.
- Existing sampling: temperature 0, maximum output 8,192 tokens per call.
- Cohort: `js-forth`, `py-rest-api`, `rust-poker`, `cpp-space-age`, `py-react`,
  `py-transpose`. The first four use the established language-balanced selector;
  the last two are explicitly labelled diagnostic tasks.
- Four cells: `r1-off`, `r1-on`, `r2-on`, `r2-off`, six attempts each.
- Same pinned binary, resources, fixtures, adapter, gold and grader in both arms;
  one task at a time; fresh task homes. The existing 600-second/60-hop benchmark
  limits are held equal. Only `ANGEL_SLOPTOMIZER_LIVE` differs experimentally.
- Cold-start within-task comparison. No claim about cross-session memory,
  model-transfer efficacy, full-cohort success, production release speed, or
  deterministic provider sampling follows from this pilot.

This uses a frozen development build with default features disabled. The host
was shared: lightweight release tests and source archive hashing overlapped
the pilot, though the recorded application build and Clippy runs finished
before it began. Provider latency and cache state were uncontrolled. Recorded
task wall time is descriptive; it does not establish a controlled latency
effect or production-build speed.

The resource snapshot predates the current adventure art, so world rendering
changes cannot alter the frozen harness comparison. Source revision
`9de863f04f368b2025bfce138bdb9c88da77a5b2` lives only in the private artifact
snapshot. The shipping implementation is committed separately as `b709e60` on
the 0.1.9 release line; the package version remains `0.1.91`. Subsequent release
integration cannot alter the pinned benchmark executable or resources.
All 154 tracked files in `agent/harness`, `drive/rl_ctl` (including `rl_ctl.rs`),
and `research/sloptomizer` match that implementation byte for byte. World
presentation code differs from the earlier snapshot.

| Binding | SHA-256 |
| --- | --- |
| Cockpit source | `bf765e26e3b433af0e00c8dbc0e4dac19a223ef63a3bc3be6d2d27bd31f0a883` |
| Executable | `3778a257ba533f397d5f904c69c6114a1bef23b224398c7a4c666f9d1cbd0a21` |
| GLM configuration | `332a95be7fbd1c9c43584d5609094a8c607545bbc89c9ae680d8c003f76fc33b` |

## Evidence and reproduction

Generated receipts remain local under
`artifacts/speed-loop/sloptomizer-live-20260930/`; they are not packaged source
files. The tables and binding hashes above preserve the report's results in the
distributed source tree.

- Runner: [sloptomizer_ab.py](../../scripts/bench/sloptomizer_ab.py), with an
  explicit `--provider glm`. Default mode is a dry run; `--execute` makes the
  external calls. It refuses to overwrite an existing output directory.
- `glm-v2-dry-run/manifest.json`
  records the evaluator, task, fixture, source, binary, config and grading hashes.
- `glm-v2-live/manifest.json` and the body-free `glm-summary.json`
  retain every attempt, both within-round comparisons, coverage, outcomes and
  measured usage. The summary reports complete coverage with no contract issues.
- `glm-input-audit.json`
  rechecks the executable, helper, config, selected tasks, fixtures, gold, grader,
  adapter, evaluator, resources and runner against the recorded hashes. All match.
- `glm-ship-core-parity.json` records the 154 matching harness/learner files.
  The Forth outlier receipt, `glm-forth-outlier.json`,
  binds its public operation summaries and measured durations to tool-argument digests.
- The body-free local summary, `local-v2-summary.json`,
  retains task timing, byte counts, warning delivery and typed verifier counts.
- Raw localhost captures remain in `scripts/bench/book-harness/out/` under
  `slop-local-v2-*`. They are generated artifacts, not checked-in benchmark data.

Validation of the corrected frozen core: 8 Python relationship tests, 7 Rust
worker/persistence tests, and 91 book/continuity tests passed (2 measurement
fixtures ignored). The 10 embedding-integrity tests and embed receipt check pass.
Seven offline analyzer regressions pass, covering input ancestry, incomplete
usage, retained failed/timed-out attempts, sampling drift, grading integrity,
and disabled-arm contamination. The
[analyzer](../../scripts/bench/summarize_sloptomizer.py) makes no provider calls.

The current application also passes the world suite (366 passed, 17 ignored),
Scryglass suite (47 passed), and default-feature build. These suites overlap;
their counts are not a combined total. The repository's all-target Clippy gate
passes with its existing dead-code/type-complexity allowances. All 41 changed
Rust files pass targeted formatting, and both source-connection checks pass.
Repository-wide formatting has pre-existing differences outside this change.

The [adventure review](../world-adventure.md) documents the two visual passes and
the labelled renderer-fixture review. It contains no live GLM benchmark data.
