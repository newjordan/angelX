# Why the full 136-task runs are slower than OpenCode · 2026-09-26

The slowdown is real and the earlier summary understated it by leading with jointly solved medians. On all 136 tasks, compact Qwen Angel takes **215.0 cumulative agent minutes versus OpenCode’s 113.8 minutes: 88.9% longer**. The published September 21 GLM comparison takes **174.1 versus 129.4 minutes: 34.5% longer**. The later GLM Angel run improves to **152.3 minutes but remains 17.6% slower**. These are sums of attempt durations, including failed and timed-out attempts, rather than elapsed time for a parallel campaign.

The explanation is model time and task-specific recovery loops, with a separate long-tail tool problem on DeepSeek. It is not supported to attribute GLM’s entire gap to verification. In the latest GLM comparison, Angel spends 1,794.3 extra model seconds while its non-model residual is 424.0 seconds lower. Qwen is slower on 128 of 136 tasks, so its deficit is broad rather than a single failure.

## Full-cohort measurements

Every row below contains exactly the same 136 task identities. Failures and successful solutions produced before a timeout remain included. “Latest” names the latest eligible historical full cell in the inventory, not the current dev binary. Muse uses different recorded model IDs (`muse-spark-1.3` versus `muse-spark-1.3-contributor`) and is descriptive, not a same-model controlled result.

| Model / run | Harness | Solved / timeouts | Agent total min | Mean s | Median s | p90 s |
|---|---|---:|---:|---:|---:|---:|
| DeepSeek Sep21 | Angel | 133/136 / 0 | 34.5 | 15.2 | 9.9 | 31.0 |
| DeepSeek latest | Angel | 134/136 / 1 | 51.4 | 22.7 | 10.7 | 31.7 |
| GLM Sep21 | Angel | 135/136 / 1 | 174.1 | 76.8 | 50.7 | 141.5 |
| GLM latest | Angel | 134/136 / 1 | 152.3 | 67.2 | 46.1 | 116.9 |
| Grok latest | Angel | 136/136 / 0 | 48.3 | 21.3 | 17.5 | 36.5 |
| Muse | Angel | 136/136 / 0 | 71.3 | 31.5 | 25.5 | 59.4 |
| Qwen rotation | Angel | 133/136 / 1 | 218.3 | 96.3 | 80.0 | 155.7 |
| Qwen compact | Angel | 135/136 / 1 | 215.0 | 94.8 | 78.9 | 155.3 |
| deepseek | OpenCode | 132/136 / 2 | 58.7 | 25.9 | 8.6 | 26.4 |
| glm | OpenCode | 133/136 / 0 | 129.4 | 57.1 | 38.9 | 97.3 |
| grok | OpenCode | 136/136 / 0 | 44.9 | 19.8 | 16.2 | 36.4 |
| muse | OpenCode | 136/136 / 0 | 67.2 | 29.7 | 24.2 | 49.0 |
| qwen | OpenCode | 135/136 / 0 | 113.8 | 50.2 | 37.7 | 84.2 |

The median describes the middle task; the cumulative chart describes the sum. Both answer valid questions, but neither can replace the other. The DeepSeek latest median is 23.6% higher than OpenCode’s while its total is 12.3% lower because OpenCode has two approximately 600-second failures. Conversely, GLM and Qwen remain slower when the failures are removed.

| Angel run vs OpenCode | All-task wall delta s | Total change | All-task paired median s | Angel slower tasks | Jointly solved tasks | Jointly solved total delta s | Failure-involved delta s |
|---|---:|---:|---:|---:|---:|---:|---:|
| DeepSeek Sep21 | -1,447.1 | -41.1% | -0.32 | 61/136 | 129 | -465.0 | -982.1 |
| DeepSeek latest | -432.7 | -12.3% | 0.09 | 73/136 | 130 | -5.7 | -426.9 |
| GLM Sep21 | 2,676.3 | +34.5% | 5.58 | 94/136 | 132 | 2,034.4 | 641.9 |
| GLM latest | 1,370.4 | +17.6% | 4.30 | 91/136 | 132 | 1,158.4 | 212.0 |
| Grok latest | 202.0 | +7.5% | 0.52 | 77/136 | 136 | 202.0 | 0.0 |
| Muse | 245.0 | +6.1% | 0.15 | 69/136 | 136 | 245.0 | 0.0 |
| Qwen rotation | 6,270.0 | +91.8% | 36.44 | 127/136 | 133 | 5,789.1 | 480.8 |
| Qwen compact | 6,070.4 | +88.9% | 33.52 | 128/136 | 135 | 5,688.4 | 382.1 |

