# angelX 0.1.9 harness comparison

Source: the chart `docs/images/bench/angelx-0.1.9-harness-comparison.png`,
"angelX 0.1.9 · benchmark score and time", built from the existing tournament
benchmark datasets. Each row runs angelX and one other harness on the same model
and tasks. Time is total agent time in minutes. The run date and host hardware
are not recorded on the chart.

| Comparison harness | Model | Solved, angelX | Solved, harness | angelX min | Harness min |
|---|---|---:|---:|---:|---:|
| oh-my-pi * | DeepSeek V4.1 Flash | 133 / 136 | 53 / 59 | 34.5 | 76.5 |
| OpenCode | DeepSeek V4.1 Flash | 135 / 136 | 132 / 136 | 27.7 | 58.7 |
| Hermes | DeepSeek V4.1 Flash | 136 / 136 | 132 / 136 | 25.7 | 71.2 |
| Grok CLI | Grok 4.7 | 135 / 136 | 135 / 136 | 51.5 | 77.8 |
| DeepSeek harness | DeepSeek V4.1 Flash | 136 / 136 | 136 / 136 | 25.7 | 53.0 |
| ZCode | GLM 5.3 Flash | 135 / 136 | 132 / 136 | 168.1 | 155.9 |
| Kimi Code | Kimi k3 | 25 / 25 | 25 / 25 | 17.2 | 18.2 |
| Muse Code | Muse Spark 1.3 | 136 / 136 | 136 / 136 | 50.8 | 165.7 |
| Codex | GPT-6.1 Sol | 136 / 136 | 136 / 136 | 83.4 | 89.7 |

\* oh-my-pi is the existing 59-task stopped cohort. Kimi uses the existing
25-task matched sample. Tokens on the chart include cached input.

angelX solved as many tasks as the other harness, or more, in every row. Its
total agent time was lower in eight of nine rows. ZCode on GLM 5.3 Flash took
less time and solved three fewer tasks. Newer Muse Code and Grok CLI results
for angelX 0.2.7 are in the README and `docs/RELEASE_0.2.7.md`.
