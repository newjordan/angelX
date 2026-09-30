# Local hand-off: closing the book and speed chapter

This chapter's work is finished in the cloud. What remains is to verify it on
your own hardware, test it with GLM, and make the decisions that only you can
make. The chapter covers two things:

- the book as a braille routing encoder, with the Sloptomizer, /loop and RL
  toolset encoded in it;
- harness speed, plus the three operator calls: the reward stamp, a single
  learner, and deferred pins.

Work down this file in order. Each experiment is pre-registered below, with
its hypothesis, arms, sample size, metrics and kill condition fixed before any
data exists. Do not change them once a run has started. If one needs to
change, write the change and its reason into the notebook first, then run it.

Notebooks (append results to their Log sections, never rewrite old entries):

- `docs/telemetry/book-encoder-20260929.md`: H1–H5, the routing map, the
  morning report.
- `docs/telemetry/harness-speed-20260929.md`: the speed log, the build table,
  connections, the three operator calls.

Bench tools: `scripts/bench/book-harness/` (see its README).

## What is claimed, and how strongly

The weakest claims are the ones to test first.

| claim | evidence so far | strength |
|---|---|---|
| Introduction: behaviour kept at pre-book fresh cost (H1) | 3 turns vs 4, fresh +0.4% | **n=1**, Sonnet via relay, easy task |
| Introduction does not get cheaper over long sessions (H2 rejected) | ±2% of pre-book at 7/50/100/213 turns | scripted stub, 4 tokenizers, deterministic |
| A model acts on a route where the facts hide the next step (H3) | vignettes | **n=1 per arm**, single turn, not a live /loop |
| The loop/RL toolset speaks in routes (H4) | routing map, every row pinned by a test | tests (strong), minus a listed remainder |
| Stub session 2.9× faster end to end, harness time −58% | 21.5–22.1 s → 7.43–7.49 s | 3 runs per build, one container |
| Time to first request 123.5 → 75.5 ms | 10 interleaved runs per build | one container |
| /loop verdicts and rl_campaign rounds teach the Sloptomizer | end-to-end tests | tests only, **no live run** |
| `rl_reward` stamped honestly (`tests`, plus `rl_reward_requested`) | unit test | tests (strong) |

## 0. Setup

- [ ] `git fetch && git checkout dev && git pull`. Record `git rev-parse HEAD`
      as `DEV`.
- [ ] Build release: `cd cockpit && cargo build --release`. Copy
      `target/release/angel` and `target/release/angel-sandbox` together into
      `scripts/bench/book-harness/h/`.
      - The harness runs the sandbox helper that sits beside its own binary.
      - A build without its matching helper measures the wrong thing. This
        cost a run in the cloud.
- [ ] Build the two reference points, each in its own worktree:
      - `git worktree add ../ax-prebook d9c11e7`: pre-book, the H1 control.
      - `git worktree add ../ax-base 3295966`: the speed baseline.
      - Release-build each one and copy its `angel` + `angel-sandbox` into
        `b-prebook/` and `b-base/` next to `h/`.
- [ ] `cd scripts/bench/book-harness/tokens && npm install`.
- [ ] Write a `RUN.md` in `scripts/bench/book-harness/out/` (the directory is
      ignored) recording:
      - host, CPU, RAM and kernel;
      - the model id, provider or endpoint, quantization, temperature and
        context size;
      - `DEV` and the reference SHAs.

## 1. Verify the chapter's claims on your hardware

- [ ] **Full test suite.** Run
      `cd cockpit && cargo test --release -- --test-threads=1`.
      - In the cloud container, three tests fail for environment reasons:
        - `…every_schema_names_its_tools_section…` (grok auth);
        - two `sandbox::bwrap` tests (uid map not permitted).
      - On a normal host with user namespaces, the bwrap tests should pass.
      - Any other failure is real. Record it and fix it before any experiment.
