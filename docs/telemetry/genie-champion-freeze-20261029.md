# Genie champion selection + art-training freeze — 2026-10-29

Operator decision 2026-10-29: do not risk the hackathon on art training.
Shore up Genie / cache / memory (hymem) / execution service and quality.
Art training is FROZEN (below). Fireworks finishes the recipe search;
ONE Modal retrain reproduces the winner as downloadable weights
(Fireworks `firectl model download` is account-gated, TM anomaly A2).

Evidence: `<bench-root>/genie-rnd/CAMPAIGN.md` (scoreboard),
`<bench-root>/genie-rnd/exp/{pilot1,cell1..5}-evalv2.jsonl`
(recomputed 2026-10-29, same 131 wishes in identical order),
`docs/telemetry/glimmer-hymem-testflight-20261027.md` Addenda C/E.

## ART FREEZE (effective 2026-10-29, operator)

- NO new art SFT cells, NO art rows added to any training mix, NO
  art-terra sheet generations for training, NO VL/refimg training.
- Frozen artifacts stay on disk untouched (art-sft-*.jsonl,
  art-terra sheets/refs, ART-EVAL) for post-hackathon / 0.3.0 use.
- Deterministic code is NOT frozen: `inkify.py` (CC0 PNG -> ink)
  and checker/eval scripts may run offline at $0.
- Existing spend freeze still stands: no flights, training
  launches, or GLM/teacher calls until the operator lifts it
  for a named run. Verified idle 2026-10-29: no train/flight/
  teacher processes; Cell 6 remains UNSCORED (Flight 9 addon
  500, incident A12) and is NOT carried forward.

Why: art cells regressed Genie (cell3 pass 77.1->69.5%), diluted
animate/verb scores (cell5), burned balance $13.89->$4.66 in
lagged flight billing, and earn no judging points directly.

## Champion statistics — GENIE-EVAL v2 (n=131 held-out, frozen)

pass = valid (grantable) AND literal. Recomputed from raw exp files.

| Cell | Training mix | Pass | Grantable | Literal | Pass 95% CI | Total faults |
|---|---|---|---|---|---|---|
| pilot1 | 322 rows | 64/131 48.9% | 52.7% | 78.6% | [40.5, 57.3] | 234 |
| cell1 `genie-v1-130327` | v1-mix 1,247 (genie only) | 90/131 68.7% | 77.1% | 83.2% | [60.3, 76.0] | 137 |
| **cell2 `genie-art1-182432`** | **v1.1 3,874** | **101/131 77.1%** | **87.8%** | 86.3% | **[69.2, 83.5]** | **44** |
| cell3 `genie-art2-192822` | +PixilArt | 91/131 69.5% | 77.1% | 89.3% | [61.1, 76.7] | 84 |
| cell4 `genie-art3-211614` | quality-art 2,838 | 99/131 75.6% | 84.7% | 88.5% | [67.6, 82.1] | 57 |
| cell5 `genie-topup1-223420` | +fault-topup 3,638 | 100/131 76.3% | 87.0% | 84.7% | [68.4, 82.8] | 92 |

Paired exact tests (same 131 wishes, discordant pairs):

- cell2 vs cell5: 13 vs 12 discordant, p=1.000 — TIE on pass
- cell2 vs cell4: 10 vs 8, p=0.815 — TIE on pass
- cell5 vs cell4: 14 vs 13, p=1.000 — TIE on pass
- cell2 vs cell1: 18 vs 7, p=0.043 — cell2 beats genie-only cell1
- cell2 vs cell3: 19 vs 9, p=0.087 — trend only

Tie-breakers (why cell2, not cell4/cell5):
1. Highest point estimates: pass 77.1%, grantable 87.8%.
2. Lowest fault burden by 2x: 44 total fault strings (16 faulty
   entries) vs 57 (cell4) and 92 (cell5). Cell5's top-up erased
   name/id faults but created kind confusion (`spell tier needs
   cast...` x32, `cast is not a spell line` x15) — a worse
   failure class for a live demo.
3. Simplest lineage to reproduce: v1-mix + art-v0 + art-kenney,
   r32/2ep/lr1e-4/bs32/seq2048. NOTE: cell2>cell1 confounds
   art content with 3.1x more rows/steps; do not claim "art
   caused the gain." Under the freeze we keep cell2's *recipe
   and scale*, not new art data (see Modal dataset below).

