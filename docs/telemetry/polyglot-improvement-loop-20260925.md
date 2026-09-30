# Polyglot improvement loop · 2026-09-25

Historical report: the [2026-09-26 full136 audit](polyglot-full136-slowdown-20260926.md)
supersedes the cohort inventory and median-led comparisons below. The
[provider follow-up](polyglot-provider-slowdown-20260926.md) records the later
successful Qwen continuation probe. For new work, start with the
[current speed-loop handoff](../SPEED_LOOP.md).

The existing cross-harness runs are the primary performance evidence. This audit
recomputed 77 eligible Polyglot runs/chunks into 19 primary cells and 14 matched
comparisons, then separately analyzed 324 attempts from the earlier 18-task
repository-repair benchmark. The fresh 12-task Luna pilot is a regression check;
it is not the primary baseline and does not replace the historical evidence.

The body-free [Polyglot board](../../artifacts/polyglot-loop-2026-09-25/historical-board.md)
and [repository-repair board](../../artifacts/polyglot-loop-2026-09-25/historical-board-repair.md)
include solve counts, median/p90 wall time, model/non-model time, calls, tokens,
cache coverage and explicit failures. Their adjacent JSON files retain per-task
measurements, source pins, all eligible runs and exact recorded settings.

```sh
python3 scripts/analyze-polyglot-history.py \
  --output artifacts/polyglot-loop-2026-09-25/historical-board.json \
  --repair-root /home/frosty40/angel_tests/angelX-bench/rerun-20260921
python3 scripts/analyze-qwen-traces.py \
  --output artifacts/polyglot-loop-2026-09-25/qwen-request-shapes.json
```

## Findings from the existing runs

| Polyglot model | Angel solved | OpenCode solved | Angel / OpenCode wall median | Matched solved-task median delta |
|---|---:|---:|---:|---:|
| DeepSeek | 134/136 | 132/136 | 10.7 / 8.6 s | +0.1 s across 130 |
| GLM | 134/136 | 133/136 | 46.1 / 38.9 s | +4.6 s across 132 |
| Grok | 136/136 | 136/136 | 17.5 / 16.2 s | +0.5 s across 136 |
| Qwen, compact Angel | 135/136 | 135/136 | 78.9 / 37.7 s | +33.5 s across 135 |
| Muse | 136/136 | 136/136 | 25.5 / 24.2 s | Different recorded model IDs |

Positive deltas mean Angel was slower. The table compares the latest eligible
full Angel cells with the existing peer cells, not reruns chosen for success.
Luna has a full Angel run but no matching peer run in this collection. The
DeepSeek omp cohort is partial (93 tasks); it is labeled accordingly. Quarantined
GLM results, aborted runs, smoke tests and selective retries are excluded from
the primary board. The 17 Qwen rotation chunks per harness have disjoint task
identities and matching recorded settings; overlaps or configuration drift are
rejected. The later compact Angel run was not interleaved with those peer runs.

“Same recorded settings” means evaluator model ID, sampling configuration and
wall cap match. Native requests, serving builds and effective output limits are
not proven identical. This distinction matters for the Qwen alias and for Muse:
Angel used `muse-spark-1.3`, whereas the peers used its `-contributor` variant.
Proxy `prompt_tokens` count uncached input in these records; total input adds
cached input. Missing usage is unknown, not zero. Peer non-model time is a wall
residual that includes tools and harness work, not an isolated tool timer.

Higher cache percentage alone did not predict speed. DeepSeek OpenCode consumed
67.70M total input tokens versus Angel's 12.73M, while uncached input was
1.75M versus 1.45M. On jointly solved tasks the median harness-exclusive time
was lower for Angel: DeepSeek 1.043 versus 2.018 seconds and GLM 1.633 versus
2.379 seconds. The larger opportunities are model output and repeated turns.

The separate repository-repair corpus is 18 tasks × three repetitions, not 54
distinct tasks. Every harness solved all 54 attempts for each model. Angel's
median task/repetition delta versus OpenCode was −1.6 seconds for DeepSeek and
−3.6 seconds for GLM; versus omp it was −2.8 and −0.2 seconds. These results
show that the relative speed changes with workload, so they are not pooled with
Polyglot.