- [ ] **Speed reproduction.** Three stub runs per build, interleaved (base 1,
      head 1, base 2, …):
      ```sh
      cd scripts/bench/book-harness
      for i in 1 2 3; do
        STUB_CYCLES=70 MAX_HOPS=260 ./run_ts.sh b-base/angel b-base/angel-sandbox base-$i plain
        STUB_CYCLES=70 MAX_HOPS=260 ./run_ts.sh h/angel h/angel-sandbox head-$i plain
      done
      python3 harness_time.py base-1 base-2 base-3 head-1 head-2 head-3
      ```
      - Expect the ratios to hold even though the absolute times will differ:
        about 2.9× end to end, and harness time down more than 50%.
      - Per-hop growth should be nearly flat: check with
        `python3 phases.py head-2`.
      - If the ratio falls below 2×, find out why before going on.
- [ ] **Time to first request.** Ten interleaved short runs per build
      (`STUB_CYCLES=1`). In each task receipt, read `timing.startup_ms`
      (`out/<label>.result.json`) and compare medians. Cloud result:
      123.5 → 75.5 ms.
- [ ] **Deferred pins fail closed.** Use `strace -f` or a tiny shell loop:
      replace the `node` binary on `PATH` while `angel` starts, and confirm the
      first dispatch refuses with "changed while being pinned".
      - This is optional; the unit test already covers it.
      - It is worth doing once on a real filesystem.

## 2. Pre-registered experiments (GLM)

These rules hold for every experiment:

- n ≥ 5 per arm, with arm order shuffled within each round (`shuf`).
- The same model, endpoint, temperature and context size across all arms,
  recorded in `RUN.md`. Fix `seed` too if the provider honours it.
- Report the median and the range, never just the best run.
- A run that errors on the transport counts as a failed run, not a skipped
  one, unless every arm hit the same outage. If so, discard the whole round
  and say so.
- Keep every `out/*.jsonl`. Summarise with `run_summary.py`.

To point the bench at GLM: `task_run.sh` runs `proxy_log.py` in front of any
OpenAI-compatible endpoint.
```sh
export PROXY_UPSTREAM=https://<glm-endpoint>/v1 PROXY_KEY=<key> ANGEL_LOCAL_MODEL=<glm-model-id>
```
Keep the key in your shell only. Never write it into the repository.

### E1: H1 replication, the introduction (primary)

- **Hypothesis.** Compared with pre-book on the two-bug task, the introduction
  (`ANGEL_BOOK_INTRO=1`):
  - uses no more turns;
  - makes 0 ledger reads;
  - stays within +10% of pre-book's fresh (uncached) prompt tokens.
- **Arms.**

  | arm | binary | setting |
  |---|---|---|
  | `intro` | `h/` | `ANGEL_BOOK_INTRO=1` |
  | `bare` | `h/` | `ANGEL_BOOK_INTRO=0` |
  | `prebook` | `b-prebook/` | none (the variable is ignored there) |

- **Metrics.** The primary metric is fresh prompt tokens per solved task,
  computed as the prompt tokens minus the cached tokens in provider usage.
  Secondary metrics:
  - solved rate;
  - turns;
  - ledger reads;
  - total prompt tokens;
  - wall time.
- **Kill.** The introduction is dropped (default flipped to `0`) if either of
  these holds:
  - its fresh tokens are more than 20% over pre-book and it saves no turns;
  - its solved rate is below pre-book's.
- **Run.**
  ```sh
  for r in 1 2 3 4 5; do for arm in $(shuf -e intro bare prebook); do
    case $arm in
      intro)   ANGEL_BOOK_INTRO=1 ./task_run.sh h       e1-intro-$r   twobug ;;
      bare)    ANGEL_BOOK_INTRO=0 ./task_run.sh h       e1-bare-$r    twobug ;;
      prebook)                    ./task_run.sh b-prebook e1-prebook-$r twobug ;;
    esac
  done; done
  python3 run_summary.py out/e1-*.jsonl
  ```
- [ ] Run E1, update the H1 row, and log the result in the book notebook.
- [ ] If it passes on the two-bug task, repeat it on a harder task (a real repo
      bug of yours) with n=3. The two-bug task is easy, and easy tasks hide
      turn savings.

