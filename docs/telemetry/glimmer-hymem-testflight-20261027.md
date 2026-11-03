# TECHNICAL MEMORANDUM TM-2026-1027-GL1

## Evaluation of a Fine-Tuned Muse Glimmer 30B Auxiliary Model for the AngelX Agent Harness: Ground Tests and Test Flight 1

**Program:** AngelX × Muse — Meta Global AI Developer Hackathon 2026 (Indie Developer Track, Push the Frontier)
**Date:** 2026-10-27
**Classification:** Unclassified — public-safe campaign record
**Operator:** Jordan Newman (solo builder) · **Test conductor:** Muse Code session `ochre-andromeda`
**Repository state:** branch `muse/meta-hackathon`, HEAD `20ce1673c` (test artifacts external to cockpit source; no harness code modified)

---

### ABSTRACT

A LoRA adapter (`hymem-glimmer-v1`, rank 32) previously trained on Fireworks AI
against Meta's open-weight Muse Glimmer 30B was subjected to a first
evaluation campaign. The adapter targets four auxiliary "memory" functions of
the AngelX harness — compaction, digest, distill, and handoff (collectively,
*hymem*). Ground tests established that the base model operates locally on a
single consumer GPU (RTX 3090, 24 GB) at 41 tok/s generation and that the
untuned base, while structurally competent, violates the harness grounding
requirement in 43% of sampled compaction tasks. Export of the tuned adapter
from the training provider was found to be restricted at the account level;
Test Flight 1 therefore evaluates the adapter in situ on a dedicated
Fireworks deployment (1× NVIDIA H200), against the BF16 base model on
identical hardware. A corpus census for a future capability — procedural
generation of game enemies and items ("mechanics master") — enumerated 1,868
candidate repositories under a two-tier license policy.

### NOMENCLATURE

| Term | Meaning |
|---|---|
| hymem | compaction / digest / distill / handoff auxiliary task family |
| LoRA | low-rank adapter (rank 32 herein) |
| grounded | every hash/number in output appears verbatim in the prompt |
| Tier A | permissive-license source; code may be used in training with attribution |
| Tier B | copyleft/unlicensed source; mechanics-card distillation only, no code or text |
| GGUF Q4_K_M | 4-bit k-quantized local weights (16.8 GB) |

### 1. INTRODUCTION

AngelX routes bulky summarization work away from its frontier driver model
through a dedicated endpoint contract (`ANGEL_COMPACT_URL`, cockpit
`harness/compact.rs`), shared by background compaction and the Treebeard
parked-output digester (`harness/compactor.rs`). The objective of this
campaign is to determine whether a locally servable 30B model, fine-tuned on
operator trace data, can fill that auxiliary seat — and, prospectively, a
second seat generating game content (enemies, world items) for the delve
minigame system. Test objectives:

- **O1.** Demonstrate local operation of the base model on operator hardware.
- **O2.** Measure base-model compliance with hymem output discipline.
- **O3.** Measure the tuned adapter against the base under matched conditions.
- **O4.** Establish corpus and licensing ground truth for the mechanics-master phase.
- **O5.** Record all anomalies encountered.

### 2. APPARATUS

**2.1 Training article (pre-existing).** `hymem-glimmer-v1`: Fireworks
supervised fine-tune, completed 2026-10-21 22:48 UTC, wall time 52 min,
52,554,871 training tokens, billed USD 0.00 under the Glimmer promotional
program (free SFT through 2026-11-30). Base: `muse-glimmer-30b`
(29,776,626,688 parameters, BF16, 131,072 ctx, Apache-2.0). Adapter: LoRA
r=32, target modules q/k/v/o/gate/up/down, 1 epoch, 32,768 training ctx.

**2.2 Dataset.** `hymem-fw` v1: 8,379 train / 300 dev / 100 long-prompt eval
rows in Fireworks messages format, derived from operator harness traces.
Composition (train): compaction 3,844; digest 3,509; distill 698; handoff
328. Target provenance: GLM 7,852; hy 527. Pre-flight audit (deterministic
checkers, 43 pytest cases): 0 ungrounded hashes/numbers, 0 secrets
(294 redactions and 15 live-credential drops applied), token lengths under
the genuine Glimmer tokenizer p50 4,551 / p95 14,856 / max 23,371.