The trace-backed implementation opportunity is failed-test diagnostic loss.
Three DeepSeek `rust-bowling` recovery calls spent 4.325 model seconds retrieving
failure details that the tool's tail-only truncation had removed; a GLM
`py-bowling` recovery spent 30.613 seconds. The updated test-output formatter
preserves the first actionable failure and terminal test summary within its
existing 1,500-byte budget. Historical snippet replays verify preservation;
these durations are observed opportunities, not measured new speedups. Exact
evidence is in [the diagnostic audit](../../artifacts/diagnostic-output-evidence-2026-09-25.md).

## Qwen: output volume and a concrete telemetry correction

The recorded route is `toymaker`, described by the benchmark configuration as
Qwen3.8 Flash Next 125B-A6B, served by a local ds4 engine with MTP depth one.
It is not a request to the public OpenRouter or Qwen Cloud service. The complete
compact Angel run has the same single failed task as OpenCode; its jointly
solved tasks took 5,955.8 more model seconds and 267.4 fewer non-model seconds.

The following counts use only actually produced `calls[].node` responses,
avoiding duplicate branch history in `nodes[]`. They describe normalized trace
characters, not tokenizer counts or exact HTTP bytes. The Angel column covers
812 observed responses from 813 attempted calls; the missing/error response
remains visible in the JSON. OpenCode covers all 763 calls.

| 135 jointly solved tasks | Compact Angel | OpenCode |
|---|---:|---:|
| Generated reasoning characters | 732,719 | 397,486 |
| Generated visible-prose characters | 200,810 | 15,117 |
| Generated tool-argument characters | 340,215 | 319,449 |
| Median serialized request message characters | 17,622.5 | 19,598 |
| Tool-schema characters per request | 6,083 | 15,643 |

Angel generated 1.84× the reasoning and 13.28× the visible prose, despite smaller
request messages and tool schemas. For `js-wordy`, both used five calls: one
Angel reasoning response took 159.3 seconds and produced 21,326 reasoning
characters; its peer took 63.3 seconds and produced 8,610. This supports an
output-volume bottleneck; it does not establish which prompt or continuation
choice caused it. The current coding discipline already discourages routine
narration, so the next measurement should test the frozen current prompt rather
than assume another prompt-shortening change is needed.