### E2: H2 under GLM's tokenizer

- **Hypothesis.** H2 stays rejected: across 7 to 213 turns, the introduction's
  total tokens stay within ±5% of pre-book's, whatever the tokenizer.
- **Method.** Run the stub replay with `STUB_CYCLES` = 2, 16, 33 and 70 for
  each arm. Then do one of these:
  - add GLM's tokenizer to `tokens/` and use `curve.mjs`;
  - if GLM's tokenizer is not packaged, send the recorded requests through the
    provider with `max_tokens=1` and read `usage.prompt_tokens`.
- **Kill.** Reopen H2 (the introduction gets cheaper with length) if the gap
  grows monotonically by more than 5% between 7 and 213 turns.
- [ ] Run E2 and add a GLM column to the H2 table.

### E3: H3 live, the /loop route-action rate

- **Hypothesis.** In a live /loop, when a Sloptomizer or loop route is
  introduced, the model's next action matches the route's action more often
  than in the bare arm. Examples of routes:
  - `⡪⠊`: stall, research idle;
  - `⡪⠙`: unverified;
  - `⡪⠉`: verifier failed.
- **Setup.** There is no headless `--loop`, so this is a TUI run:
  1. Start `proxy_log.py` by hand:
     `PROXY_LOG=out/e3-<arm>-<n>.jsonl PROXY_PORT=18099 python3 proxy_log.py`.
  2. Run `angel` with `ANGEL_DRIVER=local`,
     `ANGEL_LOCAL_URL=http://127.0.0.1:18099/v1` and the model id.
  3. Give `/loop` a goal with a verifier that stays red for a few iterations,
     so stalls and failed verdicts actually happen.
- **Arms.** `ANGEL_BOOK_INTRO=1` vs `0`. n ≥ 5 /loop sessions each, with the
  same goal and verifier.
- **Metric.** For every request whose newest tool result carries a route from
  the routing map, check whether the next assistant tool call matches the
  route's `action`. The matching script is task E3a.
- **Kill.** If the introduction's match rate is not at least 15 points above
  the bare arm's, H3 is downgraded to "no live effect". The routes then stay
  only as ledger bookkeeping.
- [ ] **E3a.** Write `route_action.py`, which reads a proxy log and prints, for
      each route, how many times it was raised and how many times the next
      call matched. Take the route → expected-tool table from the notebook's
      routing map. Pin it with a small fixture before trusting it.
- [ ] Run E3 and record the result against H3 in the notebook.

### E4: one learner, live

- **Hypothesis.** Each verified /loop iteration and each `rl_campaign` round
  adds one Sloptomizer observation. Once the first one is admitted, `suggest`
  returns warm advice.
- **Method.** During E3's intro arm, call
  `loop_research {"action":"status"}` after each verdict.
  - `loop_observations` is absent until the first admission or error. After
    that, `loop_observations.admitted` should equal the number of verified
    iterations, not counting reused red verdicts.
  - `last_error` should be empty.
  - After the first admission, `suggest` should no longer be cold (`⡪⠓⠉`).
- **Kill.** Treat a mismatch between admitted observations and verified
  iterations as a bug, not a finding: fix it, then rerun.
- [ ] Run the E4 checks. Optionally, to measure whether the shared learner
      helps, first build the ablation toggle (task X2) and compare iterations
      to green with observation on vs off.

### E5: speed behind a real model

- **Hypothesis.** Behind GLM:
  - the harness's residual time per hop is under 10% of model time;
  - the startup gain is fully realised, because the first model reply takes
    longer than the hash.
- **Method.** For both `b-base/` and `h/`, run E1's intro arm (n=5 each). From
  the `timing` block in each task receipt, compare:
  - `model_ms`;
  - `tool_ms`;
  - `residual_ms`, `other_ms` and `startup_ms`.

  In the cloud, only the stub was measured. Behind a real model, the receipt's
  own split is the evidence.
