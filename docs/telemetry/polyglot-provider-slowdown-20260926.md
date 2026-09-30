# Qwen and GLM slowdown: request-level evidence

This audit examines the complete 136-task compact-Qwen Angel cohort and the
136-task GLM Angel cohort against their recorded OpenCode peers. It does not
substitute jointly solved tasks for the full benchmark. Full-cohort inventory,
failed tasks, and cumulative timing reconciliation are in
`artifacts/polyglot-full136-20260926/full136.json`. Request-level derived evidence
is in `artifacts/polyglot-provider-slowdown-20260926.json`.

## What is established

The excess time is primarily inside model requests, not local tests or harness
CPU. Across all 136 tasks, compact Qwen Angel takes 6,070.4 additional wall
seconds: 6,330.0 additional recorded model seconds, offset by 259.6 fewer
non-model seconds. GLM takes 1,370.4 additional wall seconds: 1,794.3 additional
model seconds, offset by 424.0 fewer non-model seconds. These are serial
cumulative task times, not elapsed time for a concurrent batch.

A controlled four-call probe additionally establishes a causal continuation
effect on the current configured local Qwen route: omitting historical
`reasoning_content` caused substantially more new reasoning and a slower reply
for the same recorded continuation. This is not a measurement of a whole-task
success rate, and it does not explain every historical slowdown.

## Controlled Qwen continuation probe

The probe reconstructed OpenCode's recorded `js-wordy` fourth request: two file
reads, a completed implementation write, and the returned write result. It kept
the recorded system/user messages, tool schemas, model, temperature 0, effort
`low`, 8192-token cap, and `tool_choice=auto`. Only historical assistant
`reasoning_content` was retained or omitted. Streaming and usage reporting were
enabled identically. Returned tools were not executed.

The original recorded and current Qwen configuration files are byte-identical;
the resolved original configuration also matches the exact local endpoint and
model `toymaker`. The source task is the pinned public Aider Polyglot benchmark.
Same-destination proof, paths, and hashes are in
`artifacts/qwen-continuation-probe-provenance-20260926.json`.

| Order | Condition | Wall seconds | First activity seconds | New reasoning characters | Completion tokens | Input tokens | Cache-read tokens |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | Retain | 8.851 | 7.079 | 32 | 91 | 11,160 | 0 |
| 2 | Omit | 14.021 | 5.500 | 881 | 404 | 8,618 | 0 |
| 3 | Omit | 13.873 | 5.519 | 881 | 404 | 8,618 | 0 |
| 4 | Retain | 8.709 | 6.948 | 32 | 91 | 11,160 | 0 |

Retaining reasoning reduced mean wall time by 5.167 seconds, or 37.05%, in this
specific continuation. It increased input and first-activity latency, but
reduced time after first activity from 8.438 to 1.767 seconds. The output hash
repeated exactly within each condition. Both conditions requested `bash`; their
argument strings differed, so this does not establish equivalent executed test
behavior. There was no measured cache-read benefit.

Four calls completed in 45.46 seconds under a 55-second per-call and 240-second
total bound. Metadata-only results and the two request hashes are in
`artifacts/qwen-continuation-probe-20260926.json`. The request is a canonical
reconstruction of normalized evaluator data, not an archived raw HTTP body;
unrecorded extension fields cannot be recovered. Current renderer/build identity
is also unknown. Initial automatic approval review rejected the transfer; one
reviewed retry was approved after exact same-destination provenance was supplied.

## Historical Qwen: output volume is the large remaining cost

The following counts include all observed sampled responses, including failed
tasks: 823 of 825 attempted Angel calls and all 771 OpenCode calls. They count
characters from normalized message fields, not tokenizer estimates. The two
missing Angel response nodes remain explicit in the artifact.

| Generated output | Compact Angel | OpenCode |
| --- | ---: | ---: |
| Reasoning characters | 789,085 | 428,175 |
| Visible prose characters | 201,341 | 15,117 |
| Tool argument characters | 353,804 | 320,490 |
| No-tool response model seconds | 1,768.6 | 394.6 |
| No-tool response prose characters | 126,471 | 12,402 |

There are 137 Angel no-tool responses versus 136 OpenCode responses; these are
mostly final answers, but the label deliberately does not assume every prose
reply was terminal. This category alone contains 1,374.0 excess model seconds.
Tool-bearing replies also contain 74,870 Angel prose characters versus 2,715 for
OpenCode. The historical Angel prompt mandates one or two visible sentences
before every tool; OpenCode repeatedly instructs minimal output and fewer than
four lines. This is a concrete prompt difference associated with substantial
avoidable narration, although a prompt-only A/B has not yet isolated its effect.

Input length alone does not explain the result. Median normalized request
message size is 17,652 characters for compact Angel versus 19,612 for OpenCode;
tool schemas are 6,083 versus 15,643 characters. Both recorded sampling settings
use temperature 0, effort `low`, and cap 8192. OpenCode additionally records
explicit `tool_choice=auto`. Normalized sampling does not preserve every raw
provider extension, so it cannot establish complete wire equivalence.

The largest equal-call examples locate the delay after the first output event:

| Task | Calls Angel / OC | Angel implementation call | OC implementation call | Angel first activity |
| --- | ---: | --- | --- | ---: |
| `js-wordy` | 5 / 5 | 159.34 s; 21,326 reasoning chars | 63.28 s; 8,610 reasoning chars | 3.395 s |
| `py-wordy` | 5 / 5 | 134.44 s; 18,664 reasoning chars | 50.86 s; 6,394 reasoning chars | 3.963 s |
| `rust-ocr-numbers` | 5 / 5 | 107.49 s; 12,180 reasoning chars | See per-call artifact | 4.256 s |

