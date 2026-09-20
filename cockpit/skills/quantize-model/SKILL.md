---
name: quantize-model
description: Quantize a model (GGUF / FP8 / NVFP4) and prove quality survived — perplexity delta and a behavioral spot-check, never "it loads, ship it".
---

# Quantize a model

A quant that loads is not a quant that works. The job has three parts: convert,
**verify**, document — and the middle one is the point.

1. **Pick the format for the target runtime, not by fashion.**
   - llama.cpp/GGUF: `Q4_K_M` is the sane default; `Q5_K_M`/`Q6_K` when VRAM
     allows; `Q8_0` as the near-lossless reference. Below Q4, use an
     importance matrix (`llama-imatrix` on a representative text corpus, then
     quantize with `--imatrix`) — low-bit without imatrix bleeds quality.
   - vLLM/TensorRT: FP8 (weight+activation) via llm-compressor or ModelOpt;
     NVFP4 where the hardware supports it. Watch for mixed-format models —
     e.g. MoE draft/expert layers quantized differently than the trunk need
     their routing handled per-format at load.
2. **Convert.** GGUF: `convert_hf_to_gguf.py` → `llama-quantize <in> <out>
   <TYPE>`. These are long jobs — run under `proc_run` and poll, don't block
   the turn. Check free disk first: conversion needs roughly original +
   converted + quantized sizes at once.
3. **Verify against the least-compressed baseline you can run:**
   - **Perplexity delta** (`llama-perplexity -m <quant> -f <corpus>`), quant vs
     Q8_0/BF16 on the same corpus. Rule of thumb: Q4_K_M costs a few percent;
     a jump beyond that means something's wrong (bad imatrix, wrong template,
     broken tensor).
   - **Behavioral spot-check:** serve it, `llm_bench` for speed, then a few
     fixed prompts you know the full-precision answers to — code generation
     breaks before chat does, so include a coding prompt.
4. **Document like a release:** base model + revision, exact conversion
   commands, tool versions, perplexity table (baseline vs quant), measured
   speed, known regressions. If publishing to HF, that block *is* the README —
   see `hf-model-ops`.
