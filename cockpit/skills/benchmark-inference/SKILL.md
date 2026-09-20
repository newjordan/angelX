---
name: benchmark-inference
description: Produce measured, reproducible inference numbers — TTFT and decode tok/s with pinned config — instead of vibes or single-run anecdotes.
---

# Benchmark inference (measured numbers only)

A number without its config is noise. Every benchmark you report must be
reproducible from what you write down.

1. **Pin the config first.** Record: model + quantization, server + version
   (`llama-server --version`, `pip show vllm`), GPU + driver (`gpu_stat` gives
   both), the full launch command, and the prompt. A result that can't be
   re-run is not a result.
2. **Warm up, then measure ≥3 runs.** First run pays load/compile/cache costs —
   discard it (`llm_bench` does this for you and reports per-run + mean).
   Report mean *and* spread; runs that disagree wildly mean something else is
   on the GPU — check `gpu_stat` for squatters before trusting anything.
3. **Separate the two numbers.** TTFT is prefill (compute-bound, scales with
   prompt length); decode tok/s is generation (bandwidth-bound). A change can
   help one and hurt the other — speculative decoding typically boosts decode
   while leaving TTFT alone. Never blend them into one "speed".
4. **Change one variable per comparison.** Quant vs quant, flag vs flag, same
   everything else. Comparing Q4 on box A to FP8 on box B is two experiments
   pretending to be one.
5. **Know your tool's precision.** `llm_bench` is single-stream and counts
   SSE chunks when the server doesn't report usage (≈ tokens, exact when it
   does — the output says which). For throughput-vs-concurrency curves or
   long-context sweeps, drive the dedicated harnesses via `proc_run`:
   `llama-bench` (llama.cpp) or `vllm bench serve`.
6. **Report as a table** — config column, TTFT, decode tok/s, spread — and
   state the single-stream vs batch context. Note anything that would trip a
   reader trying to reproduce it.