OpenCode retained prior `reasoning_content` in 628 calls; Angel retained none.
Angel's private reasoning continuation is currently restricted to recognized
DeepSeek V4 routes. This is a high-priority experiment, not a justified generic
HTTP default. [OpenRouter's reasoning documentation](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)
describes returning reasoning during tool continuations.
[Qwen Cloud's request contract](https://docs.qwencloud.com/api-reference/chat/openai-chat)
describes supported models' `preserve_thinking` flag and historical assistant
`reasoning_content`. Those public contracts do not prove this custom server's
ChatML rendering semantics.

The engine source named by the benchmark, commit
`5707d4f23362a7208483028e6d34519267266729` in the local
`comps/qwen38-125b-a6b-cuda-v1` repository, does parse assistant
`reasoning_content`. However, that pinned server source contains DeepSeek and
GLM renderers, while the benchmark config names a ChatML serving path. The
deployed renderer/build therefore needs an additional identity check. Its
generic effort parser maps `low`, `medium`, `high` and `xhigh` to the same HIGH
mode; recorded `low` cannot be interpreted as the Qwen Cloud low token budget.

A current read-only `/models` request returned HTTP 200 and confirmed alias
`toymaker`, model name Qwen3.8 Flash Next 125B-A6B, owner `ds4.c` and a 65,536-token
context. It exposes no serving build revision or renderer identity. Available
local evidence does not resolve the ChatML/source mismatch. The route is
reachable, but that exact provenance gap blocks a safe retained-reasoning
comparison in this cycle. The body-free check is preserved in
`artifacts/polyglot-loop-2026-09-25/qwen-route-availability.json`; no generation
request was made.

There is an actionable measurement fix now: pin
`ANGEL_OPENROUTER_STREAM_USAGE=1` **for this known Qwen route** in a new cell.
The recorded engine source accepts `stream_options.include_usage` and emits
stream usage only when requested. Angel omits it by default because the unknown
`toymaker` alias does not advertise cache capability, explaining the absent
usage fields in every Angel Qwen task. OpenCode and omp do report usage. Do not
retroactively fill historical token counts or force this field on unknown
strict servers.

The next bounded experiment should first record serving build/renderer identity
and verify its reasoning rendering. Then compare the frozen default with an
explicit route capability that preserves returned assistant `reasoning_content`
on tool-call turns. Hold model alias, temperature 0, recorded effort `low`,
8,192 output cap, wall/hop limits and the usage override fixed. Use
`js-wordy`, `py-wordy`, `rust-forth` and `rust-parallel-letter-frequency`,
interleave both variants serially, and report correctness, calls, output volume,
cached/uncached input and wall time. Add `preserve_thinking` only if the actual
renderer requires it; do not assume the cloud flag applies to ds4. No new Qwen
model calls or provider-default changes were made for this analysis.

## Fresh regression pilot method

The pilot has 12 tasks, three per language, selected by a fixed hash of task
names without looking at prompts or outcomes. It is separate from the published
full-corpus benchmark results.

The reproducible runner is `scripts/run-polyglot-loop.py`. Its default performs
local task loading and evaluator configuration validation without model calls.
`--execute` enables a real run. Every invocation requires a new output directory
and preserves earlier results.

```sh
python3 scripts/run-polyglot-loop.py \
  --angel-bin /path/to/pinned/angel \
  --sandbox-bin /path/to/pinned/angel-sandbox \
  --resource-root /path/to/matching/source-checkout \
  --output artifacts/polyglot-loop-2026-09-25/new-cell

# Use another new cell directory and append --execute for a model-backed run.
python3 scripts/summarize-polyglot-loop.py \
  artifacts/polyglot-loop-2026-09-25/baseline-12-r2 \
  artifacts/polyglot-loop-2026-09-25/final-candidate-cell \
  --output artifacts/polyglot-loop-2026-09-25/comparison.json
```

The default route is the existing ChatGPT Codex Responses seat, `gpt-6-luna`, at
medium reasoning effort. Both cells use one attempt per task, 600 seconds of
agent wall time, 60 hops, concurrency one, and the same existing guard settings.
The evaluator records 8,192 output tokens in its sampling configuration. The
direct Codex endpoint does not accept `max_output_tokens`, so its actual output
budget remains plan-managed; 8,192 is not claimed as an enforced limit here.
No deterministic model seed is claimed.
`--openai-replay off` or `--openai-replay on` explicitly pins
`ANGEL_OPENAI_REPLAY` in the cell manifest; omission preserves the binary default.
The final binary defaults this experimental protocol behavior off.

| Language | Selected tasks |
|---|---|
| JavaScript | `js-forth`, `js-bottle-song`, `js-say` |
| Python | `py-rest-api`, `py-dot-dsl`, `py-variable-length-quantity` |
| Rust | `rust-poker`, `rust-acronym`, `rust-pig-latin` |
| C++ | `cpp-space-age`, `cpp-parallel-letter-frequency`, `cpp-bank-account` |

Selection sorts each language's task names by
`SHA256("angelx-polyglot-speed-v1:" + name)` and takes the first three. The source
catalog contains 136 tasks: 48 JavaScript, 34 Python, 30 Rust and 24 C++.
Its SHA-256 is
`0faa31697e91d67447dd3cbc488045d1f688c081adc9f95985daa153d3a42a50`.
`--per-language` changes cohort size; repeated `--task` options explicitly select
another cohort. Comparisons must use matching selection and limits.

Each cell snapshots the binary, native sandbox helper, runtime resources,
selected pristine fixtures and evaluator adapter. It records file hashes,
binary build information, the exact command, task identities, evaluator version
and evaluator source hash in `manifest.json`. It verifies the copied Python
adapter is the module actually imported. Task prompts, verifier commands, gold
solutions and harness guards are not changed. Historical benchmark directories
are read only.

Credentials are copied only for execution into a private temporary directory,
then removed on normal completion or keyboard interruption. They do not enter
the cell artifacts. The evaluator receives an allowlisted environment and each
rollout receives a fresh HOME. Toolchain and download caches remain shared, as
in the original evaluator. Provider prompt caches may also remain warm across
cells. Interleaved repeat runs are needed before attributing small differences
in latency to a code change.

The copied OAuth snapshot has a lifecycle limitation: a refresh would update
only that temporary copy, without synchronizing the primary credential file.
This pilot's access credential was checked to cover the bounded run comfortably;
neither credential synchronization nor refresh behavior is part of this cycle's
performance changes. Longer campaigns need an expiry preflight before starting.

The evaluator is Prime Intellect Verifiers 0.3.1 with the existing local
`angel-action-v1` adapter, using its subprocess runtime and evaluator-owned
grading after any agent exit. This is diagnostic local execution, not a claim
of Docker-isolated leaderboard evidence.

Artifacts are under `artifacts/polyglot-loop-2026-09-25/`. Each cell contains:

- `manifest.json`: provenance and settings; start/end time and runner exit code.
- `eval.log`: evaluator progress and errors.
- `run/traces.jsonl`: grader outcomes and agent timing/usage envelopes.
- `run/wire/`: private per-call metadata for transport diagnosis.
- `bin/`, `resources/`, `evaluator/`, `fixtures/`, `tasks.json`: pinned inputs.

The summarizer reports solve count, field coverage, wall time, model time, tool
time, startup and shutdown, tool overhead, verifier time, calls, hops, input,
cached input, output and reasoning tokens. Missing telemetry remains `null`.
For paired cells it lists contract mismatches, missing tasks, solve gains/losses
and per-task candidate-minus-baseline deltas. It does not print prompt, answer,
credential or wire contents.

The first attempted cell, `baseline-12`, failed during task loading before a
model call: fixture symlinks were correctly rejected by the evaluator's path
confinement. The runner now copies the selected fixtures and performs an actual
taskset-load preflight even in dry mode. `dry-baseline-fixtures` loaded all 12
tasks and validated the config successfully. The selected fixture snapshot is
66 files / 2,067,181 bytes and contains no dependency or compiler caches.

Offline regression checks:

```sh
python3 -m unittest discover -s tests/python -p test_polyglot_loop.py -v
```

These checks cover stable balanced selection, explicit cohort validation,
fixture confinement and copy independence, environment isolation, unknown
telemetry versus reported zero, partial coverage, comparison mismatches, proxy
token normalization, percentile rendering and response counting without
duplicating replayed trace nodes. All ten checks passed.

## Baseline pilot

`baseline-12-r2` completed with runner exit code 0. All 12 tasks passed the
evaluator, all 12 reported complete core usage, and no trace reported a timeout,
integrity failure or error. The body-free machine-readable summary is
`artifacts/polyglot-loop-2026-09-25/baseline-summary.json`.

| Metric | Total | Median per task |
|---|---:|---:|
| Agent wall | 361.021 s | 25.959 s |
| Model time | 325.229 s | 21.496 s |
| Tool time | 33.049 s | 0.893 s |
| Evaluator verifier time | 27.632 s | 0.386 s |
| Startup and shutdown | 1.352 s | 0.116 s |
| Tool overhead | 3.367 s | 0.204 s |
| Model calls / hops | 78 | 6 |
| Tool calls | 84 | 6 |
| Input tokens | 501,766 | 31,410 |
| Cached input tokens | 349,184 | 20,736 |
| Uncached input tokens | 152,582 | 10,374.5 |
| Output tokens | 11,068 | 745.5 |
| Reasoning tokens | 458 | 0 |

The aggregate prompt-cache hit ratio was 69.591%. Timing fields are separate
reported spans and should not be added as disjoint components.

The baseline is release profile with default features disabled
(`video_decode=false`), from source checkout commit `4415f66`. Its bound cockpit
source digest is
`b658023f27406c20480525d5566524887758233dbd68a71b1be9fb2db33ec862`;
the executable digest is
`abfab10199e196838706d222d8fcc8bc279536bb207a4d9ec207973e7d97413e`.
The full helper, adapter, evaluator and resource pins are in the cell's
`manifest.json`. A baseline alone does not establish an improvement.

## Final default-off regression result

`final-default-12` completed with runner exit code 0 and all 12 tasks passing.
Both baseline and final reported complete usage for every task, zero trace
errors, zero timeouts and zero integrity failures. The body-free paired result
is [final-vs-baseline.json](../../artifacts/polyglot-loop-2026-09-25/final-vs-baseline.json).

| Metric | Baseline total | Final total | Baseline / final task median |
|---|---:|---:|---:|
| Agent wall | 361.021 s | 318.977 s | 25.959 / 27.649 s |
| Model time | 325.229 s | 286.932 s | 21.496 / 23.247 s |
| Tool time | 33.049 s | 29.461 s | 0.893 / 0.766 s |
| Evaluator verifier time | 27.632 s | 25.064 s | 0.386 / 0.381 s |
| Startup and shutdown | 1.352 s | 1.214 s | 0.116 / 0.099 s |
| Tool overhead | 3.367 s | 2.449 s | 0.204 / 0.180 s |
| Model calls / hops | 78 | 65 | 6 / 5 |
| Tool calls | 84 | 74 | 6 / 6 |
| Input tokens | 501,766 | 382,666 | 31,410 / 29,108.5 |
| Cached input tokens | 349,184 | 256,000 | 20,736 / 17,408 |
| Uncached input tokens | 152,582 | 126,666 | 10,374.5 / 9,937.5 |
| Output tokens | 11,068 | 9,511 | 745.5 / 821 |
| Reasoning tokens | 458 | 230 | 0 / 0 |

Aggregate wall time fell 11.6%, model calls 16.7% and total input 23.7%. The
paired task wall median was −0.891 seconds (final minus baseline), while the
unpaired cohort median increased by 1.690 seconds. `js-forth` alone improved
44.936 seconds, more than the entire 42.043-second aggregate gain; the other
11 tasks together took 2.893 seconds longer. Cache hit ratio decreased from
69.591% to 66.899%, even as uncached input fell by 25,916 tokens. These are
descriptive results from one attempt per task, not a repeatable general speedup
or causal attribution to an individual code change. The firm result is no
observed correctness regression across this cohort.

Task selection, prompts, grading, fixtures, gold, evaluator, adapter, model,
effort, limits, concurrency, release profile and disabled video feature matched.
The summarizer flags one intended environment difference: the baseline omitted
`ANGEL_OPENAI_REPLAY`, while the final explicitly pins it to `0`. Binary and
runtime resource changes are the candidate under test and remain individually
hashed. The runner digest changed to expose that explicit switch and correct
abnormal-exit labeling; successful execution otherwise followed the same runner
path. Sequential provider/cache conditions and stochastic choices remain
uncontrolled, so the historical cross-harness board remains the primary
performance reference.

The final bound cockpit source digest is
`390ae7411823e6a3603834bd550e6e4e26b4bcb0042da4d923cc3953d52f716f`;
the executable digest is
`86383298f3309276b531f2fafd32e69b81f53f0afc67283efdad2be305c62b79`.

## Interrupted experimental replay cell

`candidate-12` completed only its first task. It was interrupted with exit code
130 after `js-bottle-song` stopped producing completed model-call records for
more than six minutes. Its `experiment-status.json` preserves the event. This
is incomplete experimental protocol evidence, not a completed solve rate or
performance improvement. The final default-off candidate and bounded explicit
replay probe must be reported as separate cells.

The original baseline and experimental candidate runner digests also differ by
an exit-handler correction: interrupted runs now report 130 rather than being
misclassified as timeouts. This did not affect the successful baseline's task
execution. Exact per-cell runner hashes remain in their manifests.

The final binary's bounded `js-bottle-song` probes both passed, with 120-second
wall caps and 12-hop limits. `replay-off-probe` took 36.577 seconds / six model
calls; `replay-on-probe` took 23.878 seconds / seven calls. Both runner exits were
0 and both reported complete usage. Binary, helper, resources, runner and task
contracts matched; the only intended contract difference was
`ANGEL_OPENAI_REPLAY=0` versus `1`. The body-free comparison is
`replay-probe-comparison.json`. One sequential task establishes protocol
completion under this bound, not a repeatable latency improvement. Replay stays
opt-in and the interrupted earlier cell remains part of the record.
