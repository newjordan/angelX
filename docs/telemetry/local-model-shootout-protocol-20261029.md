# Local-model shootout protocol — the optimal local model to run with AngelX

Written 2026-10-29, the day `glimmer-comb1` (combined Genie+hymem LoRA on
Muse-Glimmer-30B, Modal run `ap-9ziTckXaRIj0lEWwT9QT5x`, adapter sha256
`2074380f…187ad3`) first served from the DGX Spark host (GB10, bolt llama.cpp :8002).

## 0. The question, and how it gets decided

"Optimal local model to run with AngelX" is FOUR seats, not one crown.
A contender may win one seat and lose the rest; record per-seat winners.

- [ ] **Driver** — the in-hand cockpit model a human drives (`ANGEL_DRIVER`).
- [ ] **Compactor** — the Treebeard-lane rolling digest + background
      compaction writer (today: treebeard/Qwen3.6-35B-A3B via :8011).
- [ ] **Genie** — the wish-granting seat (Knights Wish spelling/granting).
- [ ] **Hymem** — the memory-article seat (bulleted, grounded, token-thrifty).

Decision rule: per-seat Pareto (quality per token / per minute / per watt),
paired statistics against the incumbent, no win claimed on point estimates
alone. Ties break toward: lower fault burden, smaller footprint, ours
(reproducible weights beat a download we cannot retrain).

## 1. Contenders — pin everything before any run

- [ ] **glimmer-comb1** — base `Muse-Glimmer-30B-KQuant-17GB-Q4_K_M.gguf` +
      `glimmer-comb1-f16.gguf` (sha256 `e35b8ab5…9426c`), on the Spark host
      `~/models/serve-glimmer-comb1.sh`, bolt llama.cpp, ctx 32768, np 1.
- [ ] **Base Glimmer-30B Q4_K_M ablation** — same GGUF, no `--lora`.
      This is the control every LoRA claim is measured against.
- [ ] **Gemma-4-26B-A4B** (Spark host `~/models/Gemma-4-26B-A4B-NVFP4`) — record
      serving stack + quant exactly as flown; NVFP4 vs Q4_K_M is a
      different-quant comparison, say so in every table.
- [ ] **treebeard (Qwen3.6-35B-A3B UD-Q5_K_XL)** — incumbent compactor,
      the Spark host's `:8001` direct (not the :8011 router, no overflow mixing).
- [ ] Optional extra Spark-host residents only if a seat is unclaimed:
      DeepSeek-V4.1-Flash, Ling-3.0-flash-int4. No new downloads mid-shootout.
- [ ] For every contender record: weights file + sha256, server build
      commit, full serve command, ctx, slots, chat-template source.
- [ ] Template parity check per contender: render one fixed conversation
      through the server and through the reference tokenizer template;
      diff byte-for-byte (comb1 already proven: HF template == GGUF
      template, 9,992 B).

## 2. Fairness rules (apply to every suite below)

- [ ] Identical prompts, identical order, identical harness binary
      (bound binary discipline from the Pareto baseline) for all contenders.
- [ ] Sampling pinned per suite and printed in the results header
      (temp/top_p/seed/max_tokens); genie + hymem suites at their campaign
      settings so old scoreboards stay comparable.
- [ ] Held-out sets stay frozen: GENIE-EVAL v2 (n=131), hymem strat-40,
      slang memfree (n=58). Nothing tuned, prompted, or patched against
      them during the shootout.
- [ ] Report Wilson 95% CIs and paired exact tests (discordant pairs) —
      the campaign format in `genie-rnd/CAMPAIGN.md`, not bare percentages.
- [ ] Every raw output saved (jsonl per contender per suite, `exp/` style)
      so any number can be recomputed by a skeptic, including us later.
- [ ] Fault taxonomy counted, not just pass rate: total fault strings and
      worst failure class per contender (a worse failure class loses ties).
- [ ] One contender at a time on the GB10 for perf numbers; quality runs
      may share only if memory headroom is printed with the result.

## 3. Suite A — Driving AngelX (the headline seat)