“Failure-involved” sums the paired differences for tasks not solved by both harnesses; it is not the raw time spent in failed tasks. The jointly solved calculation is a secondary diagnostic, not a substitute for the all-task result. For GLM latest, removing those tasks still leaves 1,158.4 extra seconds; for Qwen compact it leaves 5,688.4 extra seconds.

## Where the time goes

| Latest Angel vs OpenCode | Model time delta s | Non-model delta s | Calls Angel / peer | Output tokens Angel / peer | Tasks over 60 s Angel / peer |
|---|---:|---:|---:|---:|---:|
| DeepSeek latest | -1,035.7 | 603.0 | 1,055 / 1,699 | 309,584 / 349,248 | 8 / 6 |
| GLM latest | 1,794.3 | -424.0 | 1,074 / 1,001 | 222,019 / 178,729 | 45 / 32 |
| Grok latest | 368.8 | -166.8 | 681 / 673 | 133,909 / 112,138 | 4 / 1 |
| Muse | 408.7 | -163.6 | 673 / 793 | 311,199 / 354,466 | 14 / 10 |
| Qwen compact | 6,330.0 | -259.6 | 825 / 771 | — / 243,245 | 92 / 32 |

The residual includes tool execution, scheduling, harness processing and any time not measured by the model proxy. It is not a pure CPU-overhead or verification timer. Input and cache totals and their coverage remain in the machine-readable data. Angel Qwen has no reported token usage; generated-character measurements must not be relabeled as tokens.

GLM uses 73 more calls and emits 43,290 more output tokens, including 22,344 more reported reasoning tokens. Qwen compact adds only 54 model attempts, yet consumes 6,330.0 extra model seconds. Its 823 sampled responses produce 1,344,230 characters versus OpenCode’s 763,782 across 771 responses; the full-cohort generated reasoning and visible prose are materially larger. Existing trace shape analysis shows smaller median input messages and schemas in Angel, so “the system prompt is larger” is not an adequate diagnosis.

## Language contributions, including failures

| Latest Angel run | Language / tasks | Solved Angel / peer | Total min Angel / peer | Wall delta s | Model delta s | Calls delta |
|---|---|---:|---:|---:|---:|---:|
| DeepSeek latest | js / 48 | 47 / 47 | 26.8 / 19.3 | 447.0 | -270.2 | -168 |
| DeepSeek latest | py / 34 | 34 / 34 | 6.8 / 15.1 | -499.6 | -445.9 | -244 |
| DeepSeek latest | rust / 30 | 29 / 28 | 11.8 / 17.9 | -371.4 | -352.1 | -245 |
| DeepSeek latest | cpp / 24 | 24 / 23 | 6.1 / 6.2 | -8.8 | 32.4 | 13 |
| GLM latest | js / 48 | 48 / 48 | 54.7 / 40.2 | 871.4 | 914.9 | 51 |
| GLM latest | py / 34 | 34 / 33 | 34.1 / 29.1 | 303.0 | 518.5 | 14 |
| GLM latest | rust / 30 | 29 / 29 | 42.0 / 35.7 | 378.8 | 386.1 | -1 |
| GLM latest | cpp / 24 | 23 / 23 | 21.5 / 24.5 | -182.8 | -25.2 | 9 |
| Grok latest | js / 48 | 48 / 48 | 14.2 / 13.7 | 27.2 | 94.7 | -1 |
| Grok latest | py / 34 | 34 / 34 | 10.5 / 9.9 | 37.1 | 78.3 | 5 |
| Grok latest | rust / 30 | 30 / 30 | 13.1 / 12.2 | 52.1 | 74.3 | -6 |
| Grok latest | cpp / 24 | 24 / 24 | 10.5 / 9.1 | 85.6 | 121.4 | 10 |
| Muse | js / 48 | 48 / 48 | 23.7 / 21.6 | 130.1 | 210.1 | -47 |
| Muse | py / 34 | 34 / 34 | 16.8 / 14.7 | 125.0 | 160.1 | -13 |
| Muse | rust / 30 | 30 / 30 | 18.5 / 19.2 | -41.9 | -22.9 | -40 |
| Muse | cpp / 24 | 24 / 24 | 12.3 / 11.7 | 31.7 | 61.3 | -20 |
| Qwen compact | js / 48 | 48 / 48 | 63.5 / 34.4 | 1,744.4 | 1,795.5 | 21 |
| Qwen compact | py / 34 | 34 / 34 | 46.1 / 25.2 | 1,251.1 | 1,291.5 | 18 |
| Qwen compact | rust / 30 | 29 / 29 | 70.4 / 34.1 | 2,177.8 | 2,188.8 | 13 |
| Qwen compact | cpp / 24 | 24 / 24 | 35.0 / 20.0 | 897.1 | 1,054.2 | 2 |

