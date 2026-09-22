# polyglot-v1 · 136 tasks · 2026-09-21

- Evaluator: Prime Intellect Verifiers v0.3.1 (`angel-action-v1`)
- Models: DeepSeek V4.1 Flash (`deepseek-flash`, thinking off); GLM-5.3-Flash (`glm-5.3-flash`, thinking low)
- Harnesses: angelX `98d7340`; OpenCode 1.18.31; oh-my-pi (`omp`) 18.2.4

## DeepSeek V4.1 Flash

| | angelX | OpenCode | omp |
|---|:---:|:---:|:---:|
| Solved | 133 / 136 (97.8%) | 132 / 136 (97.1%) | 86 / 93 (92.5%) * |
| Python | 34 / 34 | 34 / 34 | 21 / 23 |
| C++ | 24 / 24 | 23 / 24 | 23 / 23 |
| Rust | 29 / 30 | 28 / 30 | 21 / 23 |
| JavaScript | 46 / 48 | 47 / 48 | 21 / 24 |
| Input tokens | 11,541,247 (87.9% cached) | 67,700,028 (97.4% cached) | 200,112,329 |
| Output tokens | 257,203 | 349,248 | 865,720 |
| Tokens / task | ~84,800 | ~497,800 | ~2,151,700 |
| Median wall | 9.9 s | 8.6 s | 15.3 s |
| Timeouts | 0 | 2 | 7 |

\* omp reached its 200M-token budget cap after 93 tasks. The run was paused once for debugging and resumed.

## Harness notes

- angelX keeps the model's `content` text across tool turns (`PENDING_TOOL_CONTENT`, `assistant_calls_full`).
- On spin or error streaks, angelX injects a `write_file` rewrite instruction and resets the streak counter instead of ending the task.
- angelX compacts aged tool arguments and diagnostics at cache-stable boundaries. omp has no turn limit or compaction; its prompts exceeded 150k tokens, and some tasks used over 36M tokens.