- [ ] Polyglot subset16 first (`tasks-subset16.json`, bound binary):
      pass n/16, wall minutes, output tokens, prompt-cache %.
- [ ] Full polyglot-136 for the top two driver contenders only.
- [ ] Tool-call validity: parse-failure count, malformed-edit count,
      apply-success rate of proposed edits (harness logs, not vibes).
- [ ] Loop behavior: tasks completed without stall/`says_done` false-done,
      wrap-up compliance, turns-to-done distribution.
- [ ] Long-session drive: one fixed 30-turn scripted session per contender;
      count context-loss events (contradicts own earlier statement).
- [ ] Instruction-following spot suite: 20 fixed hard-format asks
      (exact-JSON, diff-only, word-limits); score format-valid %.
- [ ] Refusal sanity: 5 benign-but-edgy asks (over-refusal count) and
      5 clearly-out asks (compliance count). Record, don't moralize.

## 4. Suite B — Genie seat

- [ ] GENIE-EVAL v2 (n=131): pass (grantable AND literal), grantable %,
      literal %, total faults, per-kind breakdown (cast/hold/guard/…).
- [ ] Compare comb1 against the frozen champion line: cell2
      `genie-art1-182432` pass 77.1% / grantable 87.8% / 44 faults.
      Ship gates (pass ≥90%, grantable ≥98%) are NOT met by any cell yet;
      print distance-to-gate for every contender.
- [ ] Slang memfree (n=58) same scoring.
- [ ] Non-glimmer contenders fly zero-shot with the campaign prompt —
      that asymmetry is the point (a general model vs a trained seat),
      and must be labeled as such wherever numbers are quoted.
- [ ] Latency per wish at np=1 (a live demo cares): p50/p95 seconds.

## 5. Suite C — Hymem seat

- [ ] Strat-40: strict-ungrounded rows, token count, empty count,
      bulleted n/40, completion tokens total. Bars to beat (Addendum E):
      hymem v1 = 3 ungrounded / 40-40 bulleted / 25,645 completion tok;
      base Q4 = 2 / 34-40 / 80,773.
- [ ] Groundedness audit sample: 10 articles hand-checked against source
      spans (automatic counters miss fluent fabrication).
- [ ] Token thrift IS the claim: report completion tokens per article
      next to every quality number, never quality alone.

## 6. Suite D — Compactor seat (Treebeard lane)

- [ ] Digest faithfulness: fixed set of parked tool outputs (small/12k/
      40k tokens); score = downstream QA — can a reader answer 5 fixed
      questions per source from the digest alone? Plus handle-recall:
      facts present in source, absent or altered in digest (count both).
- [ ] Compression ratio and seconds per 12k-token chunk, single-slot
      (incumbent reference: ~25 s/chunk noted in `~/.angelX/angel.env`).
- [ ] Background compaction: run AngelX's own bg-compaction path against
      each contender endpoint; measure timeout rate at 120 s and
      summary-survival (does the driver later contradict the summary?).
- [ ] Reasoning-off discipline: contender must not leak thinking into
      digests at the pinned settings (count leaked-reasoning digests).

## 7. Suite E — Context ladder