- **Report** harness overhead as a percentage of the session's wall time for
  each build. This number decides whether any more speed work is worth doing.
- [ ] Run E5 and log it in the speed notebook as "Behind a real model".

### E6: startup on your host

- [ ] Covered by the section 1 time-to-first-request check. Also record which
      verifier runtimes are present (node, cargo, python), because the
      critical path is whichever one is largest.

## 3. Decisions only you can make

- [ ] **D1: core size (H5).**
  - The standing core is about 1,037 tokens a turn. It is the only remaining
    token lever, because shortening stamps saves almost nothing.
  - Shrinking it means rewriting pages the book keeps verbatim.
  - Decide whether to try it. If yes, pre-register H5 the same way as E1:
    same task and arms, with the compact core as a fourth arm.
- [ ] **D2: honouring `ANGEL_RL_REWARD`.**
  - Today every live evaluator produces test-contract evidence, so only
    `TestReward` can score it.
  - The alternative is a code-health or lint evaluator, which would let
    `ANGEL_RL_REWARD=lint|code_health` be honoured rather than just recorded.
  - Decide whether that evaluator is worth building.
- [ ] **D3: where loop findings persist.** Either an atlas proposal (reviewed)
      or memory (automatic). Nothing persists them today beyond the Sloptomizer
      state.
- [ ] **D4: harness tool ledger into loop harvest.** Decide whether each
      iteration's harvest should carry the ledger's tool summary, which gives
      the Sloptomizer richer ideas at the cost of larger observations.
- [ ] **D5: remaining English.**
  - What is left: `/rl` operator text, JSON type-check errors and rare record
    errors.
  - Encode them into pages, or leave them as operator-facing English?
- [ ] **D6: research polling.** Should polling a live research run trip the
      repeated-poll guard, as polling a stale job does? Log E3's poll counts
      first, then decide from them.

## 4. Engineering tasks for GLM sessions

These are small, independent, and each testable on its own. None of them adds
a gate or blocks work. Run the affected tests for each one and commit it on
its own.

- [ ] **X1.** Call `background::observe` once per hop instead of walking every
      tool result, and stop cloning the two sample vectors into each refresh.
      - This is the 3–5 ms history-proportional remainder at hop 213.
      - Prove it with `phases.py`: the last quarter should cost the same per
        hop as the first.
- [ ] **X2.** Add an `ANGEL_LOOP_OBSERVE=0|1` toggle (default `1`) around
      `observe_loop_verdict` and `observe_campaign_round`.
      - It is an ablation switch for E4, not a gate: off means only "do not
        teach", never "refuse".
      - Add a row to `cockpit/docs/ENV.md`.
- [ ] **X3.** Normalise Sloptomizer ideas: a stable form for the loop
      direction and the campaign proposal (trimmed, whitespace-collapsed, with
      the route removed), so the same idea from /loop and from a campaign
      lands in the same learning bucket. Add a test that the two paths share a
      bucket.
- [ ] **X4.** E3a's `route_action.py` (above).
- [ ] **X5.** If E1 or E3 kills the introduction, flip its default in
      `book/introduction.rs` and `ENV.md` and record the kill. Do not delete
      the code: the ledger and the bare arm still use it.

## 5. Rigour checklist (before closing the chapter)

- [ ] Every experiment was pre-registered here before it ran. Any deviation is
      written into the notebook with its reason.
- [ ] Every result row names:
      - the SHA, model, provider, temperature and host;
      - n, the median and the range.
- [ ] Kills are recorded as prominently as wins, and anything the data
      rejected is dropped from the defaults.
- [ ] Raw logs (`out/*.jsonl`, `*.result.json`) are archived outside the repo,
      for example as a tarball next to `RUN.md`. The notebook links to where
      they live.
- [ ] Both notebooks' verdict tables are updated, and each carries a closing
      "Chapter closed" entry covering:
      - what holds;
      - what was killed;
      - what is deferred, and to which decision.
- [ ] The full suite is green on the closing SHA.