**2.3 Ground station.** Operator workstation: NVIDIA RTX 3090 (24,576 MiB),
91 GB RAM, 32 cores, driver 615.78. Runtime: llama.cpp b11936 prebuilt CUDA
12.8 binaries (no local CUDA toolkit installed). Weights and artifacts on
`<bench-root>/` (2.3 TB free) due to home-volume constraint (28 GB).

**2.4 Test range (Flight 1).** Fireworks dedicated deployment
`glimmer-testflight-1`: 1× NVIDIA H200 141 GB, BF16, addons enabled,
created 2026-10-27 17:42 UTC. GB300 shape unavailable (account quota 0);
H100/A100 rejected by the platform (model minimum 2 accelerators at 80 GB).

**2.5 Instrumentation.** `eval_hymem.py` — replays dev prompts verbatim,
scores outputs with the dataset's own grounding/secret checkers
(`hymem-fw/build/checks.py`), records tokens and latency per row.
`gen_probes.py` — five schema-constrained generation probes (2 enemy,
2 item, 1 mechanic) with numeric band validation derived from delve's
`EnemyKind` table (40 kinds) and `Arms` weapon parameterization.
Sampling: temperature 0 (hymem), 0.7 (generation).

### 3. GROUND TEST RESULTS

**3.1 Local operation (O1 — PASS).** Following correction of an anomaly
(A1), base Glimmer Q4_K_M served via llama-server at 41 tok/s generation,
449 tok/s prompt processing, 32k context, 22.8 GB total VRAM including
desktop load. OpenAI-compatible endpoint verified at `127.0.0.1:18080`.

**3.2 Base hymem compliance (O2).** n=30 compaction rows (dev file order is
not shuffled; kind coverage is a known limitation, §6):

| Metric | Base, local Q4 |
|---|---|
| Rows with ≥1 ungrounded token | 13 / 30 (43%) |
| Ungrounded tokens (total) | 61 |
| Empty outputs | 1 / 30 |
| Bulleted format | 24 / 30 |
| Secret leaks | 0 |
| Length ratio (median vs target) | 0.93 |
| Mean latency | 46.2 s/row |
| Tokens (prompt/completion) | 194,046 / 49,720 |

**3.3 Base generation probes.** 5/5 valid JSON. Enemies generated in delve
register ("Rune Mason — *It builds a wall, then bills you for the labor*";
"Mire Gullet — *It ate a knight and kept the boots for later*"). Item cards
exhibited generic naming; one band flag traced to an instrumentation defect
(A3), not the model.

**3.4 Corpus census (O4).** Fixed-query GitHub census, snapshot 2026-10-27,
window 2026-08-27+: **1,868 repositories** — 956 Claude-attributed games,
912 library/canon (engines, procgen, roguelike); Tier A 1,085 / Tier B 783;
MIT 906, unlicensed 411, copyleft ≈150. One query reached 500 of 909
results (API paging); completion by date-slicing is scheduled. A stratified
pilot of 20 repositories was selected for distillation costing.

### 4. TEST FLIGHT 1

Procedure as planned: hot-load adapter onto the H200 deployment; smoke both
arms; hymem dev-30, adapter arm then BF16 base arm (matched hardware);
generation probes, adapter arm; unload, delete deployment, capture billing.
The flight was truncated by anomaly A6 (account balance exhaustion) partway
through the base arm; all adapter-arm measurements are complete.

**Smoke check (qualitative, decisive).** Prompted to reply with one word,
the adapter returned `ready` in 5 completion tokens, `finish_reason: stop`.
The base returned empty content with the entire 32-token budget consumed by
reasoning, `finish_reason: length`. The fine-tune removed the reasoning
preamble from the response channel entirely.

