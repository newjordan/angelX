---
name: serve-local-llm
description: Bring up a local inference server (llama.cpp / vLLM), confirm it's ready, and hand back a measured endpoint — not a "probably running" one.
---

# Serve a local LLM

The deliverable is a **probed, benchmarked endpoint**, never just a launched
process. The loop:

1. **Size it before launching.** VRAM needed ≈ weights + KV cache + overhead.
   Check headroom first with `gpu_stat` — a half-loaded server that OOMs three
   minutes in wastes more time than the check. If it's tight: smaller quant,
   smaller `--ctx-size`, or quantized KV cache (`-ctk q8_0 -ctv q8_0`).
2. **Launch detached with `proc_run`** — never `shell` (it blocks the turn and
   kills the server on return):
   - llama.cpp: `llama-server -m <model>.gguf -ngl 99 -fa --port 8080`
   - vLLM: `vllm serve <model> --port 8000` (add
     `--gpu-memory-utilization 0.90`, `--tensor-parallel-size N` for multi-GPU)
3. **Snapshot `proc_status` between other work until the readiness line appears**
   in the log tail — llama.cpp: "server is listening"; vLLM: "Uvicorn running".
   Never set `wait_ms` (it is ignored and used to freeze thinking). Model load
   can take minutes on big weights; if the process *exits*, the log tail has the
   real error (usually OOM or a bad flag) — fix and relaunch, don't retry blind.
4. **Confirm with `llm_probe`** — the served model id is what clients must use.
5. **Measure with `llm_bench`** and report TTFT + decode tok/s alongside the
   exact launch command. A server nobody measured is a rumor.
6. **When done, `proc_stop`** — unless the user asked for a persistent server;
   then say it's still running, with its log path and how to stop it.

Failure playbook: OOM → smaller quant / ctx / KV-quant, or check `gpu_stat` for
a squatter process holding VRAM. Port in use → another server is already up;
probe it before killing anything. Slow decode vs expectations → confirm `-ngl`
actually offloaded (llama.cpp log prints layer counts) and flash attention is
on.
