# Hand-off: the Treebeard compactor (2026-10-04)

The operator's idea, now built: the Treebeard lane runs a **rolling trash
compactor**. A local model works beside the paid driver and handles "live
cache work": it reduces the root's footprint and keeps the details that don't
need to reach the SOTA model on the local box.

- Commit `877fdce` on `dev`. It was cherry-picked from `49dbb1c` on `multi`.
- Private only; it is not yet in a public release.

## What it does

1. **Live digests at entry.** The Treebeard bulk veto parks a tool result of at
   least 16 KB under a handle (`handle_store::eager_offload_tool_result`). When
   it does, the compactor reads the parked body and writes the root a digest.
   The digest rides the receipt under `⡌⠃`. The root sees the handle and the
   digest, never the bulk, and can still `handle_read` the exact bytes.
2. **Rolling compaction.** The same helper writes the background compaction
   summaries at 80% of budget, via `ANGEL_COMPACT_URL`.

**Cache safety.** The digest is attached before its message is ever sent, so
nothing already sent is rewritten and the provider's prefix cache never moves.

## Where it lives

| piece | place |
|---|---|
| compactor, start/finish/render | `cockpit/src/agent/harness/compactor.rs` |
| hook (per-hop tool-result loop) | `cockpit/src/agent/harness/turn/mod.rs`, around the `eager_offload_tool_result` call |
| book: `⡌` shelf (overflow of `⠌` connected seats) | `cockpit/src/agent/harness/book/st_connected.rs` (`SHELF`, `COMPACTOR` `⡌⠁`, `COMPACTOR_DIGEST` `⡌⠃`); TOC is now 51 |
| auxiliary source | `treebeard_digest` in `auxiliary.rs` |
| tests | `tests/cockpit/harness/compactor__tests.rs`: 7 unit/integration tests and 1 ignored live test |
| docs | `cockpit/docs/ENV.md` (`ANGEL_TREEBEARD_DIGEST`); `docs/MODELS.md` § Treebeard compactor; `/connect local` |

**How it behaves:**
- **Which model digests:** `ANGEL_COMPACT_URL` when set. Otherwise a reachable
  local utility club, but only when the in-hand driver is a paid link or a
  fan-out. The in-hand driver never digests for itself.
