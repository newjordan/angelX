# DeepSeek, as DeepSeek drives it — lab notebook (2026-09-29)

The operator's brief: make angelX use DeepSeek the way DeepSeek's own
open-source harness and papers define it. Every claim below names its source
or the run that measured it.

## Sources

| source | what it settles |
|---|---|
| `deepseek-ai/deepseek-harness` at `639ed01` (2026-09-29, 0.2.0-rc.2) | the wire and loop contract as DeepSeek implements it; full spec in `/work/artifacts/deepseek-blueprint-20260929/harness-spec.md` |
| api-docs.deepseek.com (thinking mode, KV cache, tool calls, models, Anthropic API) | the documented contract; `/work/artifacts/deepseek-blueprint-20260929/api-docs-spec.md` |
| DeepSeek-V4.1-Flash tech report and model card | recommended output cap, scaffold results, prompt encoding |
| the 2026-09-14 audit (`DEEPSEEK-HARNESS-IMPLEMENTATION-AUDIT-20260914.md`, DS-01…DS-08) | the stages this work closes or leaves |

## The contract, in six rules

1. **Reasoning goes back on every earlier assistant turn while `tools` ride
   the request**, final answers included, across user turns. Missing it may
   answer 400. (Thinking-mode guide; the harness's serializer; the V4.1
   encoder disables `drop_thinking` whenever tools exist.)
2. **`max_tokens` is set explicitly.** The API defaults to 64K in thinking mode
   (128K at `max`); the V4.1 card recommends at least 256K, and the harness
   always sends 256,000.
3. **The prefix is append-only.** A hit costs 1/50 of a miss. Effort renders at
   token 0 and tools before the history, so changing either mid-session, or
   rewriting any earlier message, re-reads everything after it uncached.
4. **Compaction waits.** The harness compacts at
   min(0.8·W, W − O − 65,536) = 678,464 tokens of the 1M window, and prunes
   tool results only once compaction qualifies.
5. **The catalogue is fixed.** Tool additions arrive as `tool_addition`
   blocks with `defer_loading` (Messages only), so the declared tools stay
   byte-identical.
6. **The wire is Messages.** The harness has spoken only the
   Anthropic-compatible `/anthropic/v1/messages` since 2026-09-19.

## What angelX did (before), with evidence

- **Rule 1:** reasoning was captured and replayed on tool-call turns only.
  Final answers dropped theirs at capture, so every `/loop` iteration and
  follow-up lost the previous answer's chain of thought. Within one task,
  replay was faithful: 79 of 79 reasoned turns in the E1/E5 logs.
- **Rule 2:** no `max_tokens` was sent, so the provider's 64K applied.
- **Rule 3:** three kinds of prefix break showed up in the live heesch loops
  (old build):
  - the tool bubble re-seeded at every user turn, so the tool list changed at
    each boundary;
  - `tool_search` activations re-rendered the tools block, a 49,331-token miss;
  - tool-result aging, then rolling flushes every 12 hops, cost 34–44k-token
    misses each, to save a few hundred cached tokens.

  In a two-turn TUI session, the follow-up request hit the cache for **1,536
  of 4,117** prompt tokens.
- **Rule 4:** every paid route compacted at 120k. The heesch loop compacted
  at 138,554 tokens (a 27,203-token miss plus the lost context) and 11 times
  in 334 requests.
- **Accounting:** `prompt_cache_miss_tokens` was ignored, so uncached input
  was always "unknown".
- **Knobs:** `ANGEL_DEEPSEEK_REASONING_EFFORT` was documented but never read.
  The calibration's `max` for flash is keyed to the retired
  `deepseek-v4-flash`, so V4.1 Flash has no measured effort. That is left
  unmeasured on purpose, not re-keyed.

### The live probe that settled rule 1 (deepseek-flash, tools present)

A reasoning-heavy first turn (2,708 reasoning tokens), then a follow-up user
turn:

| follow-up carries the answer's reasoning | prompt | cache hit | status |
|---|---|---|---|
| yes (replayed) | 6,054 | 5,888 | 200 |
| no (angelX before) | 3,346 | 3,200 | 200 |
| empty string | 3,346 | 3,200 | 200 |