Ship gates NOT met by any cell: pass >=90%, grantable >=98%.
Cell2 is the champion to *carry forward and improve*, not a
ship claim. Distance to gate: +17 passes, +14 grantable /131.

## Champion statistics — HYMEN / memory (stratified-40)

From TM-2026-1027-GL1 Addendum E (Flight 2, H200, $0.60 total):

| Article | Strict-ungrounded rows | Tokens | Empty | Bulleted | Completion tok |
|---|---|---|---|---|---|
| Base Q4 (3090) | 2 | 7 | 1 | 34/40 | 80,773 |
| **hymem v1 (1 epoch)** | 3 | 4 | 0 | **40/40** | 25,645 |
| hymem v2 (3 epochs) | 3 | 4 | 0 | 40/40 | 23,651 |

Paired v2-vs-v1: 1-1-38 — indistinguishable; eval loss shows
epoch 3 overfits (0.511 @2ep -> 0.533 @3ep). CHAMPION (memory):
**hymem v1 recipe at 1-2 epochs** (v1 artifact `hymem-glimmer-v1`;
2 epochs is the loss optimum, 1 epoch is behaviorally equal and
cheaper). Both tuned articles: 3.2x fewer tokens than base,
zero format failures — that token thrift IS the cache/memory
Pareto claim. Next lever is DPO (3,000 pairs built, upload-gated
A8), not more epochs.

## Critical gap — read before any Modal spend

NO single artifact carries Genie + hymem together. Genie cells
were never flown on hymem strat-40; hymem articles were never
flown on GENIE-EVAL. "Cache" is a harness property (prompt-cache
74.4% on subset16), not a LoRA skill. Therefore:

- Carry forward TWO champions (cell2 recipe for Genie seat,
  hymem-v1 recipe for memory seat), OR
- Train ONE combined mix (genie + hymem-replay) — the campaign's
  own line 175 design — which has never been trained or flown.
  A combined model is a NEW experiment, not a reproduction.

Recommendation: finish on Fireworks with one combined-mix cell
(genie v1-mix expanded kind-balanced + hymem replay slice,
NO new art rows; cell2 hparams), dual-fly it (GENIE-EVAL v2 +
hymem strat-40 on one deployment), and only then spend Modal
credits reproducing the winner. If the combined cell degrades
either skill vs its champion, ship two LoRAs / two seats.

## Fireworks finish regime -> Modal retrain -> download

Fireworks (free SFT, recipe search only; each step operator-gated):
1. Kind-balanced genie data fix (offline, checker-gated):
   hold/guard generation wave + cast-grammar rows from existing
   pools; every row passes genie validate + literal. No GLM
   teacher calls until cache-resume proven on 2 wishes.
2. ONE combined cell (above), r32/2ep/lr1e-4/bs32/seq2048.
   Delete training session after promotion (A11).
3. ONE foreground flight, balance printed before takeoff,
   DELETED-VERIFIED after: GENIE-EVAL v2 + hymem strat-40 +
   slang memfree (n=58) dual read. Budget gate: balance >=10x
   predicted burn (~$0.60-1.60/flight observed).

Modal (exactly one winning reproduction, weights downloadable):
4. Parity gate (free, do first): diff Fireworks `muse_glimmer`
   renderer vs tokenizer chat template token-for-token on one
   example; record in CAMPAIGN.md. `modal/modal_train.py`
   already encodes r32/2ep/lr1e-4/eff.batch32/seq2048/H200.
   Modal auth present (`~/.modal.toml`); `modal` package not
   installed in system python — install in project venv.
5. `modal run modal_train.py --dataset <winning.jsonl>
   --out-name glimmer-genie-final` (operator word = spend).
6. Download adapter (safetensors+config) from the Modal volume,
   SHA256 it, serve locally (llama.cpp 41 tok/s proven), re-fly
   both evals against the LOCAL artifact. Only local-artifact
   numbers go in the submission.

## Databricks (operator: API accepted)

Yes — three dataset surfaces, none of which replace our own
checker-gated corpora (full note in chat 2026-10-29):
`samples` catalog sample datasets, Databricks Marketplace
(Delta Sharing data products, free samples + commercial), and
Hugging Face datasets loadable into Unity Catalog volumes for
Foundation Model fine-tuning. The Oct 28 corpus survey already
found no pixel-art data there (dolly-15k/officeqa class only) —
irrelevant under the art freeze. Candidate value for us:
general instruction/preference data (e.g. dolly-style) for DPO
replay -- only if license-clean and checker-admissible.
