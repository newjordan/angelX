---
name: hf-model-ops
description: Move models to and from the Hugging Face Hub — resumable downloads, multi-shard uploads, disk hygiene — without re-pulling 60 GB twice.
---

# Hugging Face model ops

Weights are tens of GB; every mistake here is measured in hours. Plan the disk
and the transfer before touching the network.

1. **Disk first.** `df -h` the target volume. Downloads land in
   `~/.cache/huggingface/hub` unless redirected — on a small root disk set
   `HF_HOME` (or pass `--local-dir`) to the big volume *before* starting, not
   after 40 GB.
2. **Download with the `hf` CLI** (successor of `huggingface-cli`; both are
   invoked the same way):
   - Whole repo: `hf download <org>/<repo> --local-dir <dir>`
   - One quant out of a GGUF repo: add `--include "*Q4_K_M*"` — never pull a
     20-file repo for one file.
   - Big pulls: `HF_HUB_ENABLE_HF_TRANSFER=1` (needs `pip install hf_transfer`)
     saturates fast links. Interrupted downloads resume — re-run the same
     command rather than deleting partials.
   - Long pulls are `proc_run` jobs: launch detached, poll the log.
3. **Auth:** gated/private repos need `HF_TOKEN` set (or `hf auth login`
   once — suggest the user run that interactively; never echo a token into a
   log).
4. **Upload:** create the repo (`hf repo create <name>`), then
   `hf upload-large-folder <org>/<repo> <dir>` for multi-shard safetensors —
   it checkpoints and survives disconnects, unlike a naive `hf upload`. Set
   `--private` at creation when unsure; public is one flag later, un-leaking
   is not.
5. **A model card is measured numbers, not adjectives.** What it is, the base
   + revision, how it was made (exact commands), a results table (perplexity /
   evals / tok/s per `benchmark-inference`), and a runnable example command.
6. **Verify integrity when a download misbehaves:** file count and sizes vs
   the repo page; a truncated safetensors shard fails at load with a cryptic
   header error — re-pull the one file, not the repo.