- **Rolling, never waiting (2026-10-06):** the hop's digests run in parallel
  and the hop never waits for one. A digest that has landed by the time the
  hop's own tools finish rides its receipt under `⡌⠃`, attached before that
  message is sent. One still in flight rolls forward and, when it lands, joins
  the tail of a later request as a Harness note `⡌⠉ <handle>` (append-only, so
  the prefix cache never moves). Digests still out when the driver's turn ends
  wait in the registry (`ToolRegistry::rolling_digests`) and land at the next
  turn's first request; a bounded delegate seat never touches that stash.
  Nothing cuts a digest off. (The first cut had a 30 s deadline, removed the
  same day; the 2026-10-06 cut before this one waited for every digest, which
  let one slow or queued digest hold every session's step.)
- **Misses:** a digest that fails or comes back empty is a miss; its receipt
  stays bare, as before this change.
- **Per-hop cap:** at most `ANGEL_COMPACT_FANOUT` digests per hop (default 8).
- **Input bound:** the helper reads at most the summarizer's per-call ceiling,
  `ANGEL_COMPACT_CHUNK_TOKENS` × 4 bytes (about 48 KB). Anything larger is
  elided in the middle with the `⡨⠃⠛` mark.
- **Digest cap:** 2 KiB on the receipt, with braille runs broken by `·`.
- **Off switch:** `ANGEL_TREEBEARD_DIGEST=0`.

**The book pages are new English.** The operator may want to reword them.
Per the no-rewording rule, they have not been tuned since.

- `⡌⠁`:
  - "The root model sees a handle to the tool output below, not the output
    itself."
  - "Write the digest it needs to act without opening the handle: what the
    output is, and the paths, line numbers, names, values and errors that
    matter."
  - "Quote a line exactly only when its exact text matters."
  - "Terse bullet points, no preamble."
- `⡌⠃`: "read the exact bytes with handle_read when the digest is not enough"

## The helper model

**Recommended model:**
[unsloth/Qwen3.6-35B-A3B-GGUF](https://huggingface.co/unsloth/Qwen3.6-35B-A3B-GGUF),
file `Qwen3.6-35B-A3B-UD-Q5_K_XL.gguf`.
- 26.6 GB, Apache-2.0.
- sha256 `25233af7642e3a91bd52cc4aeefdbd4a117479088e06cf1aea5b6bedb443c506`,
  revision `a483e9e`.
- It needs llama.cpp b9743 or newer. b9672 lacks `qwen35moe`.

**Serving on sparky:**
- Script: `~/models/serve-treebeard.sh`. It runs llama.cpp
  `~/llama.cpp-qwen36` with `-np 4 --kv-unified -c 131072 --reasoning-budget 0`
  and alias `treebeard`, on `:8001`.
- It answers at `http://<helper-host>:8001/v1`.
- Log: `~/models/treebeard-serve-20261004.log`.
- It was started with `setsid nohup`, so it does not survive a reboot.
- The GGUF sits on the external HDD, so a cold load takes about 12 minutes.
- Stop it by PID (`pgrep -x llama-server`). `pkill -f …` over ssh kills the ssh
  shell too.

**The Qwen3.8-27B was a wrong first guess** and is now stopped. Its numbers:
- vLLM NVFP4: 11.5 tok/s.
- BF16: 4.4 tok/s.
- A 12k-token compaction chunk took about 124 s.
- Sparky's `angelX-mm/loop.py --ask-qwen` points at `:8001` and now gets
  treebeard. llama.cpp accepts any model name.

**Operator wiring (atlas, `~/angelX/.angel.env`, gitignored):**

```sh
ANGEL_COMPACT_URL=http://<helper-host>:8001/v1
ANGEL_COMPACT_MODEL=treebeard
ANGEL_COMPACT_REASONING_DIALECT=qwen     # served name has no "qwen3"
ANGEL_COMPACT_REASONING_EFFORT=none
ANGEL_COMPACT_BG_TIMEOUT_SECS=120
```

## Measured (Qwen3.6-35B-A3B Q5 on GB10, llama.cpp)

| what | result |
|---|---|
| decode / prefill | 55–59 tok/s / ~1,750 tok/s |
| digest of 16 KB parked output | 13.3 s, 1.8 KB digest |
| digest at the 48 KB input ceiling | 17.3 s, 1.7 KB digest |
| live test through angelX's HTTP club and connected seat | 20.0 s, landed; 2.7 KB digest capped to 2 KiB |
| 12k-token compaction chunk | 25.2 s |
| thinking on (27B) | 8+ min for one chunk; keep thinking off |

## Evidence

- **Tests:** 7 compactor tests pass. They include
  `a_parked_result_reaches_the_paid_root_with_the_local_digest`. In that test a
  scripted paid driver calls a 48 KB tool, and its next request carries the
  receipt and the `⡌⠃` digest, not the bulk.
- **Live test:** `ANGEL_TREEBEARD_LIVE_URL=http://<helper-host>:8001/v1
  ANGEL_COMPACT_REASONING_DIALECT=qwen cargo test --bin angel live_compactor --
  --ignored --nocapture`.
- **Wire check:** an `angel --ask` through a logging proxy showed
  `chat_template_kwargs.enable_thinking=false` with effort `none`.
- **Not yet seen:** a real `angel --task` that parked bulk. The local driver
  read with paged `read_file`, which never reaches the 16 KB floor. The first
  real session with a paid driver and a large shell or grep output is the
  in-session proof. Look for the `treebeard: <helper> digested N parked
  output(s)` notice.
- **Pre-existing failures:** `run_turn::loop_detectors_stamp_tool_results_and_offer_deli_at_the_stop`
  and `run_turn::yolo_turn_stamps_consecutive_tool_errors_and_keeps_going` fail
  on bare `origin/dev` (`fa72791`) and on `multi` without this change. They are
  not caused by it; they are worth a look on their own.

## Tool hand-off (2026-10-06)

A foreground shell call still running after `ANGEL_TOOL_HANDOFF_SECS` (default
600) is handed to the background-job table instead of holding the turn — the
2026-10-06 hash session sat 12h42m on one `kissat` solve. Nothing is stopped:
the call's waiter thread keeps the child and its exit channel moves to the job
(`proc::JobChild::HandedOff`); the output readers switch from a recent-output
ring to the job's rotating log under one lock (`exec/handoff.rs::Tap`). The
model gets a receipt `[handoff: job N pid P ran Ss, no output Ts]` with the
command, process tree (state, lifetime CPU%) and output tail, then `⡌⠙⠏⠙`.
The helper (any lane, same as the compactor) reads it from `⡌⠑` and its read
rolls like a digest (`⡌⠋` on the receipt or `⡌⠋ job N` at the tail). Off in
sealed tasks. The four `⡌` pages added for this (`⡌⠉`, `⡌⠙`, `⡌⠑`, `⡌⠋`) are
new English awaiting the operator's wording.

## Open

1. **Run it live with a paid driver.** Check how often the root still calls
   `handle_read` after a digest, and the token and time delta against bare
   receipts. This is a natural A/B: `ANGEL_TREEBEARD_DIGEST=0` against on.
2. **Concurrency.** Four 48 KB digests at once share 4 slots and serialized
   prefill, so a hop that parks several large results waits for all of them
   (about 20 s each alone). Watch hop latency; lower `ANGEL_COMPACT_FANOUT` or
   add server slots if it hurts.
3. **Digest cap.** At 2 KiB, a long digest loses its middle. If the root misses
   facts, try a higher fixed cap, or ask for fewer bullets in `⡌⠁`. Rewording
   that page is the operator's call.
4. **The eager floor.** It stays 16 KB. With a digest in hand, smaller results
   might be worth parking; measure before changing it.
5. **Persistence.** Make the treebeard serve a systemd user unit, and consider
   moving the GGUF off the HDD for faster restarts.

## Push log

- `multi`: `49dbb1c` (this change only; the operator's WIP was left unstaged).
- `dev`: `877fdce`, cherry-picked onto `fa72791`. On dev, the compactor,
  book, introduction, handle-store, `/connect` and `run_turn` filters gave 279
  passed and 2 failed: the two pre-existing failures above.
