---
license: apache-2.0
library_name: transformers
pipeline_tag: image-text-to-text
language:
- en
base_model:
- microsoft/Mage-VL-Cowork
tags:
- vision-language-model
- computer-use
- gui-grounding
- mage-vl-cowork
- angelx
---

# Mage Hand — Mage-VL-Cowork-hands-v2

![Mage Hand: a spectral golden hand points to a single point on a dark slate (cover art generated for this release with OpenAI Codex ImageGen)](assets/mage-hand-banner.png)

**Mage Hand** is the moniker; the full model name is
**Mage-VL-Cowork-hands-v2**. A GUI-grounding fine-tune of
[microsoft/Mage-VL-Cowork](https://huggingface.co/microsoft/Mage-VL-Cowork)
for [angelX](https://github.com/newjordan/angelX) computer use:
given a screenshot and a target description, return the click point.
Released with angelX 0.3.0, the Muse Hackathon Update
(Meta Global AI Developer Hackathon 2026).

## What this is

Base `Mage-VL-Cowork` + the `cand1/final` LoRA adapter merged
(r=64, alpha=128, scale 2.0, 252 language-model linears + merger
tensors swapped; merge script `train/merge_lora.py` in the angelX
repo). Trained on desktop/web grounding data collected for the
angelX hands tools (see `scripts/bench/mage-hands/` in the repo).
Continue-trained from the v1 adapter: 1,195 steps, 9,557 samples
(~0.8 epoch), mean loss 0.0689.

## Evaluation (full holdout, offline single-click grounding)

1,258 held-out rows, identical evaluator and row order for both
models. Primary metric: predicted point inside the target box.

| Model | Target-box hit | Within ±15 of centre | Parsed |
|---|---:|---:|---:|
| hands v1 | 88.08% | 78.70% | 100% |
| **hands v2 (this)** | **94.99%** | **94.12%** | 100% |

A matched re-evaluation of this checkpoint on a second device
(RTX 4080) scored 95.15% box / 93.96% ±15 on the same holdout.
The largest gain over v1 is precision: v1 often hit the box but
landed off-centre; v2 centres (e.g. steam-navigation ±15
0.584 → 0.977). Per-family results and dataset hashes are in
`PROVENANCE.md` beside the weights and in the angelX repo.

## Intended use

Single-click GUI grounding at ≤1920×1080 (angelX caps the
grounding frame there; larger frames drift). In angelX the model
points and a human approval capsule gates the click; garbled or
off-frame answers fail closed. Use the model's own chat template.

## Limitations

- The holdout is ~74% scripted/synthetic replay (Steam and
  shopping flows dominate). Weight the 94.99% accordingly.
- Native-desktop coverage in the holdout is 12 rows — treat
  native-app performance as effectively unmeasured.
- Training data is click-only: no type/scroll/drag. This
  checkpoint grounds; it does not actuate.
- Offline grounding accuracy is not live task completion.

## Attributions and license

Apache-2.0, following the base model (see `LICENSE` in this repo).

- **This fine-tune (Mage Hand — Mage-VL-Cowork-hands-v2):**
  Jordan Newman / angelX, 2026 — grounding dataset, LoRA training
  runs, merge, and evaluation.
- **Direct base model:**
  [microsoft/Mage-VL-Cowork](https://huggingface.co/microsoft/Mage-VL-Cowork)
  by Microsoft, Apache-2.0 — including the checkpoint's remote
  code (`configuration_llava_onevision2.py`,
  `modeling_llava_onevision2.py`) shipped here unmodified.
- **Upstream of the base:**
  [microsoft/Mage-VL](https://huggingface.co/microsoft/Mage-VL),
  the checkpoint Mage-VL-Cowork was trained from.
- **Text backbone:**
  [Qwen/Qwen3-4B-Instruct-2507](https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507)
  by the Qwen team (Alibaba), per the checkpoint's own
  `config.json` (`text_config._name_or_path`).

Full training and merge provenance is in `PROVENANCE.md` beside
the weights.