The two Wordy implementation requests occur before substantial prior reasoning
exists: Angel had generated only 144 and 161 reasoning characters beforehand;
OpenCode retained 129 and 63. Therefore the continuation probe cannot be used to
claim that missing prior reasoning explains these first implementation bursts.
The model/prompt interaction that produces those long bursts needs a separate
controlled comparison.

Trace references, relative to
`/home/frosty40/angel_tests/angelX-bench/polyglot-20260921`:

- Angel: `runs-compact-full/qwen/angelx/seed0-20260925T202320Z/traces.jsonl`,
  line 134 (`js-wordy`, call 3), line 118 (`py-wordy`, call 3), and line 67
  (`rust-ocr-numbers`, call 3).
- OpenCode JS: `runs-qwen/qwen/opencode/seed0-20260925T044244Z/traces.jsonl`,
  line 6, calls 3 and 4 (the probe's source continuation).
- OpenCode Python: `runs-qwen/qwen/opencode/seed0-20260925T034514Z/traces.jsonl`,
  line 6, call 2.
- Angel wire: the compact run's `wire/wire-3292875.jsonl` line 3 and
  `wire/wire-3224468.jsonl` line 3 supply the Wordy first-activity measurements.

## GLM: output and repair loops, with some unexplained wait outliers

All 136 GLM tasks contain 1,074 attempted Angel calls versus 1,001 OpenCode calls.
Of these, 1,073 and 1,001 have sampled response nodes. Recorded completion tokens
are 222,019 versus 178,729 (24.2% more); the sampled-response call-time subset is
8,748.2 versus 6,958.8 seconds (25.7% more). This subset excludes the Angel call
without a sampled response; the full 136-task model-span total used above is
8,753.2 seconds for Angel. The aggregate completion-token rate is similar, about
25.4 versus 25.7 tokens per model second. This supports increased generated work
as a major contributor; it is not proof of equal provider throughput per call.

Angel produces 58,100 reasoning tokens versus 35,756 and 114,019 visible-prose
characters versus 24,958. No-tool responses take 1,032.7 versus 565.2 seconds.
Large failed/repair-heavy tasks contribute substantially: `rust-alphametics`
times out and adds 45 calls, while `rust-decimal`, `js-alphametics`, and
`js-zebra-puzzle` add 22, 16, and 27 calls. Full-task ranks, including failures,
belong in the full-136 artifact rather than a success-only table.

Two concrete trace examples show different causes:

- `js-transpose`, Angel trace line 128: 11 calls versus 10, but 151.8 versus
  56.4 model seconds. Angel calls 3, 7, and 8 each generate roughly 1,000
  reasoning tokens and take 31–39 seconds. Typed test replies before calls 5
  and 7 begin with an earlier-output-omitted marker. The existing diagnostic
  retention fix directly targets that lost failure context, but its speed effect
  on this task still needs measurement.
- `cpp-grade-school`, line 44: two initial `code_mode` calls fail with
  `missing 'path'` before a direct read batch succeeds. This is a real wasted
  sequence, but only two such failures appear across the cohort, so it cannot
  explain the broad slowdown. Its final reply also takes 37.34 seconds for only
  197 completion tokens, with first observed activity at 34.99 seconds. The
  trace does not identify whether that delay is provider queueing, prefill, or
  stream buffering.

Likewise, Angel `rust-ocr-numbers` has nearly the same total completion tokens
as OpenCode (682 versus 676), but its final reply waits 23.67 seconds before
observed activity. These cases rule out a blanket claim that every excess second
is caused by verbosity. OpenCode's retained artifacts contain no matching wire
first-token measurements. Angel's first-activity timestamp is a transport
observation, not an independently measured provider queue or prefill timer.

GLM trace roots are `runs/glm/angelx/seed0-20260923T195647Z` and
`runs/glm/opencode/seed0-20260921T142517Z`; the selected task line numbers match.
The historical runs occurred on different dates, so backend load is a remaining
confound. Native `thinking`/`clear_thinking` fields are not part of normalized
`calls[].sampling`; current source alone cannot prove their historical values.

## Bounded next changes and measurements

1. Add narrowly scoped Qwen private-reasoning continuation behind an explicit
   route capability/flag, preserving the exact assistant tool turn and excluding
   it from visible text, saved sessions, and unrelated providers. The four-call
   result justifies testing that mechanism on this route; it does not justify
   enabling it for every compatible HTTP provider or claiming cache savings.
2. Run the frozen current coding prompt against the old compact prompt on a
   small predeclared set containing both Wordy tasks, OCR, Forth, and simple
   controls. Current source already removes mandatory routine narration. Hold
   model settings and continuation policy fixed; measure all task outcomes,
   output reasoning/prose, and model/tool time. Do not shorten the prompt again
   merely because an older cohort was verbose.
3. Keep usage collection on for the local Qwen experiment. The historical Angel
   Qwen traces lack provider token/cache usage; OpenCode reports it. The controlled
   direct probe confirms this current endpoint accepts `include_usage` and
   reports usage. Unknown historical usage is not zero cache.
4. Recheck the existing test-diagnostic retention fix on a declared GLM failure
   case before attributing a speedup to it. Preserve failures and timeouts in the
   comparison; fewer local test seconds alone cannot fix the recorded model gap.

No harness source was changed during this investigation. No model reasoning
text, credentials, or raw request bodies are exported in its artifacts.