Qwen loses time in every language; Rust contributes 2,177.8 seconds of its 6,070.4-second deficit. GLM’s largest language contribution is JavaScript (+871.4 seconds), followed by Rust (+378.8) and Python (+303.0); C++ is faster (−182.8). A Rust-only rerun is useful regression evidence but cannot represent the whole workload. The new Luna Rust rerun also has no matching OpenCode Luna cell here, so it cannot by itself explain a cross-harness latency gap.

## Largest individual contributions

Raw references below use run IDs in the provenance appendix and one-based JSONL line numbers. They refer to actual model-call nodes rather than every node in a graph, which would double-count replayed history.

### GLM latest

| Task | Angel / peer wall s | Wall delta s | Model delta s | Calls Angel / peer | Outcome Angel / peer | Raw line Angel / peer |
|---|---:|---:|---:|---:|---|---:|
| `rust-alphametics` | 600.1 / 87.3 | 512.8 | 498.7 | 50 / 5 | fail / pass | 11 / 11 |
| `rust-decimal` | 415.5 / 146.1 | 269.4 | 266.1 | 35 / 13 | pass / pass | 23 / 23 |
| `js-alphametics` | 301.7 / 52.4 | 249.3 | 250.5 | 22 / 6 | pass / pass | 5 / 5 |
| `js-zebra-puzzle` | 252.2 / 29.0 | 223.2 | 215.7 | 31 / 4 | pass / pass | 135 / 135 |
| `js-food-chain` | 193.0 / 37.9 | 155.1 | 151.1 | 26 / 9 | pass / pass | 37 / 37 |
| `py-pov` | 202.3 / 59.8 | 142.6 | 142.1 | 14 / 6 | pass / pass | 78 / 78 |

### Qwen compact

| Task | Angel / peer wall s | Wall delta s | Model delta s | Calls Angel / peer | Outcome Angel / peer | Raw line Angel / peer |
|---|---:|---:|---:|---:|---|---:|
| `rust-doubly-linked-list` | 600.1 / 218.0 | 382.1 | 374.2 | 12 / 8 | fail / fail | 31 / 8 |
| `rust-parallel-letter-frequency` | 225.3 / 21.1 | 204.2 | 190.9 | 10 / 4 | pass / pass | 71 / 8 |
| `rust-decimal` | 299.0 / 105.1 | 193.9 | 195.8 | 8 / 6 | pass / pass | 23 / 8 |
| `rust-react` | 241.7 / 74.2 | 167.5 | 168.0 | 11 / 6 | pass / pass | 83 / 4 |
| `py-forth` | 174.4 / 31.3 | 143.1 | 143.9 | 7 / 4 | pass / pass | 38 / 7 |
| `js-wordy` | 194.2 / 76.5 | 117.8 | 118.8 | 5 / 5 | pass / pass | 134 / 6 |

