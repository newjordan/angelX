# Full Rust Polyglot rerun — 2026-09-26

The complete **30-task Rust cohort solved 30/30**, with 0 evaluator timeouts. This is one fresh attempt per original Rust task, using the existing ChatGPT Codex seat: `gpt-6-luna`, medium effort, 600-second agent wall limit, 60 hops, concurrency one, and `ANGEL_OPENAI_REPLAY=0`. All tasks were selected explicitly from the original 136-task catalog; task rows, fixtures and grading were preserved.

## Full-cohort results

Times are seconds. Model/tool time comes from the harness envelope; wall time is evaluator agent time. Output is reported output tokens, including any reasoning tokens counted by the provider. Each historical row is the exact 30 Rust tasks sliced from a complete 136-task run.

| Run | Solved | Wall median | Wall p90 | Wall total | Model total | Tool total | Model calls | Tool calls | Output tokens | Timeouts |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Historical early `seed0-20260923T012019Z` | 27/30 | 31.76 | 75.30 | 1,300.52 | 1,239.36 | 53.62 | 187 | 205 | 42,904 | 0 |
| Historical latest `seed0-20260923T195649Z` | 30/30 | 32.70 | 61.29 | 1,462.86 | 1,217.96 | 237.62 | 178 | 204 | 45,822 | 0 |
| New frozen default | 30/30 | 25.15 | 62.28 | 1,084.43 | 1,019.94 | 56.82 | 204 | 209 | 36,353 | 0 |

Against the latest full run: 30 jointly solved tasks; median per-task wall delta -5.04 s (new minus old); solve gains none, losses none.
Against the earlier full run: 27 jointly solved tasks; median per-task wall delta -4.93 s (new minus old); solve gains rust-decimal, rust-forth, rust-scale-generator, losses none.

Excluding `rust-forth`, wall totals across the other 29 tasks are 876.77 s new versus 1,104.26 s in the latest historical run. This makes the influence of the slow Forth case explicit.

Reported cache share was 73.81% new versus 72.81% in the latest historical run. Total input was 1,456,794 versus 1,435,954 tokens; uncached input was 381,594 versus 390,450. All 30 tasks reported complete core usage in both runs. These small cache differences do not establish the cause of the wall-time difference.

## The three pilot Rust tasks

The shared pilot slice is `rust-acronym`, `rust-pig-latin`, and `rust-poker`. The full 30-task aggregate is not compared directly with a three-task aggregate.

| Run | Solved | Wall total (s) | Wall median (s) | Model total (s) | Tool total (s) | Model calls | Tool calls | Output tokens |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Pilot baseline | 3/3 | 81.19 | 25.57 | 78.02 | 2.53 | 21 | 22 | 2,628 |
| Pilot final default | 3/3 | 80.31 | 27.86 | 77.50 | 2.23 | 18 | 21 | 2,707 |
| New run, matching 3 tasks | 3/3 | 72.99 | 24.11 | 69.84 | 2.55 | 20 | 19 | 2,198 |

## Forth: observed repairs, not a stalled request

The new `rust-forth` task solved in 207.66 s: 183.62 s model time over 24 calls, 23.82 s tool time over 24 calls, and zero provider retries. Nine tool errors were recorded: seven failing test runs, one malformed `write_file`, and one patch match failure. Repeated allocation-limit failures and later case/definition behavior drove repairs. The longest individual tool call was a 16.688-second `run_tests`; no evaluator deadline was reached.

The latest historical Forth attempt took 358.60 s, with 152.22 s model time and 206.16 s tool time over 15 model calls. Its longest tool was a 182.570-second shell call. The new attempt used more model calls and model time while spending much less time inside tools; this single case does not demonstrate a cache or model-speed improvement.

The malformed write at hop 5 reported `missing 'path'`. Its argument keys are unavailable: direct Codex traces have empty evaluator nodes/calls, rollout capture was off, tool events retain argument digests only, stderr tails do not include that call, and the temporary runtime was removed. It is not possible to determine whether a path alias was supplied. Hop 19's patch diagnostic identifies an old-text/leading-whitespace mismatch. No speculative argument-recovery change was made.

Per-call model durations, per-tool durations/error categories and exact trace references are preserved without task bodies in the [comparison data](../../artifacts/polyglot-loop-2026-09-26/rust30-comparison.json), under `forth_evidence`.

## Provenance and limits

- Binary executable SHA-256: `86383298f3309276b531f2fafd32e69b81f53f0afc67283efdad2be305c62b79`.
- Bound cockpit source SHA-256: `390ae7411823e6a3603834bd550e6e4e26b4bcb0042da4d923cc3953d52f716f`.
- Runtime resources came from `/tmp/angelx-polyglot-loop/final-source`; the source digest was checked against the binary before execution. Implementation commit `7ad833c` (`7ad833cf649c84ac894100eb61a50c9473b2c0c4`) maps to this source. The embedded binary identity was not rewritten. The later clerk-rendering commit `04a712f` is outside this benchmark binary.
- Manifest records all 30 explicit task names, catalog/fixture/grading hashes, binary/helper/resource provenance, replay-off pin and evaluator command. The runner exited 0 after all tasks were scored.
- The access credential was checked to cover the bounded run; only its remaining lifetime was inspected, and no credential values entered reports. Runner-owned temporary credentials are removed on normal completion.
- Sequential single attempts, changing cache warmth and model nondeterminism prevent causal speedup claims. Focused UI debug compilation/testing overlapped part of this local run; it is not an idle-host or isolated-throughput measurement.
- The recorded 8,192 output budget is evaluator sampling metadata; the direct Codex endpoint retains its plan-managed output budget.

Artifacts: [source and task manifest](../../artifacts/polyglot-loop-2026-09-26/rust30-default/manifest.json), [complete evaluator traces](../../artifacts/polyglot-loop-2026-09-26/rust30-default/run/traces.jsonl), [evaluator log](../../artifacts/polyglot-loop-2026-09-26/rust30-default/eval.log), and [body-free comparison data](../../artifacts/polyglot-loop-2026-09-26/rust30-comparison.json). Historical paths and the two pilot paths are retained in the comparison data. These artifacts are retained locally and are not checked into Git.