| Metric | Base BF16 (H200) | LoRA v1 (H200) | Base Q4 (local, ref) |
|---|---|---|---|
| Rows completed | 5/30 (A6) | 30/30 | 30/30 |
| Rows with ≥1 ungrounded token | 2/5 | 17/30 | 13/30 |
| Ungrounded tokens (total) | 8 (n=5) | 53 | 61 |
| Empty outputs | 25 (all A6 errors) | 0 | 1 |
| Bulleted format | 5/5 real | 30/30 | 24/30 |
| Secret leaks | 0 | 0 | 0 |
| Length ratio (median) | 1.00 | 0.91 | 0.93 |
| Mean latency | 10.5 s (n=5) | 7.1 s | 45.6 s |
| Completion tokens (total) | — | 17,382 | 49,720 |

**Paired analysis, adapter vs local Q4 base (29 rows both real):**
ungrounded tokens 51 vs 61; rows affected 16 vs 13; per-row wins 8 vs 9
with 12 ties. **Grounding was not materially improved by the fine-tune.**
Completion tokens 16,541 vs 47,412 — a 65% reduction, attributable to
elimination of reasoning overhead. On the matched-hardware subset (n=5)
the same pattern holds: comparable grounding, adapter completions 2–3.5×
shorter, lengths nearer target.

**Generation probes (adapter arm): not flown.** All five probes returned
HTTP 412 following A6. No adapter generation data exists; base-model probe
results of §3.3 stand as the only generation baseline.

**Flight cost:** USD 1.28 total (577 accelerator-seconds recorded at first
billing capture; deployment verified DELETED after forced teardown, A7).
Account balance at flight end: USD 0.00.

### 5. ANOMALY LOG

- **A1 — Silent CPU fallback.** Prebuilt llama.cpp failed to load
  `libggml-cuda` (cudart absent from library path); throughput 3.0 tok/s
  presented without error. Corrected via `LD_LIBRARY_PATH` to the bundled
  cudart tree. *Lesson: assert device placement in the harness, never infer
  from load success.*
- **A2 — Adapter export restricted.** `firectl model download` returns
  `FailedPrecondition: model downloading is restricted`. Local serving of
  the tuned article is blocked pending provider access grant; the dataset
  is portable and a Modal retrain is the designated contingency.
- **A3 — Validator band error.** Enemy locomotion speed band (0.2–8.0)
  was incorrectly applied to weapon projectile speed in probe scoring.
  Model output was within the correct band. Validator corrected in record;
  probe re-runs will use per-domain bands.
- **A4 — Accelerator quota.** Default GB300 deployment shape: account
  quota 0. Flight proceeded on H200.
- **A5 — Serverless unavailability.** Base Glimmer supports neither
  serverless inference nor serverless addons on Fireworks; dedicated
  deployment is mandatory for any tuned-model evaluation.
- **A6 — Balance exhaustion cut power mid-flight.** The account entered
  the flight with USD 1.00. Inference began returning HTTP 412
  (Precondition Failed) after base-arm row 5 and for all subsequent calls,
  including the generation probes. No error indicated the cause; balance
  was found at USD 0.00 on inspection. *Lesson: preflight budget is a
  flight-safety item; arm the cheapest and most decisive measurements
  first (done by happenstance herein — the adapter arm flew first).*
- **A7 — Teardown initially failed; vehicle left on the pad.** LoRA
  unload was refused ("inference requests in the last hour") and the
  scripted delete did not execute as intended. The deployment was found
  live (UPDATING, 1 replica ready) during data reduction and was
  force-deleted with `--ignore-checks`; DELETED state verified.
  *Lesson: teardown requires positive confirmation, not script exit 0.*
- **A8 — Dataset uploads disabled.** `firectl dataset create` returns
  `FailedPrecondition: dataset uploads are temporarily disabled`. New-data
  iteration (DPO pairs, rebalanced SFT, game rows) is provider-gated;
  hyperparameter iteration on the v1 dataset remains available and free.
- **A10 — LoRA load refuses PARTIAL_READY.** `load-lora` returns
  FailedPrecondition until the deployment reaches full READY, although
  inference already serves. Flight 2's first arm was voided thereby
  (quarantined as `*.INVALID-partialready.jsonl`); the refly waited for
  a ready replica plus a successful explicit load before evaluating.
  *Lesson: gate every measurement stage on its own success signal; a
  pipeline that continues past a failed load manufactures empty data.*