### DeepSeek latest

| Task | Angel / peer wall s | Wall delta s | Model delta s | Calls Angel / peer | Outcome Angel / peer | Raw line Angel / peer |
|---|---:|---:|---:|---:|---|---:|
| `js-bowling` | 600.3 / 30.5 | 569.8 | 26.1 | 30 / 13 | fail / pass | 25 / 25 |
| `js-zebra-puzzle` | 204.6 / 12.8 | 191.9 | 11.4 | 10 / 6 | pass / pass | 135 / 135 |
| `rust-doubly-linked-list` | 143.1 / 32.2 | 110.9 | 103.5 | 52 / 11 | pass / fail | 31 / 31 |
| `js-rectangles` | 100.8 / 7.6 | 93.2 | 88.1 | 33 / 4 | pass / pass | 109 / 109 |
| `js-alphametics` | 81.1 / 8.6 | 72.5 | 66.5 | 34 / 4 | pass / pass | 5 / 5 |
| `rust-forth` | 77.8 / 18.0 | 59.8 | 54.9 | 35 / 7 | pass / pass | 39 / 39 |

For GLM, `rust-alphametics` alone adds 512.8 seconds and 45 calls; the timeout is dominated by 580.9 seconds of recorded model work. `rust-decimal`, `js-alphametics` and `js-zebra-puzzle` are successful but require many more calls. Their extra model work cannot be explained by a fixed startup or test-confirmation fee. `js-transpose` adds 95.3 model seconds with only one extra call, identifying a response-latency/output-length problem in addition to loop count.

For Qwen, the failed `rust-doubly-linked-list` attempt contributes 382.1 extra seconds, only 6.3% of the total deficit. `js-wordy` uses five calls in both harnesses but adds 118.8 model seconds; one Angel reasoning response takes 159.3 seconds for 21,326 reasoning characters versus 63.3 seconds for 8,610 on OpenCode. `rust-forth` is 105.6 seconds slower despite two fewer calls. These are concrete per-response generation differences rather than excess harness scheduling.

DeepSeek illustrates why tail accounting matters in both directions. Angel’s `js-bowling` ends at the 600-second cap with only 52.3 recorded model seconds; after its final model call, the shell probe occupies a 539.2-second gap before termination. The successful `js-zebra-puzzle` attempt spends 182.8 tool seconds, including one 180.1-second shell test (`npm test` piped through `head`). Those are tool/test tails. They are offset in the full total by OpenCode’s 600-second `rust-book-store` and `js-complex-numbers` failures. The smaller diagnostic-output fix from the prior audit helps known recovery turns but does not account for these larger stalls.

The release handoff identifies the final bowling probe precisely: a here-document followed by `npx jest --rootDir /` crawled the filesystem. Verification classification deliberately rejects here-documents, so the old classified test deadline did not bind that call. The audited dev source still had this gate and the 180-second/wall-third test budget. The fix from separate worktree commit `401cd01` has now been adapted into dev: task mode supplies a default 120-second cap to managed foreground processes regardless of verification classification, with explicit overrides and the old environment knob retained as a deprecated fallback. An independent review also found and fixed a YOLO path that cleared the cap for sleeping children. The worktree itself was only inspected read-only.

This is configurable foreground-process containment, not a cumulative tool deadline or arbitrary native-code preemption; fixed internal verifier/recon deadlines remain independent. Background `proc_run` jobs remain exempt, and legitimate long work can use an operator override. A bounded regression reproduces both direct and here-document spinning test commands, checks their classification differs, stops each at two seconds, then repairs the source and runs a successful test through the same registry. Companion tests cover background survival, legacy/new knob precedence, and a sleeping YOLO child with idle protections disabled. These offline checks do not establish a new full136 speed or solve-rate result.

