# The book as a routing encoder — lab notebook (2026-09-29)

Branch `claude/dreamy-carson-7d17h8`. Every claim below names the run that
supports it; numbers without a run are labelled estimates.

## Goal

By the morning, the book's wire is an efficient routing encoder:

1. **Understood without spending turns.** A stamp the harness raises reaches
   the model in a form it acts on without a `ledger://` read.
2. **Lean.** Over a session, the wire costs fewer tokens than the English
   prompt it replaced (`d9c11e7`), and never more turns.
3. **Covers the loop and RL toolset.** The Sloptomizer (`loop_research`: the
   Pareto selector, UCB bandit and MicroLearner), `rl_campaign`, `reinforce`
   and the `/loop` controller speak in routes, and detectors raise the right
   route at the right moment (logic pathing), with tests.

## Metrics

| id | metric | better |
|---|---|---|
| M1 | model turns to solve | lower |
| M2 | turns spent only reading the ledger | lower (0 is the target) |
| M3 | input tokens, total and fresh (prefix cached), DeepSeek V4 tokenizer | lower |
| M4 | task solved (the evaluator's own check) | must hold |
| M5 | the model acts on a raised route (judged from its next turn) | yes |
| M6 | standing prefix and per-stamp wire tokens | lower |

## Method

- **Model in the loop:** Sonnet as a subagent behind a relay (`bench/relay.py`):
  the real headless `angel --task-json` sends each request to the relay, a
  fresh Sonnet sees the request as a file and answers one turn. Caveat: the
  prompt is quoted text to Sonnet, not its real system prompt.
- **Deterministic replay:** a scripted stub model (`bench/stub_model.py`)
  replays the same session against each build, so only the harness's bytes
  differ; four tokenizers (DeepSeek V4, GPT-4o, Qwen3, Llama 3).
- **Held constant:** `ANGEL_SOTA_CAVEMAN=0`, `ANGEL_TASK_RECON=0`, fresh `HOME`,
  the same fixture and prompt.

## Results so far

### R1 — static wire, scripted 7-hop task (stub replay)

| | first request | total input | fresh input | turns |
|---|---|---|---|---|
| pre-book `d9c11e7` | 2,577 | 19,686 | 3,244 | 7 |
| braille only, never decoded | 1,495 | 12,201 | 2,183 | 7 |
| braille only, each route decoded once | 1,493 | 46,068 | 4,350 | 13 |

Braille costs 2–3 tokens per cell. Across the 489 replaced hints, addresses
cost 19% (DeepSeek, Qwen) to 29% (GPT-4o, Llama) of the English; hints under
~5 English tokens cost more as braille. Decoding a route cost more than the
English it replaced (2,093 vs 1,002 tokens), 31% of it the address repeated
on every ledger line.

### R2 — Sonnet, one-bug task

| | turns | stamps read | total | fresh | solved |
|---|---|---|---|---|---|
| pre-book | 4 | — | 11,608 | 3,083 | yes |
| braille only | 4 | 0 | 6,848 | 1,840 | yes |
| braille + "read the braille" rule | 6 | 7 | 22,457 | 4,519 | yes |

Without a rule, Sonnet ignored every stamp; with one, it obeyed and spent two
read-only turns.

### R3 — Sonnet, two-bug task

| | turns | stamps read | total | fresh | solved |
|---|---|---|---|---|---|
| pre-book | 4 | — | 12,425 | 3,410 | yes |
| braille only | 4 | 0 | 8,196 | 2,425 | yes |
| braille + static legend (3,522 tok) | 3 | 0 | 16,547 | 5,892 | yes |

With a legend, Sonnet acted on the stamps without reading: it finished on the
`⠺⠓` cue at once and batched its re-run with the fix. The static legend was too
large for a short task.

## Hypotheses (pre-registered)

- **H1** The introduction (English at first sight, stamps after; `0f704c4`)
  keeps R3's behaviour (0 reads, fewest turns) at a fresh-token cost within 10%
  of pre-book on the two-bug task. *Kill:* more than 20% over pre-book with no
  turn saved → revise the entry format.
- **H2** Over a long scripted session (200+ hops), the introduction's total
  input is below pre-book's, and the gap grows with length.
- **H3** A model shown an introduced loop/RL route acts on it on its next turn
  (M5) without a read.
- **H4** Every model-facing sentence of the loop and RL toolset is a page, and
  each lifecycle state of the Sloptomizer that should change the model's next
  step has a detector and a route, pinned by tests.

## Log

(appended as the work proceeds)

### 2026-09-29 — H1: the introduction on the two-bug task (Sonnet relay)

Build `0f704c4` (introduction on by default), same fixture, prompt and relay as R3:

| | turns | stamps read | total | fresh | solved |
|---|---|---|---|---|---|
| pre-book | 4 | — | 12,425 | 3,410 | yes |
| braille only | 4 | 0 | 8,196 | 2,425 | yes |
| braille + static legend | 3 | 0 | 16,547 | 5,892 | yes |
| **introduction** | **3** | **0** | **9,002 (−28%)** | **3,422 (+0.4%)** | yes |

GPT-4o tokenizer: introduction 9,192 total / 3,483 fresh vs pre-book 11,933 / 3,280.
Sonnet batched the fix with its verifier and answered on the `⠺⠓` cue with no read,
the legend's behaviour at the braille-only wire's total cost. **H1 supported**
(fresh within 10% of pre-book, one turn fewer). One run, one model, easy task.

### 2026-09-29 — H2: a 213-turn scripted session (stub replay)

A deterministic stub runs 70 red cycles (read, red run, swap one wrong operator
for another), then the fix, a green run and the answer: 213 turns, identical
model behaviour against each build. Cumulative input, DeepSeek V4 tokenizer:

| turns | pre-book total / fresh | braille only | introduction |
|---|---|---|---|
| 7 | 19,807 / 3,083 | 12,312 / 2,015 | 19,696 / 3,079 |
| 50 | 232,007 / 6,652 | 178,784 / 5,595 | 232,382 / 6,657 |
| 100 | 674,172 / 10,902 | 568,541 / 9,862 | 674,355 / 10,890 |
| 213 | 2,447,085 / 20,569 | 2,226,087 / 19,573 | 2,443,791 / 20,545 |

GPT-4o at 213: pre-book 2,347,297 / 19,738; braille 2,176,220 / 19,015;
introduction 2,393,056 / 19,986.

**H2 rejected as stated.** The introduction tracks pre-book within ±2% at every
length; the gap does not grow. Only three routes fired in 213 turns (the hygiene
cue, three red runs `⠧⠙`, the finish cue): detectors latch by design, so stamps
are sparse and their per-event saving is negligible. Braille only is 9% lighter,
all of it the untranslated core (~1,037 tokens × 213 turns), which R2/R3 show a
model ignores.

**What it means.** On tokens the book is a wash: the wire costs what its core
costs. Its measured gains are turns (H1: one fewer) and routing capability. A
leaner wire needs a leaner core, not shorter stamps.

- **H5 (new)** A compact core — each of the type's routes introduced as its
  signal and action instead of its full pages — keeps H1's behaviour (0 reads,
  3 turns, solved) with fewer fresh tokens than pre-book. *Kill:* a lost task or
  an extra turn.