- **A9 — Training is not balance-gated.** A new 3-epoch SFT job was
  accepted and running at USD 0.00 balance under the promotional program,
  while inference refuses at zero (A6). Plan accordingly: experiments that
  end in a trained artifact are free; experiments that end in a
  measurement cost deployment time.

### 6. LIMITATIONS

Dev-30 covers only the compaction kind; per-kind base performance was
later measured (Addendum C); per-kind tuned performance awaits the v2
evaluation flight. Grounding is scored against the
prompt as presented; semantic fidelity (omission of material facts) is not
yet instrumented. Flight-1 adapter results characterize the BF16 hosted
article; fidelity after 4-bit local quantization remains to be measured
when export (A2) is resolved.

### 7. CONCLUSIONS AND RECOMMENDATIONS

1. **The auxiliary-seat concept is validated; the v1 article is not yet
   flightworthy.** One epoch of rank-32 SFT produced perfect format
   discipline (30/30 bulleted, 0 empties, 0 leaks), near-target lengths,
   and 65% fewer generated tokens than base — but grounding, the
   property the harness depends on most, was statistically unchanged
   (paired wins 8–9). Verbatim copying of hashes and figures appears to
   be a capability SFT alone did not instill at this scale of training.
2. **Local serving is unconditionally feasible for the base vehicle**
   (41 tok/s, single consumer GPU) and the harness endpoint contract
   requires no modification. Adapter export (A2) is the sole blocker
   to local tuned operation; pursue the provider access grant and, in
   parallel, price the Modal retrain (weights downloadable) — training
   itself is a 52-minute, 52.6M-token event, trivially repeatable.
3. **Recommended next test article (v2):** (a) 2–3 epochs with stratified
   sampling across all four hymem kinds; (b) preference tuning (DPO)
   using the grounding checker as the preference oracle — rejected
   samples are precisely the flight-recorded ungrounded outputs;
   (c) retain the harness-side verify-and-repair option (deterministic
   checker + near-miss repair already implemented in `checks.py`) as a
   defense-in-depth layer regardless of model scores.
4. **Flight discipline:** no future flight lifts without (i) confirmed
   budget ≥10× predicted burn, (ii) teardown verified by state query,
   not exit code, (iii) stratified eval sampler covering all kinds.
5. **Mechanics-master phase remains on plan.** Base generation is
   schema-compliant and stylistically close; the 1,868-repo census and
   pilot-20 selection are recorded and ready for the distillation
   costing pass (teacher expenditure pending operator authorization).

### ERRATUM (2026-10-27, post-flight data reduction)

The grounding figures in §3–§4 aggregate all six checker token types.
Re-examination against the dataset's own enforcement contract
(`DROP_TYPES = {hash, number}` — the only types vetted at dataset build)
yields a materially different reading:

| Ungrounded by type (dev-30) | hash+number rows | path tokens | codespan tokens |
|---|---|---|---|
| Gold targets | 0/30 | 5 | 71 |
| Base Q4 local | 0/29 | 23 | 38 |
| LoRA v1 (H200) | 1/30 (1 token) | 3 | 49 |

