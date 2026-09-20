# 12 distinguishing features

Source-backed implementation shortlist. Comparative performance remains unmeasured.

| # | Feature | Implemented behavior | Source |
|---|---|---|---|
| 1 | A visual terminal workspace | Coding, portraits, image inspection, research views and a navigable Dotmax world share the ordinary terminal. | [Compositor](../cockpit/src/ui/draw.rs), [world](../cockpit/src/stage/world_viz.rs) |
| 2 | Model selection inside agent formations | Choose a model and supported thinking level; configure individual MoA seats and run declared agent graphs. | [Formations](../cockpit/src/agent/formations.rs), [graph controls](../cockpit/src/app/control/commands.rs) |
| 3 | Tool programs in embedded V8 | `code_mode` runs loops and filters locally; batch execution respects tool effects and conflicts. | [V8 runtime](../cockpit/src/agent/code_mode.rs) |
| 4 | Repository intelligence before model calls | Scoped reconnaissance, filename search, outlines and batched definitions; optional LSP queries. | [Harness](../cockpit/src/agent/harness), [tools](../cockpit/src/agent/tools), [LSP commands](../cockpit/src/app/control/commands.rs) |
| 5 | Content-anchored edits | Hashline edits bind line operations to file content; stale anchors require recovery or rejection. | [Hashline](../cockpit/src/agent/hashline.rs) |
| 6 | Managed context across long sessions | Stable prompt prefixes, result aging and deduplication, and rolling compaction control retained context. | [Harness](../cockpit/src/agent/harness), [compaction](../cockpit/src/agent/compaction.rs) |
| 7 | Reviewed knowledge connected to source | Atlas records observations, relationships and review decisions; accepted project knowledge can enter later task context. | [Atlas](../cockpit/src/knowledge/atlas), [commands](../cockpit/src/app/control/commands.rs) |
| 8 | Persistent autonomous work | Goals, sessions and loop evidence survive resume; bounded turns and repeated-poll detection can stop unproductive execution. Bounds are configurable. | [Sessions](../cockpit/src/knowledge/session.rs), [loop controller](../cockpit/src/drive/loop_ctl) |
| 9 | Structured, attributable headless runs | Task JSON records runtime settings, source identity, tool activity and optional rollout/acceptance bindings. | [Runner contract](../cockpit/docs/COMPETITION_RUNNER.md), [runner smoke](../scripts/release/verify-runner-smoke.mjs) |
| 10 | Measured reinforcement campaigns | Isolated attempts produce verifier results; policy installation requires the campaign's independent audit path. | [Campaign tool](../cockpit/src/agent/tools/rl_campaign.rs), [campaign tests](../tests/cockpit/app/rl_ctl__campaign_tests.rs) |
| 11 | Optional research within a loop | Sloptomizer supplies Pareto, bandit and memory suggestions plus paired experiments; Deli supplies bounded fresh-context deliberation. | [Research options](../cockpit/research/sloptomizer/README.md), [Deli](../cockpit/src/drive/deli.rs) |
| 12 | Measured paired-sample arithmetic | `benchmark_compare` calculates changes from supplied paired samples. | [comparison](../cockpit/src/agent/tools/benchmark.rs) |

Research execution and feedback are implemented; general task-quality gains and
provider-weight training are not established. Upstream work is credited in
[attributions](../THIRD_PARTY_NOTICES.md).