- **The server cached the output with its reasoning.** Replay adds 2.7k tokens,
  all cache hits at 1/50.
- **Omitting it raises no 400** on a final answer, but it silently removes the
  model's own chain of thought.
- **An empty `reasoning_content` renders as no reasoning**, so it is a safe
  repair for history whose reasoning was never held.

## Fixes (dev, `553d5af`…`1ccc334`)

| commit | fix | measured |
|---|---|---|
| `553d5af` | every reasoned turn goes back, answers and prose-recovered calls included; a turn without held reasoning goes back empty on DeepSeek's route with tools | a two-turn TUI session: turn 1's answer carries its reasoning into turn 2 |
| `4f10eba` | `max_tokens` 256,000 on DeepSeek's route (source: model card); operator caps still win | wire: every request carries it |
| `2a51841` | compaction at DeepSeek's own trigger, 678,464 (`Club::provider_compaction_budget`) | fixed loop at 147k tokens: 0 compactions (old: 3–11 per session) |
| `735f3da` | the miss field settles cache writes at zero: uncached input exact | unit + stream-commit test |
| `6dbca2b` | `ANGEL_DEEPSEEK_REASONING_EFFORT` reaches both seats | test |
| `da5dd48`, `4e8a261` | a prefix-cached seat keeps its tools across turns and seeds the bubble once per session | two-turn follow-up: cache hit **1,536 / 4,117 → 4,096 / 4,344 (94%)** |
| `1ccc334` | DeepSeek's route stays append-only between compactions (no rolling flush) | probe: every prior byte survives 16 hops |
| `00bfc38` | DeepSeek's Messages API (`ANGEL_DEEPSEEK_API=messages`): the Chat body translated out, each stream event translated back | live: solves, `input_tokens` confirmed uncached |
| `ff1b4f9` (earlier) | no llama.cpp `/props` probes on a cloud seat | startup 1,169 → 403 ms |

**Two-bug A/B** (DeepSeek flash, n = 5 per arm, interleaved; `out/dsab-*`):

| arm | solved | turns | fresh tokens | wall | startup |
|---|---|---|---|---|---|
| old build, Chat | 5/5 | 6 | 4,009 | 8.2 s | 1,192 ms |
| fixed, Chat | 5/5 | 5 | 4,095 | 6.9 s | 423 ms |
| fixed, Messages | 5/5 | 5 | 4,253 | 6.9 s | 413 ms |

A one-turn task has no turn boundaries and never nears compaction, so tokens
tie; the fixes act on long sessions and follow-ups. The long-session
comparison is running on sparky (below).

## Open, and whose call

- **Messages as the default (operator).** It is opt-in until the sparky loops
  (fixed Chat against Messages, heesch `/loop`) finish. What the Messages
  wire adds beyond the fixed Chat path: cache-safe tool additions
  (`tool_addition` plus `defer_loading`, flash only, not built yet), thinking
  signatures (not stored yet; the official harness sends none for foreign
  history either), and in-history system updates on flash (translated).
- **Reasoning across a resumed session.** Private reasoning is never
  persisted, so a resumed session sends it back empty. That is valid and
  cache-neutral, but the model loses its earlier thought. Persisting it
  privately is a design call (DS-01's recovery path).
- **`/v1/models` at startup (about 400 ms).** DeepSeek's catalog carries
  `context_window` (1,048,576), `max_output_tokens` and effort levels, and
  `catalog_capabilities` reads none of them. Either read them or take the
  probe off the path to the first request.
- **V4.1 Flash effort.** No measured calibration exists for this model, and
  the provider default (`high`) applies. The V4.1 report ran its agent evals
  at `max`.
- **Scaffold.** The V4.1 report's best scaffold was DSH Minimal (one `bash`
  tool, a one-line system prompt): 72.6 / 90.6 on DeepSWE / Terminal-Bench
  2.1, ahead of the 26-tool standard preset (70.5 / 85.8), Claude Code and
  Codex. A minimal DeepSeek preset is a research lead, not a change made here.
- **Sandbox children outlive the TUI.** When a session's TUI died, 24 solver
  processes the model had started under `angel-sandbox` kept running. The
  bench now stops them; the harness should own them itself.

## Log

(appended as results arrive)