- [ ] Needle + instruction retention at 8k / 16k / 32k (comb1 trained at
      seq 18432 — expect degradation past it; measure, don't assume).
- [ ] One 18k-token hymem-shaped prompt per contender: completion
      validity at comb1's home distance.
- [ ] Prompt-processing tok/s at each rung (long-context driving is
      prefill-bound in practice).

## 8. Performance & operations on the GB10 (the DGX Spark host)

- [ ] Generation tok/s and TTFT at ctx 2k/16k/32k, np=1, alone on the box.
- [ ] Co-residence: same numbers while treebeard (:8001) is serving —
      the real operating picture (61 GiB available at last probe).
- [ ] Footprint: process RSS + GPU/unified memory at idle and at full ctx.
- [ ] Concurrency: np=2/4 if a seat wants slots (compactor does);
      tok/s per slot and total.
- [ ] Cold load time (service start → first 200 from `/v1/models`).
- [ ] 30-minute soak at mixed load: OOMs, crashes, tok/s drift, log errors.
- [ ] Power draw if measurable (`nvidia-smi` watts): quality per watt is
      the local dollar. The Spark host's disk is 99% full (42 G free) — print
      `df -h` in every run header; no contender installs mid-shootout.

## 9. Quant & adapter-format sensitivity (comb1 only)

- [ ] Adapter GGUF f16 (shipping) vs q8_0: GENIE-EVAL pass delta.
- [ ] Base Q4_K_M vs a higher quant IF disk allows later — deferrable,
      mark as such; never block the shootout on it.
- [ ] LoRA-scale sanity: scale 0.0 must reproduce base-ablation outputs
      on 5 fixed prompts (proves the adapter path, not luck).

## 10. Scoreboard & decision

- [ ] One table per seat: quality (CI), faults, tokens, minutes,
      footprint, tok/s. Pareto chart in the house style
      (`muse-pareto-baseline-20261026.md` precedent).
- [ ] Winner per seat named with the paired-test p-value vs incumbent;
      "no significant difference, keep incumbent" is a valid verdict.
- [ ] Update `~/.angelX/angel.env` club entries to the winners; losers
      keep serve scripts but leave the default line.
- [ ] Closeout note in this directory + `genie-rnd/CAMPAIGN.md`;
      record the two adapter facts plainly:
      vision-tower LoRA pairs exist in the safetensors, untrained
      (text-only mix) and excluded from the text GGUF; PEFT's
      `gate_proj` also trained `self_attn.gate_proj` (attn gate) —
      both facts belong in the model card, not in a footnote's footnote.

## 11. HuggingFace release laundry list (glimmer-comb1)

- [ ] **License first**: read `meta-models/Muse-Glimmer-30B` license in
      full; confirm adapter-weight redistribution terms and required
      notices BEFORE uploading anything. Record the clause, not a vibe.
- [ ] **Data provenance**: cite `genie-hymem-combined-v2` manifest —
      teacher sources, synthetic vs human rows, license per source;
      art rows are frozen/out (cell2's PixilArt/Kenney are NOT in comb1);
      hymem corpus rights stated explicitly or the dataset stays private.
- [ ] **Model card**: base model, recipe (r32/α64, 2ep, lr1e-4, eff.
      batch 32, seq 18432, assistant-only loss), intended seats,
      eval tables WITH CIs and unmet ship gates, limitations
      (ctx >18k degradation per Suite E, vision excluded, Q4 base).
- [ ] Files: `adapter_model.safetensors` + config, GGUF f16 + q8_0,
      `chat_template.jinja` (parity note), `SHA256SUMS`, README with the
      exact llama.cpp serve command from `serve-glimmer-comb1.sh`.
- [ ] Reproduction: publish or link `modal_train.py`, the converter
      patch + its regression test (`test_glimmer_lora_unpermute.py`,
      upstream-PR-worthy: `conversion/muse_glimmer.py` LoRA unpermute),
      and the campaign scoreboard. Weights without a recipe are a demo;
      with one, they're a result.
- [ ] Versioning: `glimmer-comb1` is v1 of the combined line; name the
      repo so comb2 doesn't need a rename. Tag the AngelX release
      (0.3.0) that first ships it as a club recommendation.
- [ ] Post-upload verification: fresh-download GGUF from HF on a clean
      directory, sha256 check, serve, 3-prompt smoke, compare outputs
      to the Spark-host reference at temp 0.

## 12. Execution order & budget

1. Pins + template parity (§1–2) — one sitting, $0.
2. Suites B + C (our seats, frozen sets, existing scorers) — hours on
   the Spark host, $0 API. This is the freeze doc's step-6 re-flight for comb1.
3. Suite A subset16 all contenders → full-136 top two.
4. Suites D + E.
5. §8 perf block (box alone), then §9.
6. §10 scoreboard → §11 HF checklist in order, license read before upload.

Stop rules: any contender that fails template parity or crashes the soak
is out of that seat (record why); any suite whose scorer disagrees with a
10-item hand audit gets fixed before its numbers count.