**H5 not pursued.** Every core route (`⠞⠃ ⠺⠃ ⠺⠉ ⠺⠙ ⠺⠋ ⠞⠙ ⠧⠚ ⠕⠋ ⠍⠁–⠍⠛`) has an
empty action and a signal that labels rather than instructs ("who the model is,
compact core"); the instructions live only in the verbatim pages. A compact core
would paraphrase them, which the book forbids ("nothing is paraphrased, merged
or dropped"). Shrinking the core is an authoring decision for the operator: the
data says it is the only token lever left.

### 2026-09-29 — the ⡪ research chapter (`1e6abe3`)

No detector reacted to the Sloptomizer or rl_campaign (inventory of
`research.rs`, `loop_campaign.rs`, `rl_ctl.rs`, `loop_ctl.rs`): a candidate that
passed or beat its baseline, a run that failed, finished unverified or lost its
baseline, a learning error, a settled campaign and a stall with research idle
all reached the model as bare JSON, and an unverified run looked like a success.
The loop ledgers' overflow shelf ⡪ now holds that lifecycle, raised by pure
detectors on the run record and campaign status (`warpath` field, evidence in
the ledger): ⡪⠁ passed, ⡪⠃ won, ⡪⠉ failed, ⡪⠙ unverified, ⡪⠑ baseline red,
⡪⠋ learning error, ⡪⠛ live, ⡪⠓ advice (the evidence note's verbatim pages, the
cold-start page), ⡪⠊ stall with research idle, ⡪⠚ settled campaign (with the
no-spread page). The research invitation stays off while the verifier is
blocked or a candidate or submission is overdue. Pinned by the end-to-end
Sloptomizer tests (paired win `⡪⠃⡪⠁`, red `⡪⠉`, unverified `⡪⠙`, red baseline
`⡪⠑`, learning error `⡪⠁⡪⠋`), detector unit tests, a stall test and the
invitation gate. 289 rl/loop/reinforce tests green.

### 2026-09-29 — H3: does a model act on an introduced ⡪ route? (Sonnet vignettes)

Six single-turn vignettes: the same loop-iteration transcript with the route
and its introduction, or bare (the control). One fresh Sonnet per arm.

| scenario | with the route | bare |
|---|---|---|
| candidate won (`⡪⠃⡪⠁`) | `loop_research results` to fetch the patch, citing the stamps | `read_file` the patch diff |
| unverified run (`⡪⠙`) | **rerun with `compare: true` and the loop verifier** | reads the unverified patch to apply it |
| stall, research idle (`⡪⠊`) | **`loop_research suggest`** | spends a turn decoding `ledger://⠘⠁⠑` |

**H3 supported, with a split:** where the facts already say what to do (a
candidate that passed its verifier), the route changes little; where the facts
hide the meaning (an unverified run looks like a success) or the next move is
not obvious (a stall), the route changes the action to the one it names. n=1
per arm; vignettes, not a live loop.

### 2026-09-29 — Volume II schemas: braille vs their pages in English (kill)

Hypothesis (mine, from R1's near-equal tool totals): the braille schema
descriptions save nothing, so reading them out in English would restore their
meaning for free. Measured with `schema_expansion_dump` (every advertised
schema as sent, and with each description expanded through `ledger::expand`):

| set | DeepSeek V4 | GPT-4o |
|---|---|---|
| hot path (9 tools) | 1,365 → 2,729 (+100%) | 1,432 → 2,608 (+82%) |
| lean (69 tools) | 11,415 → 17,196 (+51%) | 11,668 → 16,365 (+40%) |
| full (69 tools) | 12,407 → 18,206 (+47%) | 12,639 → 17,346 (+37%) |

**Rejected.** The pages hold far more guidance than the old schemas carried, so
the braille schemas are a real saving against their own text; R1's parity was
with the old, shorter schemas. Keep Volume II on the wire; the parameter names
carry most of the meaning, and `ledger://` the rest.

### 2026-09-29 — H4: the rest of the loop and RL toolset

Encoded:

- **`⠳⠊⠁` / `⠳⠊⠃`** (`c3fd194`): the deep experiment's `hypothesis:` and
  `artifacts:` labels were English beside pages that already held them.
- **`⠘⠚` loop-tool refusals** (`d929c87`): ten refusals of `loop_research` and
  `rl_campaign` (an idea a run or candidate needs, a `run_id` the tool did not
  issue or cannot find, fewer than two samples, an audit without its task,
  verifier or independent source) are pages. A value of the wrong JSON type
  (`… must be an array`, `… must be boolean`) stays English, as in every tool.

Logic-pathing defects found and fixed (`dc127a8`, each pinned by a test):

- **Handoff-RL measured nothing.** `/handoff-rl` bound the RL tools with
  `verify: None`, so every research run there finished `⡪⠙` unverified and
  `rl_campaign` refused with `⠘⠊⠑`, whose page says "bind /goal cmd", which did
  nothing under handoff-RL. It now pins the goal's acceptance command as `/loop`
  does (`handoff_rl_binds_the_goals_verifier_as_loop_does`).
- **A second campaign told the model to type `/rl stop`.** An operator command
  the model cannot issue; the model's launch now answers `⠪⠊⠓` (continue useful
  work, or request stop and inspect status), and the operator keeps its text.
- **`⡪⠚⠁` could never fire.** A campaign's status carries its outcome as serde's
  `{"Ok": …}`, and the no-spread detector read the bare field, so a round with
  no advantage spread was never flagged. My unit test had used the bare shape.
  The detector reads through the wrapper; the settled-status route and the cold
  advice `⡪⠓⠉` are now pinned end to end, not just as detector units.
- **`⠟⠉`** (a stop strategy) described `loop_research` as "stamped findings over
  several rounds … on the open question": neither is what the tool does, and it
  is only offered inside a `/loop`. It now says so and names suggest then run
  with compare.

Examined and left:

- **Polling a live research run** is not counted by the repeated-poll guard.
  Changing `is_passive_status_call` also reclassifies competition in-flight hops,
  and the guard exempts live background jobs by design ("a wait, not a loop").
  No run shows a model over-polling research, so there is nothing to fix yet.
- **English still on the wire from these tools:** JSON type checks (above); the
  record's cancellation and integrity errors (`research cancelled after
  baseline`, `experiment source mismatch; no feedback admitted`, `research
  receipt corrupt: …`), which follow the model's own stop or a corrupted store;
  and `rl_ctl.rs` text shared with the operator's `/rl` (launch message,
  `log_tail`, settle errors), whose reader cannot decode the ledger.

### 2026-09-29 (local, atlas) — pre-registration: four models in place of GLM

Written before any counted run. The operator replaced the hand-off's single
GLM-5.3 with four models, to calibrate the introduction across providers and
tokenizers:

| model | seat | reasoning |
|---|---|---|
| `grok-4.7` | `grok` (xAI OAuth) | low |
| `glm-5.3-flash` | `glm-5.3-flash` (z.ai coding plan) | the seat's idle default, `low` |
| `deepseek-flash` | `deepseek-flash` (DeepSeek API) | provider default |
| `muse-spark-1.3-contributor` | `meta` (Meta Responses API) | provider default |

- Each model runs on its native seat, as angelX runs it, not through
  `ANGEL_DRIVER=local`. `proxy_log.py` sits in front of the seat's URL variable
  (`task_run.sh`, `ROUTE=`). No temperature is sent by any seat.
- **Grok is deferred.** Its OAuth seat has a fixed xAI URL, so the logging proxy
  cannot sit in front of it without handling the operator's token. How to run
  it is the operator's call.
- **Unchanged:** the arms, n = 5 per arm per model, arm order shuffled within
  each round, the metrics and the kill conditions.
- **Added decision rule.** Each model gets its own verdict. The introduction's
  default flips (X5) only if E1's kill fires on at least two of the three
  models; a kill on one model is recorded as that model's finding.
- **E2 method.** Two of the three models have no packaged tokenizer, so the
  final request of each stub session (`STUB_CYCLES` 2, 16, 33, 70) is sent to
  each provider with the smallest output cap, and its reported prompt tokens
  are read. The local GLM-4.5 and DeepSeek V4 tokenizers give the cumulative
  curve as a cross-check.
- **Pilot (not counted):** one one-bug run per seat on the head build, to prove
  the seats and the logs. All three solved with 0 ledger reads, in 4
  (GLM flash), 5 (DeepSeek flash) and 7 (Muse) requests.

### 2026-09-29 (local) — pre-registration: E3 on the Yukon heesch challenge

Written after E1, E2 and E5 and before any counted E3 session. The operator
chose the live fire: the Yukon heesch benchmark, the kind of challenge the
harness is built to compete in.

- **Workspace.** A fresh copy of the heesch checkout per session (`947f057`,
  the promoted frontier: a 15-cell polyhex, Hc = 4, the class record). The copy
  has no git remote, and the fresh `HOME` has no Yukon login, so nothing can be
  submitted or posted.
- **Verifier.** `e3/verify_heesch.py` is green only when `heesch_verify`
  accepts `submission/best.heesch` with `hc_verified >= 5`, the challenge's
  "first survivor". It checks the witness only (0.05 s); the proof gate is not
  run. The target stays red, so stalls and failed verdicts happen on their
  own.
  - *Amended after the pilot, before any counted session:* green also needs
    `heesch_verify`'s non-tiler evidence. A tiler has a witness for every
    corona count, so without that check a single hexagon would pass. In the
    pilot, DeepSeek went looking through the CI workflows for an Hc = 5
    certificate.
- **Task.** The same `/loop now iters=8 …` text in both arms. It seeds one
  direction from the operator (candidates designed top-down, as a defective
  cut-and-project slice of a higher-dimensional periodic structure) so each
  loop has something concrete to try.
- **Loop settings.** `ANGEL_LOOP_STALL_STOP=2` (default 4), so a session of 8
  iterations can raise `⡪⠊` more than once. The driver stops a session after
  5,400 s. `ANGEL_ACTION_CAPSULES=0` keeps approval modals from blocking the
  unattended TUI.
  - *Amended after the pilot, before any counted session:* `STALL_STOP=1`,
    6 iterations, a 7,200 s limit. The pilot's first heesch iteration ran
    over 20 minutes (DeepSeek built a corona solver), so two consecutive
    stalls would rarely fit a session. With one, every red `LOOP_DONE` raises
    `⡪⠊` in the next iteration's prompt.
- **Design.** Three models (GLM flash, DeepSeek flash, Muse), n = 5 per arm per
  model. Each round runs the intro and bare arms of every model at the same
  time, so both arms see the same provider conditions.
- **Metric, as registered.** `route_action.py` (pinned by
  `test_route_action.py`) reads every session's request log. For each ⡪ route
  in the newest model-facing messages, it checks whether the model's next
  call matches the route's action, taken from `d2467_research.rs`. The match
  rate is pooled per arm and per model.
- **Kill, as registered.** H3 is downgraded to "no live effect" for a model if
  the intro's match rate is not at least 15 points above the bare arm's. A
  model with fewer than 5 judged routes in either arm gets "insufficient data",
  not a verdict.
- **E4, changed method.** The operator cannot call `loop_research status`
  from the TUI. Instead, after each intro session, the learner's own state
  (`learning/*/state.json`) is read: its `loop-iteration` observations are
  compared with the verified iterations in the loop record. Any `status`
  calls the model made itself are read from the log as well.

### 2026-09-29 (local) — E3 moves to sparky (operator's call)

Written before any counted session on the new host. Benchmarks run on sparky
(DGX Spark, aarch64, 20 cores, 121 GiB), not on atlas; the bench lives on its
external drive (`/srv/models-hdd/angelx-bench`, `WORKROOT` there).

- **Atlas round 1 is not counted.** Its six sessions (both arms of GLM flash,
  DeepSeek flash and Muse) were stopped at 5,488 s of the registered 7,200 s
  when the operator moved the benchmarks. Its logs stay under `out/e3-*-1.*`
  and are reported as a truncated pilot only.
- **The counted design is unchanged:** five fresh rounds on sparky, the same
  `DEV` build (`158b7a8`, built natively for aarch64), the same heesch commit
  (`947f057`, set up on sparky) and the same settings.
- **Muse needs its key on sparky.** Its rounds run as soon as the operator
  provides it; each Muse round still pairs its two arms in time.
- **Session hygiene.** On atlas, solvers the models started under
  `angel-sandbox` outlived their TUI (24 processes). The session now stops
  every process still working in its workspace at the end. The harness bug
  behind it is fixed too (`54e6a34`, `4b5cbab`).
- **The first sparky launch was aborted, and its logs are not counted
  (`out/aborted-sparky-round1/`).**
  - A random proxy port collided: the GLM bare session's proxy failed to
    bind, and its harness talked to a DeepSeek session's proxy, which logged
    six GLM requests answered with 400s.
  - Proxies now take an OS-assigned port and prove they bound it (`d1b9d59`).
  - The counted run restarted at 16:23. The atlas logs carry no mismatched
    model.

### 2026-09-29 (sparky) — pre-registration: Grok joins E1, E2, E3 and E5

Written before any counted Grok run. The operator approved their logged-in Grok
OAuth for the benchmarks.

- **Seat.** `grok-4.7` at effort `low` on the `grok-api` seat, as the operator's
  polyglot Grok runner does it. The logging proxy carries the OAuth bearer
  (re-read from `~/.grok/auth.json` at most once a minute); the harness sees
  only a placeholder key. Near expiry, one minimal call through angelX's own
  Grok OAuth seat refreshes and persists the token.
- **Host.** Grok runs on sparky; E1, E2 and E5 for the other three models ran
  on atlas. Tokens, turns and ledger reads do not depend on the host. Wall
  time does, so Grok's wall is compared only within its own arms.
- **The same protocol throughout:**
  - E1: intro, bare and pre-book on the two-bug task, n = 5 per arm, shuffled
    within rounds.
  - E2: the final request of each stub session, counted by xAI.
  - E5: base against head, n = 5 each.
  - E3: five paired rounds on heesch.
- **Decision rule.** Grok gets its own verdicts. The four-model decision rule
  for the introduction was fixed with three models and has already fired
  (two of three killed), so Grok's E1 is reported as a fourth model's finding
  and does not reopen the flip.
- **The Muse key is on sparky**, pinned to `muse-spark-1.3-contributor` (the
  operator's call, for price). Muse's E3 rounds run on sparky, and its wire
  shows only that model.

### 2026-09-29 (local) — E1: the introduction on three models — killed on two

`DEV` = `158b7a8` (`h/`), pre-book `d9c11e7`. Two-bug task, n = 5 per arm per
model, arms shuffled within each round, all three models run at the same time.
Seats and settings as registered (`out/RUN.md`). Medians, with the range in
brackets; "fresh" is prompt minus cached tokens, as the provider reports them.

| model | arm | solved | turns | fresh tokens | total tokens | ledger reads | wall s |
|---|---|---|---|---|---|---|---|
| GLM-5.3 flash | pre-book | 5/5 | 4 (4–6) | 4,900 (4,353–5,704) | 17,516 (15,937–28,360) | 0 | 25.9 (21.9–40) |
| | bare | 5/5 | 4 (4–8) | 3,500 (1,049–4,777) | 11,436 (11,417–25,058) | 0 | 22.3 (20.5–43.4) |
| | intro | 5/5 | 4 (4–8) | 5,947 (2,761–6,239) | 17,609 (16,891–38,414) | 0 | 24.6 (19.5–49) |
| DeepSeek flash | pre-book | 5/5 | 4 (4–5) | 4,370 (4,062–5,061) | 18,360 (17,502–24,389) | 0 | 6.7 (6.1–8.1) |
| | bare | 5/5 | 5 (4–6) | 4,404 (3,917–4,494) | 17,204 (12,749–22,224) | 0 | 7.5 (6.3–9) |
| | intro | 5/5 | 5 (4–5) | 5,535 (4,692–5,708) | 23,400 (16,468–23,711) | 0 | 7.6 (5.5–8.4) |
| Muse Spark 1.3 contributor | pre-book | 5/5 | 7 (6–8) | 21,871 (20,199–26,810) | 34,391 (29,373–41,907) | 0 | 18.9 (16.4–51.6) |
| | bare | 5/5 | 8 (8–8) | 17,008 (14,255–17,636) | 30,888 (30,820–30,968) | 0 | 26 (17.8–44.7) |
| | intro | 5/5 | 8 (7–8) | 13,824 (13,311–17,129) | 30,389 (27,068–30,968) | 0 | 19.8 (18.5–25.3) |

Verdicts against the registered kill (fresh tokens more than 20% over pre-book
with no turn saved, or a lower solved rate):

- **GLM flash: killed.** Intro fresh tokens +21% over pre-book, turns 4 vs 4.
  This is borderline: the intro's range (2,761–6,239) overlaps pre-book's.
- **DeepSeek flash: killed.** +27%, turns 5 vs 4. The ranges barely overlap.
- **Muse: not killed.** Intro fresh tokens are 37% under pre-book. H1's "no
  more turns" still fails (8 vs 7).
- **Every run solved (45/45), and no arm made a single ledger read.** No
  model spent a turn decoding the braille, with or without the introduction.
- **By the registered rule (a kill on two of three models), the default
  flips:** `ANGEL_BOOK_INTRO` is now off (X5, `0e6857f` on
  `eng/handoff-20260929`). The code stays, and `1` turns it back on.
- **The harder-task repeat is not run.** The registration makes it
  conditional on a pass on the two-bug task.
- **The bare wire (braille only) cost the fewest fresh tokens** on GLM (−29%)
  and Muse (−22%) against pre-book, and the same on DeepSeek (+1%). Against
  pre-book it took the same turns on GLM and one more on DeepSeek and Muse.

**H1 row: not supported across models.** The single Sonnet relay run did not
generalize: two of three models paid the introduction's tokens and saved no
turn.

### 2026-09-29 (local) — E2: H2 under three providers' own counts

Stub sessions at `STUB_CYCLES` 2, 16, 33 and 70 (9, 51, 102 and 213
requests), on `h/` for intro and bare and on `b-prebook/`. Each session's
final request went to each provider (output cap 16 tokens); the table shows
the prompt tokens each provider reported, as pre-book / bare / intro:

| requests | GLM-5.3 flash | DeepSeek flash | Muse Spark 1.3 contributor |
|---|---|---|---|
| 9 | 3,667 / 2,906 / 3,996 | 4,259 / 3,348 / 4,446 | 4,572 / 3,687 / 4,764 |
| 51 | 7,625 / 6,866 / 7,978 | 10,153 / 9,239 / 10,357 | 11,013 / 10,124 / 11,221 |
| 102 | 12,382 / 11,623 / 12,735 | 17,256 / 16,342 / 17,460 | 18,779 / 17,890 / 18,987 |
| 213 | 22,780 / 22,094 / 23,280 | 32,759 / 31,919 / 33,111 | 35,725 / 34,910 / 36,081 |

- **Intro vs pre-book, per request:**
  - GLM: +329 tokens at 9 requests to +500 at 213 (+9.0% to +2.2%);
  - DeepSeek: +187 to +352 (+4.4% to +1.1%);
  - Muse: +192 to +356 (+4.2% to +1.0%).
- **Bare vs pre-book:** a constant −690 to −910 tokens per request, the
  untranslated core.
- **Cumulative cross-check** (local DeepSeek V4 tokenizer, `curve.mjs`,
  213-request sessions):
  - intro vs pre-book: −0.2% at 7 requests, +0.7% at 25, +0.8% from 50
    to 213;
  - bare: −37.8% at 7, down to −8.8% at 213.

**H2 stays rejected on all three tokenizers.** The introduction never gets
cheaper with length: it costs a steady 200–500 tokens per request more than
pre-book, and that surplus grows slightly as more stamps are introduced. The
registered kill (a gap that grows monotonically in the intro's favour) does
not fire. The only point outside ±5% is GLM at 9 requests (+9%), where the
introduction's English is the largest share of a short prompt.

GLM's tokenizer counts the same 213-request wire about 30% smaller than
DeepSeek's or Muse's.

### 2026-09-29 (sparky) — Grok: E1 and E2

Grok 4.7 at effort `low`, as pre-registered (n = 5 per arm, shuffled; 15/15
solved; 0 ledger reads).

| arm | turns | fresh tokens | total tokens | wall s |
|---|---|---|---|---|
| pre-book | 4 | 8,838 (4,199–18,201) | see `out/e1-grok-*` | 7.8 |
| bare | 4 | 3,958 (3,387–8,958) | | 7.8 |
| intro | 5 | 5,995 (5,042–9,541) | | 8.5 |

- **E1:** not killed. The introduction's fresh tokens are 32% *under*
  pre-book. H1's "no more turns" still fails (5 against 4). The pre-book
  arm's range is wide: Grok's cache hits varied run to run.
- **E2**, final request of each stub session, counted by xAI (pre-book /
  bare / intro):

  | requests | pre-book | bare | intro |
  |---|---|---|---|
  | 9 | 4,911 | 3,868 | 4,940 |
  | 51 | 8,947 | 7,936 | 9,006 |
  | 102 | 13,880 | 12,829 | 13,919 |
  | 213 | 24,573 | 23,522 | 24,686 |

  - The introduction adds only 0.3–0.7% per request on Grok, the least of the
    four providers.
  - The bare wire saves a steady 1,040–1,050 tokens per request.
  - H2 stays rejected on a fourth tokenizer.
- **Across four models:**
  - The introduction was killed on GLM flash and DeepSeek flash, and cheaper
    than pre-book on Muse and Grok.
  - No model read the ledger in any of the 60 runs.
  - The bare wire cost the fewest fresh tokens on three of four models (Grok
    −55%, GLM −29%, Muse −22%) and tied on DeepSeek (+1%).

### 2026-09-29 (local) — correction: Muse never received the introduction

Found while answering the operator's question: did any model use the legend?

- **The introduction exists only on the Chat Completions wire.** It is
  applied in `club/http.rs` (`introduction::apply` at `:2318`). Muse's seat is
  a Responses API seat (`CodexClub`), so `ANGEL_BOOK_INTRO=1` changed nothing
  on Muse's wire. No Muse request in E1 or E3 carries the introduction's lead
  sentence; every GLM, DeepSeek and Grok intro-arm request carries it.
- **So Muse's E1 "intro" arm is a second braille-only sample.** Its
  −37% was never an introduction effect; it was noise between two
  braille-only samples. **Muse's E1 verdict: not tested.** The E1 kill rule
  stands: it fired on GLM flash and DeepSeek flash, the two of the three
  original models whose intro arm was real. Grok, the fourth model, was not
  killed (−32%).
- **Muse's E3 comparison is invalid for the same reason.** Its remaining
  rounds are stopped to spare the metered spend. The round already running
  finishes as braille-only data.
- **The operator's question, answered from the wire:**
  - No model made a single `ledger://` read: 0 in 60 E1 runs, and 0 in 623
    `/loop` requests so far.
  - In the intro arm the meaning is pasted in at first sight, so no lookup
    is needed. Behaviour matched braille only anyway: after the first green
    test run, every model answered on the next request in 29 of 30
    braille-only and intro runs (the other took one more).
  - On this task the models act on the plain facts ("tests: 3 passed"), not
    the stamp. Whether a stamp moves an action where the facts hide the next
    step is E3's question.

### 2026-09-29 (local) — the legend is back on, and now on every wire

The operator's call: the reactive legend (a stamp's English the first time a
session shows it, then the stamp alone) is the approved design and stays on.
What went wrong here, stated plainly:

- **The E1 kill judged the legend by the wrong measure.** Its rule (fresh
  tokens more than 20% over pre-book with no turn saved) ran on a
  four-turn task where the legend cannot save a turn; the hand-off itself
  warned that easy tasks hide turn savings. The rule fired on GLM flash
  and DeepSeek flash, and `0e6857f` switched the legend off. That reversed
  a core design decision on evidence that could not judge it. **Reverted in
  `dc0f65a`.** What E1 does show is the legend's token cost on a trivial
  task (GLM +21%, DeepSeek +27%, Grok −32%). It shows nothing about the
  legend's routing value.
- **The legend reached one wire only.** It lived in `club/http.rs`, so the
  Responses seats (Muse, and the operator's Codex seats) and the Grok ACP
  harness surface only ever saw bare braille. **Fixed in `95c85d2`:** every
  wire builds from the same introduced copy, with a wire test per transport.
  DeepSeek's Messages wire inherits it from the Chat body.
- **"No model used the legend" was the wrong reading.** With the legend
  working, zero `ledger://` reads is the expected outcome, because the
  meaning arrives inline. Whether the legend's routes change actions is
  E3's measure.
- **Muse's legend arm is rerun on the fixed build**
  (`e1-muse-introfix-*`, sparky), since its first intro arm never received
  the legend.

### 2026-09-29 (sparky) — does a route change the next move? (decision replay)

The test E1 could not run: at a route's decision point, does the model's next
move become the route's action? The harness's own requests at three decision
points were captured with the scripted stub (`decide_capture.sh`,
`decide/scenarios.json`), then replayed to each model ten times
(`decide/decide.py`, 440 calls, no errors).

- **Points.** ⠺⠛ after three single reads (action: share the next reads in one
  response); ⠧⠋ after an answer that followed an untested edit (action: run
  the tests, or say the change is untested); ⠺⠓ after a green run (action:
  answer).
- **Wires.** Current build with the legend (`intro`), braille only (`bare`),
  braille with the decision stamp's line removed (`nocue`), and the pre-book
  build at the same point (`prebook`). ⠧⠋ has no `nocue` request: without the
  checkpoint the harness sends none. Pre-book shows no cue at ⠺⠛ or ⠺⠓ and its
  own stop stamp `⠥⠟` at the checkpoint.
- **Same bytes to every model**, over Chat Completions, with each seat's
  settings (GLM thinking at low effort, Grok 4.7 at low effort, Muse
  contributor, DeepSeek flash).

Replies whose next move was the route's action, out of 10:

| point | model | pre-book | no cue | braille | braille + legend |
|---|---|---|---|---|---|
| ⠺⠛ batch | GLM-5.3 flash | 5 | 3 | 4 | **10** |
| | DeepSeek flash | 0 | 2 | 3 | **10** |
| | Muse contributor | 1 | 6 | 6 | 5 |
| | Grok 4.7 low | 10 | 10 | 10 | 10 |
| ⠧⠋ untested | GLM-5.3 flash | 2 | – | 7 | **8** |
| | DeepSeek flash | 10 | – | 1 (8 decoded) | 8 |
| | Muse contributor | 0 | – | 0 (2 decoded) | **10** |
| | Grok 4.7 low | 7 | – | 2 (2 decoded) | **9** |
| ⠺⠓ finish | GLM, DeepSeek, Grok | 10 each | 10 each | 10 each | 10 each |
| | Muse contributor | 8 | 0 | 1 | 4 |

Totals: ⠺⠛ 16 / 21 / 23 / **35** of 40; ⠧⠋ 19 / – / 10 / **35** of 40.

- **The legend is what makes a route act.** Braille alone barely beats no cue
  at ⠺⠛ (23 vs 21 of 40). With the legend, GLM and DeepSeek batch every time.
  At ⠧⠋, braille alone makes models decode first (DeepSeek reads
  `ledger://⠧⠋⠟⠁` in 8 of 10 replies). That is the protocol working, at one
  extra round trip. The legend skips the round trip: 35 of 40 act at once.
- **Muse is the exception at ⠺⠛ and ⠺⠓.** Its other moves are `tool_search`
  for the "systematic-debugging" skill, which the standing skills catalog
  advertises on every wire, pre-book included. After a green run on braille,
  Muse keeps exploring (`list_dir`) instead of answering. With the legend it
  answers 4 of 10 times, at 45 s and 3,746 output tokens median (18 s and 731
  on braille only). Pre-book gets 8 of 10.
- **Cost:** the legend adds 1,071–1,151 prompt tokens at these early requests,
  which carry the session's first sightings (E2: +0.5–2.2% per request by 213
  requests).
- **Harness finding, not fixed here:** ⠧⠋'s action names `run_tests`, but
  `run_tests` is not among the nine tools loaded at that request (shell,
  read_file, write_file, str_replace, apply_patch, grep, list_dir, code_mode,
  tool_search). Models run the tests through `shell`; Grok once searched for
  the tool first.
- **Limits:** single decisions on small workspaces, not whole sessions. n = 10
  per cell, so the intervals are wide (95% Wilson in the report). Muse was
  asked over its Chat Completions endpoint, not its Responses seat.

Data: `/work/artifacts/book-handoff-20260929/decide/` (captures and every
reply); report section "Does a route change the next move?".

### 2026-09-29 (sparky) — the legend against English at every sighting

The operator's question: is the legend more efficient than injecting English
into the prompt? The fair competitor is the same routes with their English at
every sighting, not the pre-book harness. It is now a bench arm,
`ANGEL_BOOK_INTRO=every`. DeepSeek flash only (the operator's call: the model
that tests best; Muse is not calibrated and ⠧⠋ is broken, see below).

- **Tokens through one turn** (DeepSeek's counts, the scripted 213-request
  session):

  | wire | request 9 | 51 | 102 | 213 |
  |---|---|---|---|---|
  | legend | 4,176 | 10,084 | 17,258 | 33,185 |
  | English every time | 4,239 | 10,133 | 17,290 | 33,179 |
  | pre-book English | 4,133 | 10,046 | 17,220 | 32,981 |
  | braille only | 3,075 | 8,969 | 16,126 | 31,993 |

  Identical within a turn: the harness raises few stamps in one turn and
  seldom repeats one. Real heesch loops repeat 6–21 stamps per 100–240
  requests. At 25–40 tokens a line, that is well under 1% of their context.
- **Tokens across turns:** at turn 2 of a TUI session, English every time
  carries 324 more prompt tokens (15,718 against 15,394, +2.1%). Each turn
  reopens with the protocol, batching and verification lines (741 characters),
  and the cue repeats its line (121). The legend sends those as stamps.
- **Routing at a repeat sighting** (the batch cue in turn 2, whose English
  came in turn 1; 20 replies per wire): legend (stamp alone) 9/20, English
  every time 11/20, braille only 5/20, no cue 6/20, pre-book 0/20. The legend
  and the repeated English are indistinguishable at this n, and both beat
  braille alone. Every wire batches less here than at the first sighting in
  the task mode (10/10). The TUI turn carries 67 tools, not 9, and a longer
  context. English every time drops as much as the legend, so the stamp
  riding alone is not the cause.
- **Twenty turns** (a scripted TUI session, each turn three reads, the batch
  cue and an answer; DeepSeek's count at the end of each turn): English every
  time minus the legend is +145 at turn 1, then grows by exactly **176 tokens
  a turn**, to +3,489 (+16%) at turn 20 (25,080 against 21,591). The legend
  minus braille alone is 1,344 at every turn, paid once and never grown.
  **Correction:** the 324 at turn 2 above is turn 1's 145 plus one turn's 176,
  so it is not the per-turn saving. The "about 300 a turn" I told the
  operator was wrong: it is 176 in this session.
- **Verdict:** within one turn the legend costs what English every time
  costs. Across turns it saves a fixed amount per turn (176 tokens here),
  which compounds in long research sessions: about 17,600 by turn 100 at this
  rate. Its stamp alone routes about as well as the repeated English. Braille
  alone is 3–26% cheaper than the legend but routes like no cue.
- `/tmp` hit this user's quota during the captures, filled by test-suite
  fixtures. The bench scripts now take `$TMPDIR`.

## Morning report

### Verdicts

| | hypothesis | verdict | evidence |
|---|---|---|---|
| H1 | the introduction keeps the legend's behaviour at about pre-book fresh cost | **supported** | 3 turns vs 4, 0 ledger reads, fresh +0.4%, total −28% (Sonnet relay, n=1) |
| H2 | over 200+ hops the introduction gets cheaper than pre-book | **rejected** | within ±2% of pre-book at 7, 50, 100 and 213 turns; the gap does not grow |
| H3 | a model acts on an introduced loop/RL route without reading it | **supported, split** | the route changed the action where the facts hid it (unverified run, stall); little where the facts already said it (n=1 per arm, vignettes) |
| H4 | the loop/RL toolset speaks in routes, with detectors pinned by tests | **supported, with a listed remainder** | table below; the remainder is JSON type checks, rare record errors and operator-shared `/rl` text |
| H5 | a compact core saves tokens without losing behaviour | **not run** | it would paraphrase the core's verbatim pages; that is an authoring call |
| — | reading Volume II schemas out in English is free | **killed** | +37% to +100% tokens |

### Against the goal

1. **Understood without spending turns — met.** The introduction (English at
   first sight, the stamp alone after) is the only wire form under which a model
   both acted on the stamps and spent no read. Braille alone was ignored (R2,
   R3); a "read the braille" rule cost two read-only turns (R2); the static
   legend worked but cost 3,522 tokens up front (R3).
2. **Lean — not met, and the data says why.** Stamps are sparse, because
   detectors latch, so shorter stamps save almost nothing. A session costs what
   its standing core costs. The only token lever left is a smaller core (H5),
   which means rewriting pages the book keeps verbatim: your call.
3. **Covers the loop and RL toolset — met.** Every Sloptomizer and campaign
   state that should change the model's next step raises a route from a pure
   detector, pinned end to end (map below). Four logic-pathing defects turned up
   and are fixed: handoff-RL ran without a verifier, the model was told to type
   `/rl stop`, the no-spread page could never fire, and the `⠟⠉` strategy
   described a tool that does not exist.

### The routing map

| state (what the model faces) | route | raised by | pinned by |
|---|---|---|---|
| research run or campaign in flight | `⡪⠛` | `research()`, `campaign()` | `loop_pause_cancels_a_live_campaign_…` (campaign); detector units (research) |
| candidate passed its verifier | `⡪⠁` | `research()` on the record | `sloptomizer_learning_failure_…` (`⡪⠁⡪⠋`) |
| candidate beat its paired baseline | `⡪⠃⡪⠁` | `research()`, paired delta > 0 | `sloptomizer_real_tool_turn_…` |
| candidate's verifier failed | `⡪⠉` | `research()` | `sloptomizer_real_tool_turn_…` (and in the loop context) |
| finished with no verifier | `⡪⠙` | `research()` | `sloptomizer_unverified_attempt_…` |
| baseline red, candidate never started | `⡪⠑` | `research()` | `sloptomizer_failed_baseline_…` |
| result stands, learning failed | `⡪⠋` | `research()` | `sloptomizer_learning_failure_…` |
| advice with no evidence yet | `⡪⠓⠉` | `cold_advice()` | `sloptomizer_real_tool_turn_…` |
| the runner's evidence note | `⡪⠓⠁⡪⠓⠃` | `routed_advice()` | the same, and ORIGINAL_HINTS |
| the loop stalled, research idle | `⡪⠊` | `loop_pivot_after_stall` | `a_stall_with_research_idle_routes_to_the_sloptomizer` |
| campaign settled | `⡪⠚` | `campaign_route()` | `loop_native_campaign_…` |
| its last round had no spread | `⡪⠚⠁` | `no_spread()` | detector units, on the status's `{"Ok": …}` shape |
| second campaign launched by the model | `⠪⠊⠓` | `start_recorded` with a loop owner | `loop_pause_cancels_a_live_campaign_…` |
| a call refused for its arguments | `⠘⠚⠁`–`⠘⠚⠚` | `loop_research`, `rl_campaign` | `loop_tool_refusals_answer_with_their_page` |
| research invitation withheld | (no header) | verifier blocked, candidate or submission overdue | `sloptomizer_real_tool_turn_…` (history without the header) |
| handoff-RL's verifier | (binding) | `handoff_rl_bind` pins `/goal cmd` | `handoff_rl_binds_the_goals_verifier_as_loop_does` |

### Recommendations

- **Keep the introduction on** (the default; `ANGEL_BOOK_INTRO=0` turns it off).
  It is the one wire form that bought behaviour without costing tokens.
- **Decide the core's size.** If you want the wire leaner, the standing core
  (about 1,037 tokens a turn) is where the saving is; shortening stamps is not.
- **Rerun H1 and H3 on DeepSeek** once the network allows it. Every
  model-in-the-loop result here is one Sonnet run behind a relay that quotes
  the prompt instead of sending it as a system prompt.
- **Decide on research polling:** whether polling a live research run should
  trip the repeated-poll guard, as polling a stale job does. There is no data
  either way yet.
- The branch is pushed and is not merged into `dev`.

### Test status

Full suite on the final code (`dc127a8`, serial): 4,933 passed, 85 ignored,
3 failed. All three fail because of this container, not the branch:
`every_schema_names_its_tools_section…` needs grok credentials to advertise
`grok_research`, and the two `sandbox::bwrap` tests cannot create a user
namespace here (`bwrap: setting up uid map: Operation not permitted`). The
branch changes no sandbox code.