The final affected development gate passed **11 tests, zero failures**, including ordinary YOLO timeout opt-out and fixed control-probe deadline compatibility. Boundary and active-source-connection checks also passed. Reproduce with `bash scripts/check/check-cockpit-fast.sh task_call_budget call_budget_caps_tool_bounds_and_restores_them busy_process_is_killed_at_the_call_budget sleeping_process_keeps_call_budget_under_yolo yolo_does_not_disable_containment_ceilings yolo_idle_floor_reaps_a_silent_foreground_server fixed_control_probe_deadline_survives_yolo yolo_disables_external_command_deadlines hung_suite_report_names_the_tests_that_never_finished --test-threads=1`. The captured log is `artifacts/polyglot-full136-20260926/deadline-focused-tests.log`.

## What the existing confirmation experiment actually proves

Four full cohorts form two same-binary treatment pairs. The fifth `head` cohort uses a different binary and is observational. The treatment is `ANGEL_CONFIRM_GREEN_RUNS=0` versus `2`: repeat the last green test command twice before accepting completion, not a change to the evaluator’s heldout grading. Both arms use DeepSeek Flash, thinking off, temperature 0, 8,192 recorded output tokens, 60 hops and 600-second task limits. `ANGEL_VERIFY_BEFORE_DONE=0` is held fixed in both. The two arms run side-by-side for each seed label; repetitions run sequentially. A seed label is not a provider sampling seed.

| Confirmation cohort | Solved | Total wall s | Median s | Timeout tasks | Binary prefix |
|---|---:|---:|---:|---:|---|
| head seed3 | 134/136 | 2,969.8 | 11.5 | 1 | `5bd2c1a82cd8` |
| off seed1 | 133/136 | 2,359.1 | 10.3 | 0 | `fbb40b53e2be` |
| off seed2 | 136/136 | 2,779.0 | 11.6 | 0 | `fbb40b53e2be` |
| on seed1 | 133/136 | 2,842.0 | 11.3 | 1 | `fbb40b53e2be` |
| on seed2 | 134/136 | 3,351.2 | 12.6 | 1 | `fbb40b53e2be` |

Across the two paired repetitions, repeat-off solves 269/272 and takes 5,138.1 seconds; repeat-on solves 267/272 and takes 6,193.2 seconds (+20.5%). That is an observed treatment outcome, not proof that 20.5% was directly spent rerunning tests. Among jointly solved tasks, repeat-on adds 87.1 seconds in the first repetition and saves 4.3 seconds in the second; changed failure tails explain most of the total difference. Shared provider load, cache state and divergent model trajectories remain confounds.

The useful configuration correction already exists in current dev. `confirm_green_extra_runs` defaults to zero, while `confirm_green_by_chance` enables two extra checks only for task-mode edits containing randomness, clocks or thread patterns. The current benchmark runner leaves the override unset and therefore uses that selective policy. Blanket confirmation is not still enabled across all tasks. Disabling selective checks would weaken an existing correctness guard; the historical A/B does not justify doing that to manufacture a faster result. See `cockpit/src/agent/harness/turn/mod.rs` and `cockpit/docs/ENV.md` for the implementation and recorded original decision.

## Timing-schema and publication audit

Verifiers v1 `TimeSpan` serializes `start` and `end` in seconds; its `duration` property is derived and absent from JSON. The audit recomputes each agent duration as `end − start`. `timing.agent.model.duration` and `.harness.duration` are serialized second counts. The evaluator calculates the model partition as the sum of valid call timestamp spans capped at the agent span, then assigns the remainder to harness time. The re-audit found no agent partition mismatch, no proxy-call duration mismatch, and no invalid agent timestamps in the full cohorts. It does not infer model time from output-token counts.

Direct ChatGPT Luna bypasses the evaluator proxy, so its proxy model value is a placeholder zero. Angel’s completed-task timing envelope is used instead. The latest Luna `cpp-phone-number` task has no completed envelope but still consumes 600.083 agent seconds and passes the external grader; those seconds stay in the 5,847.142-second full total while its model/tool partition is unknown. Adding the model and non-model column totals with different coverage would silently omit that time. The previous board’s wall data was not wrong, but its partition totals must not substitute for all-task wall totals.