Codespan "ungrounded" is largely a citation-format artifact: a composed
`file.rs:139` reference is never verbatim in the prompt even when the file
and line both appear, and gold targets compose such citations freely
(19/30 rows). On the enforced classes both arms are near-perfect; on
paths the adapter improves on base by ~7× (3 vs 23 tokens). Conclusion 1
of §7 is therefore revised: the v1 article **does** ground materially
better than base where grounding is well-defined; the residual problem
class is citation composition, which current instrumentation cannot
distinguish from invention. Checker upgrade (parse `file:line` citations
against the prompt's numbered listings) is entered as a v2 prerequisite
— measuring the wrong quantity precisely is worse than not measuring.

### ADDENDUM B — MECHANICS CARD PILOT AND V2 TRAINING (2026-10-27 EVENING)

**Card distillation pilot, complete.** Four subagent distillers processed
the stratified pilot-20 in parallel under a fixed card specification
(`game-fw/CARD-SPEC.md`: own-words description only; no source code, text,
or names reproduced; `source_api` carries factual file pointers only).
Yield: **87 cards, 20/20 repos covered, 0 parse errors, 0 duplicate IDs**;
Tier A 67 / Tier B 20. Domain distribution is library-shaped: procgen-world
17, tooling 17, enemy-ai 12, audio-fx 10, procgen-content 8, combat 4,
economy 2, spawning 0. The next harvest wave must weight complete games
over libraries to fill combat/spawning/economy. Consolidated corpus:
`<bench-root>/game-fw/cards.jsonl` with sha256 manifest.

**Preference dataset built.** 3,000 DPO pairs (`hymem-v2/dpo-pairs-v2.jsonl`,
converted to Fireworks schema in `dpo-fireworks-v1.jsonl`): chosen = gold
target, rejected = deterministic corruption of hash/number/path tokens
(digit swap 2,831; unit suffix 2,042; line shift 348), admission verified by
the strict checker. **Upload was refused** (A8): dataset uploads are
account-gated, same as model downloads.

**V2 training article.** Job `g2j7vyjl` (output `hymem-glimmer-v2-3ep`):
v1 recipe copied verbatim via `--source-job`, single variable changed —
epochs 1 → 3. Accepted at USD 0 balance under the free-SFT program (A9:
training is not balance-gated; inference is). Reference v1 curve: eval loss
0.798 → 0.515, 262 steps, LR 1e-4 constant, ~17.4k tok/s. Flight evaluation
of v2 awaits funding (balance USD 0.00; see A6).

### ADDENDUM C — STRATIFIED BASELINE, ALL KINDS (2026-10-27 EVENING)

The §6 limitation (dev-30 covered compaction only) is closed for the base
article. Local Q4 Glimmer, 10 dev rows per kind, temperature 0, strict
metric = ungrounded hash/number/path tokens:

| Kind | strict rows | strict tok | empty | bulleted | len median | completion tok |
|---|---|---|---|---|---|---|
| compaction | 2/10 | 7 | 1 | 8/10 | 0.79 | 20,744 |
| digest | 0/10 | 0 | 0 | 10/10 | 0.86 | 12,895 |
| distill | 0/10 | 0 | 0 | 6/10 | 1.11 | 26,908 |
| handoff | 0/10 | 0 | 0 | 10/10 | 0.70 | 20,226 |

Reading: strict-grounding failures in the base concentrate in compaction;
digest/handoff are format-clean; distill is the format laggard (6/10
bulleted) and the most token-expensive; handoff runs short (0.70× target
median). These are the per-kind bars v2 must clear on its evaluation
flight. Data: `<bench-root>/eval/base-strat40.jsonl`.

### ADDENDUM D — V2 TRAINING ARTICLE, FINAL METRICS (2026-10-27 NIGHT)

Job `g2j7vyjl` (`hymem-glimmer-v2-3ep`, RECIPE = v1 verbatim, epochs 1→3)
reached `JOB_STATE_COMPLETED` at 2026-10-27 20:48 local (wall 2 h 33 min,
147,055,467 training tokens, billed USD 0 under the promotional program;
account day total USD 1.50 = Test Flight 1 deployment). Output model
state READY, HF_PEFT_ADDON, R=32. Adapter download remains gated (A2).

| Eval loss | step 0 | epoch 1 (262) | epoch 2 (524) | epoch 3 (786) |
|---|---|---|---|---|
| v1 (1 epoch) | 0.7979 | **0.5149** | — | — |
| v2 (3 epochs) | 0.7979 | 0.5218 | **0.5113** | 0.5331 ↑ |

Train loss, v2 final 0.393 vs v1 final 0.499 — the third epoch bought
memorization, not generalization: **eval loss turns upward in epoch 3**.
Findings:

1. The optimum on this dataset sits at ~2 epochs (0.5113), a margin of
   only 0.004 over the 1-epoch article. Additional epochs are not the
   lever for this task.
2. Behavioral (stratified, checker-scored) comparison of v2 vs v1 was
   **not flown**: evaluation requires a funded dedicated deployment and
   the account balance is USD 0.00 (A6). The claim for v2 is therefore
   limited to dev-set loss; no behavioral superiority is asserted.
3. Next-article recommendation stands as §7 item 3, sharpened: keep SFT
   at 1–2 epochs; spend remaining effort on the preference stage
   (3,000 pairs built, awaiting upload access A8 or the Modal path) and
   on the citation-aware checker needed to measure the residual class.

### ADDENDUM E — TEST FLIGHT 2: STRATIFIED A/B, V1 VS V2 (2026-10-28)

Funded by an operator top-up (USD 15; balance at wheels-up 14.49, at
shutdown 13.89 — **both sorties together cost USD 0.60**). One H200
deployment per sortie, LoRA hot-loaded, stratified instrument (10 rows per
kind, strict metric = ungrounded hash/number/path), generation probes,
verified DELETED teardown after each. Anomaly: the v2 arm of the first
sortie is void — `load-lora` refuses a PARTIAL_READY deployment (A10);
the arm was reflown cleanly as Flight 2B after full readiness.

| Stratified-40 totals | Base (Q4, 3090) | v1 1-epoch (H200) | v2 3-epoch (H200) |
|---|---|---|---|
| Strict-ungrounded rows | 2 | 3 | 3 |
| Strict-ungrounded tokens | 7 | 4 | 4 |
| Empty outputs | 1 | 0 | 0 |
| Bulleted | 34/40 | 40/40 | 40/40 |
| Completion tokens | 80,773 | 25,645 | 23,651 |

Paired v2-vs-v1 (40 rows): strict wins 1–1, ties 38. **Behaviorally the
3-epoch article is indistinguishable from the 1-epoch article** on this
instrument, corroborating the eval-loss plateau (Addendum D): epochs
beyond the first buy token thrift at the margins (handoff length 0.70 →
0.86 → 1.03 across base/v1/v2) and nothing else measurable. Both tuned
articles dominate the base on discipline and cost: ~3.2× fewer completion
tokens and zero format failures. Generation probes: all three arms 5/5
schema-valid; tuned articles retain general generation; novelty against
the existing 40 kinds remains weak in every arm (e.g. "Mimicrite",
repeated "Ricochet" cards) — originality is a data problem, addressed by
the mechanics-card corpus, not by hymem tuning. **Recommendation: freeze
the SFT recipe at 1–2 epochs; proceed to the preference stage.**

### APPENDIX — ARTIFACT INDEX

- Eval harness: `<bench-root>/eval/{eval_hymem.py, gen_probes.py, flight1.sh}`
- Ground data: `<bench-root>/eval/{base-dev30.jsonl, base-gen.jsonl}`
- Flight data: `<bench-root>/eval/{fw-lora-dev30.jsonl, fw-base-dev30.jsonl, fw-lora-gen.jsonl}`
- Census: `<bench-root>/game-fw/{census.py, census.jsonl, census-manifest.json, pilot20.jsonl}`
- Dataset: `~/datasets/hymem-fw/` (manifest sha256 in `reports/manifest.json`)
- Fireworks job: `accounts/<account>/supervisedFineTuningJobs/hymem-glimmer-v1`
- Cards: `<bench-root>/game-fw/{cards.jsonl, cards-manifest.json, CARD-SPEC.md, cards/slice-{A,B,C,D}.jsonl}`
- DPO pairs: `<bench-root>/hymem-v2/{build_dpo.py, dpo-pairs-v2.jsonl, dpo-fireworks-v1.jsonl}`
- Metrics: `<bench-root>/metrics/v1-metrics.jsonl`
- Fireworks job v2: `accounts/<account>/supervisedFineTuningJobs/g2j7vyjl`
- Metrics final: `<bench-root>/metrics/v2-metrics-final.jsonl`
- Stratified baseline: `<bench-root>/eval/base-strat40.jsonl`
- Flight 2 data: `<bench-root>/eval/{fw-v1-strat40, fw-v2-strat40, fw-v1-gen, fw-v2-gen}.jsonl`, logs in `<bench-root>/logs/flight2.log`
