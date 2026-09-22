# Polyglot Benchmark Evaluation (136 Tasks)

**Date**: 2026-09-21  
**Evaluator**: Prime Intellect Verifiers v0.3.1 (`angel-action-v1`)  
**Models Tested**:
- DeepSeek V4.1 Flash (`deepseek-flash`, thinking off)
- GLM-5.3-Flash (`glm-5.3-flash`, thinking low)
**Harnesses Tested**:
- `angelX` (commit `98d7340`)
- `OpenCode` (v1.18.31)
- `oh-my-pi` (`omp` v18.2.4)

---

## Executive Summary

Following harness calibration—which restored persistent assistant working memory (`PENDING_TOOL_CONTENT`), introduced active cognitive redirections on spin loops, and eliminated ghost duplicate suppressions—`angelX` achieved **1st place overall** on the 136-task polyglot repository-repair benchmark:

- **133 / 136 (97.8%)** tasks passed cleanly.
- **Python (100%)** and **C++ (100%)** achieved perfect scores.
- **5.8× more token-efficient** than OpenCode (11.5M vs 67.7M input tokens).
- **25× more token-efficient** than OMP (~85k vs ~2.15M tokens/task).
- **Zero timeouts** and zero abnormal exits across all 136 tasks.

In contrast, `omp` breached the 200,000,000 token hard cap at task 93 across its combined runs due to unbounded turn limits and the absence of loop circuit breakers, resulting in an automatic termination and benchmark failure on budget exhaustion.

---

## Final Head-to-Head Results

| Metric | `deepseek/angelx` | `deepseek/opencode` | `deepseek/omp` |
|---|:---:|:---:|:---:|
| **Overall Score** | **133 / 136 (97.8%)** 🏆 | **132 / 136 (97.1%)** | **86 / 93 (92.5%)** * ❌ |
| **Python** | **34 / 34 (100%)** | 34 / 34 (100%) | 21 / 23 |
| **C++** | **24 / 24 (100%)** | 23 / 24 (95.8%) | 23 / 23 |
| **Rust** | **29 / 30 (96.7%)** | 28 / 30 (93.3%) | 21 / 23 |
| **JavaScript** | **46 / 48 (95.8%)** | 47 / 48 (97.9%) | 21 / 24 |
| **Total Input Tokens** | **11,541,247** (87.9% cache hit) | 67,700,028 (97.4% cache hit) | **200,112,329** (Capped Breach) |
| **Output Tokens** | **257,203** | 349,248 | **865,720** |
| **Tokens / Task** | **~84,800** | ~497,800 | **~2,151,700** |
| **Median Wall Clock** | **9.9s** | 8.6s | **15.3s** (multiple >10m runs) |
| **Timeouts** | **0** | 2 | **7** |
| **Benchmark Outcome** | **PASS (1st Place)** | **PASS (2nd Place)** | **FAIL (Budget Exhaustion)** |

\* oh-my-pi stopped after 93 tasks across its two combined runs upon breaching the 200,000,000 token cap (86 solved).

---

## Architectural Distinctions

### 1. Working Thought Preservation vs. Model Amnesia
Non-reasoning chat models (DeepSeek Chat / Flash) communicate their working hypothesis in the `content` field alongside tool calls. `angelX` retains this prose across turns via `PENDING_TOOL_CONTENT` and `assistant_calls_full`, ensuring the model does not forget its hypothesis upon receiving tool diagnostics.

### 2. Cognitive Redirection vs. Hard Aborts
Previous harness versions hard-killed tasks on `SpinStop` and `ErrorStop`. The calibrated harness instead injects direct cognitive telemetry instructing a full-file rewrite via `write_file` and clears the streak counter, giving the model the runway to self-correct.

### 3. Context Compaction vs. Unbounded Growth
`angelX` uses `cache-stable` boundary compaction to shrink aged tool arguments and truncate redundant diagnostics. OMP lacks both turn limits and compaction, causing prompts to snowball past 150k tokens and burning 36M+ tokens on individual tasks.