External verifier/scoring spans are reported separately from agent time; they are not silently added to the cumulative chart. Proxy prompt-token accounting uses uncached plus cached input, and output already includes reported reasoning. Missing Qwen usage stays unknown. Response-shape counts use `calls[].node`, not every replayed graph node. Four targeted tests cover failed-tail retention, timestamp semantics, missing partitions and unknown-directory retention.

The website’s `bench.js` race chart adds every attempt’s `wall_s`, regardless of success. `scripts/render_readme_graphs.py` copies `website/js/bench-data.js`, so the README chart family uses that same cumulative representation. The numerical sources reconcile as follows:

| Published source | Raw correspondence | Maximum per-task timing difference |
|---|---|---:|
| `website/js/bench-data.js` / README charts | Sep21 DeepSeek and GLM cells | under 0.005 s (two-decimal display rounding) |
| `docs/telemetry/polyglot-benchmark-20260921.json` | Same Sep21 cells | 0 s |
| Muse harness report `out/bench-data.json` | Sep23 Angel / Sep24 peers | under 0.0005 s |
| 0.1.6 film `src/run-details.ts` | Sep23 latest Angel cells | under 0.0005 s |

The earlier `angelx-board` and `grok-board` files remain historical snapshots, not authorities for the latest cohort. Quarantined pre-fix GLM results are excluded. README’s claim that verification alone accounts for GLM’s extra wall time has been replaced with the measured full-cohort result and this report link. Historical tables and cell identities remain intact.

The separate `HANDOFF-harness-roots-20260923.md` and `HANDOFF-release-0.1.6-20260923.md` were also checked. They identify the same historical run cells and distinguish the later unmerged call-deadline work from the measured 0.1.6 build. No additional full136 corpus was found in those handoffs or the broader root inventory.

## Inventory coverage and reproducibility

The audit recursively discovers all `traces.jsonl` files under the benchmark root, rather than using a directory whitelist. Every file is retained in an inventory with its SHA-256 and classification; an unfamiliar directory is labeled UNKNOWN for review instead of disappearing.

| Classification | Trace files |
|---|---:|
| quarantined | 2 |
| full 136-task cohort | 25 |
| aborted | 4 |
| archived calibration | 1 |
| diagnostic/reproduction/selective | 168 |
| partial cohort | 4 |
| disjoint Qwen chunk | 51 |

There are **255 discovered trace files and zero unknown classifications**. The 25 eligible complete files plus three reconstructed Qwen cohorts give 28 usable full136 cells. One archived calibration full136 cell is retained in the data but excluded from comparisons, giving 29 total full-cohort entries. Each Qwen reconstruction combines 17 disjoint eight-task chunks with matching recorded configuration and binary pins; duplicates, missing tasks or pin drift fail the analysis. The four partial cohorts include the 93-task DeepSeek omp budget-capped run and its separate 34-task pickup; the pickup is not silently promoted into a full136 result.

All 30 all-task/jointly-solved comparisons have matching hashed per-task contracts after removing only the chunk-local index. Each hash covers the recorded prompt, fixture/gold definitions, verifier, protected-file hashes and remaining task data; none of those bodies is exported. Same task contracts and recorded model settings do not establish identical native HTTP requests, model-serving builds, cache warmth or effective reasoning budgets.

`runs-gentrim-full.log` is zero bytes and there is no corresponding trace directory in the recursive inventory. The proposed gen-trim binary is not a measured full run. The separate `rerun-20260921` corpus has 18 distinct tasks × three repetitions × two models × three harnesses (324 attempts); its earlier Angel/OpenCode advantage is not pooled with Polyglot. Root-level single-harness legacy/debug runs and current 12-task/Rust regression cells cannot supply missing full136 peer evidence.

Full eligible cohort inventory by variant:

| Family / harness | Full cohorts |
|---|---:|
| deepseek/angelx | 7 |
| deepseek/opencode | 1 |
| glm/angelx | 2 |
| glm/omp | 1 |
| glm/opencode | 1 |
| grok/angelx | 3 |
| grok/hermes | 1 |
| grok/omp | 1 |
| grok/opencode | 1 |
| grok/primebash | 1 |
| luna/angelx | 2 |
| muse/angelx | 1 |
| muse/omp | 1 |
| muse/opencode | 1 |
| qwen/angelx | 2 |
| qwen/opencode | 1 |
| qwen/omp | 1 |

Reproduce the audit without model calls:

```sh
python3 scripts/analyze-polyglot-full136.py \
  --output artifacts/polyglot-full136-20260926/full136.json
python3 -m unittest discover -s tests/python -p 'test_polyglot*.py' -v
```

The body-free JSON includes every discovered path/hash, all full-cohort task rows, whole-cohort and language statistics, both pairing scopes, top contributors, task-contract checks, timing audits, confirmation treatment pairs and publication reconciliation. It contains no prompts, model answers, raw reasoning, credentials or HTTP request bodies. The relevant source root is `/home/frosty40/angel_tests/angelX-bench/polyglot-20260921`.

## Primary cell identities

| Label | Angel raw run | OpenCode raw run |
|---|---|---|
| DeepSeek Sep21 | `runs/deepseek/angelx/seed0-20260921T200947Z` | `runs/deepseek/opencode/seed0-20260921T125531Z` |
| DeepSeek latest | `runs/deepseek/angelx/seed0-20260923T195645Z` | `runs/deepseek/opencode/seed0-20260921T125531Z` |
| GLM Sep21 | `runs/glm/angelx/seed0-20260921T210237Z` | `runs/glm/opencode/seed0-20260921T142517Z` |
| GLM latest | `runs/glm/angelx/seed0-20260923T195647Z` | `runs/glm/opencode/seed0-20260921T142517Z` |
| Grok latest | `runs/grok/angelx/seed0-20260923T195818Z` | `runs/grok/opencode/seed0-20260923T013311Z` |
| Muse | `runs/muse/angelx/seed0-20260923T192805Z` | `runs/muse/opencode/seed0-20260924T020927Z` |
| Qwen rotation | `runs-qwen/qwen/angelx/combined` | `runs-qwen/qwen/opencode/combined` |
| Qwen compact | `runs-compact-full/qwen/angelx/seed0-20260925T202320Z` | `runs-qwen/qwen/opencode/combined` |

`combined` is a virtual cohort, not a directory. Its 17 constituent paths and per-task raw run/line references are retained in `full136.json`; no physical trace is rewritten or fabricated.

## Next improvement priorities

1. Qwen: reduce repeated/generated reasoning and routine prose using the actual serving contract. The [request-level audit](polyglot-provider-slowdown-20260926.md) now includes a controlled four-call ABBA continuation probe on the same configured route: retaining historical `reasoning_content` took 8.780 mean seconds and 91 completion tokens versus 13.947 seconds and 404 tokens when omitted (37.1% less wall time), with zero cache reads in all calls. This confirms a local continuation effect for that request, not a whole-task success improvement or the cause of the entire historical deficit. Route-specific implementation and task validation remain necessary; a global replay switch is not justified.
2. GLM: reduce failure-recovery calls on the named expensive tasks and inspect unusually slow same-count responses. Preserve the first actionable test failure so the model need not spend another turn retrieving it; that narrow fix already has historical snippet evidence.
3. DeepSeek: the adapted `401cd01` foreground-process cap now addresses the proven classification escape, with a YOLO correction and bounded repair/background regressions. It preserves the task's grading and correctness requirements, and retains the old timeout override as a deprecated fallback. The cap is per managed process, not a hard deadline over arbitrary tool code. The historical data and offline regressions do not yet establish its solve-rate or full136 speed effect.
4. Keep aggregate wall, correctness, median, p90, language splits and failure-tail contribution together in future reports. The current Rust regression run validates current behavior; it does not establish a new full136 OpenCode-relative speed result.
