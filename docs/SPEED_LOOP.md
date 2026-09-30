# AngelX speed and routing improvement loop

Prepared from `dev` at `028b319` on 2026-09-26. This is the handoff for the next
AngelX session; preparing it does not start a background campaign.

## Start the session

Open AngelX against this checkout. Enter these commands separately:

```text
/cd /home/frosty40/angelX/cockpit
/self Read ../docs/SPEED_LOOP.md and execute its improvement cycle. Start with the message and routing inventory, then implement and measure one narrowly scoped candidate. Preserve correctness and capability discovery. Record evidence and the next experiment before integration.
```

`/self` creates an isolated branch/worktree from the current checkout HEAD. Its
existing build/test gate and operator integration step remain in place. It does
not hot-upgrade an already running binary. See [self work](SELF_MODEL.md) and
[loop controls](COMMANDS.md). Use the loop workshop to choose the session's
time, iteration and token limits; this brief does not change those settings.

## Objective and architectural hypothesis

Improve end-to-end task speed and system efficiency until AngelX can demonstrate
competitive results against OpenCode and omp on matched workloads. Treat
“less connected, more options” as a testable design hypothesis:

- Keep the ordinary path direct: one owner, the context it needs, and the tools
  needed now. Activate additional capabilities when a task requires them.
- Keep options discoverable without injecting every schema, memory, helper,
  notification or specialist into every model request.
- Give each internal message a recipient, purpose and lifetime. Coalesce
  duplicate status updates; keep UI-only progress outside model history. Retain
  actionable errors, cancellation, user steering and tool-result associations.
- Prefer existing discovery, routing and cache mechanisms. Avoid adding another
  planner, message bus, classifier call or cache layer merely to remove overhead.
- Spend additional reasoning or delegation only where evidence justifies it.
  Preserve explicit user requests for those capabilities.
- Keep ambient actors cosmetic and paced by existing rendering. They must not
  create harness work, model traffic or foreground programming dependencies.

Less input is not automatically faster. Historical Qwen requests already had
smaller messages and schemas than OpenCode while producing much more output.
Measure generation, repeated turns and failure recovery as well as input bloat.

## Evidence to read first

1. [Full 136-task audit](telemetry/polyglot-full136-slowdown-20260926.md): all
   255 discovered trace files classified, including five previously omitted
   full confirmation cohorts. This supersedes the earlier median-led board.
2. [Provider/request audit](telemetry/polyglot-provider-slowdown-20260926.md):
   output volume and the controlled Qwen continuation probe.
3. [Latest Rust30 run](telemetry/polyglot-rust30-20260926.md): 30/30, 1,084.43
   cumulative seconds versus 1,462.86 historically; one attempt per task, not
   causal evidence. Its binary corresponds to `7ad833c`, before clerks and the
   foreground timeout fix. Rebuild and pin a new baseline for current dev.
4. [Runner and earlier pilot](telemetry/polyglot-improvement-loop-20260925.md):
   runner usage and method. Its historical conclusions predate the full136
   audit and successful Qwen probe; use the newer reports for those findings.

Full136 cumulative agent-wall reference: Qwen compact 215.0 vs OpenCode 113.8
minutes; published GLM 174.1 vs 129.4; latest GLM 152.3 vs 129.4. DeepSeek's
latest total is faster despite its slower median. Do not hide failures or tails.
The recorded omp DeepSeek cohort is only 93 tasks; compare the matching subset
and label it partial. Muse recorded different model IDs. Luna has no matched
peer cohort in this collection. There is no established across-the-board win.

Local historical inputs are under
`/home/frosty40/angel_tests/angelX-bench/polyglot-20260921`; keep them immutable.
New evidence belongs in a unique `artifacts/speed-loop/<experiment>/` directory.
Artifacts are locally retained and generally not tracked by Git; commit concise
body-free findings and reproduction commands under `docs/telemetry/`.

## First experiment: identify and remove unnecessary traffic

Trace the active coding path before changing policy. Begin with
`cockpit/src/agent/harness/context.rs`, `harness/registry.rs`, the actual callers
of `cockpit/src/agent/swarm/routing.rs`, provider request assembly, and turn
history construction. The registry already supports deferred discovery and
cached advertised schemas; do not rebuild those systems. Verify which paths
run in interactive, bounded-task and explicitly requested swarm modes.

Produce a compact inventory for representative existing task traces and a
frozen current-dev pilot:

| Item | Record |
| --- | --- |
| Message/context source | Producer, recipient, activation condition, frequency, lifetime |
| Request contribution | Message count, schema count, bytes; tokens only when measured |
| Routing | Actual extra model calls, fan-out, fallback and repeated dispatch |
| Cost | Model wall, first activity when available, output/reasoning, tool wall |
| Cache behavior | Stable prefix/schema identity, invalidations, reported cached and uncached input |
| Necessity | Consumer that used it, removable duplication, discovery/fallback path |

Use existing telemetry first. If instrumentation is necessary, keep it bounded,
off the default hot path where possible, and free of prompt bodies, reasoning
text and credentials. Unknown measurements stay unknown. Do not infer active
fan-out solely from routing helper definitions or attribute provider queueing
to local messaging without evidence.

Select the largest measured avoidable contribution. Change one thing: for
example redundant context injection, premature schema activation, unnecessary
synthesis, or repeated error-retrieval turns. Preserve access to the capability
and test its discovery and recovery path. Record the predicted mechanism and
the metric that would falsify it before the candidate run.

## Repeatable improvement cycle

1. Record branch, commit, dirty state and candidate diff. Freeze matching source,
   executable, sandbox helper, resources and evaluator configuration. Do not
   benchmark an old executable against new source or compete for the same
   serving slot with another campaign.
2. Establish a current baseline. Use the deterministic balanced 12-task pilot
   for screening plus clearly labeled diagnostic cases. Keep model, route,
   effort, task contract, timeout, hop limit and concurrency matched.
3. Implement one candidate and run affected development checks. The existing
   `scripts/check/check-cockpit-fast.sh` accepts test filters; iterate without
   release LTO. Qualify and pin the final benchmark executable separately.
4. Compare at least two interleaved pairs (A/B, B/A) for a promising candidate.
   Record cache conditions and host contention. Include every attempted task,
   failed run and timeout. A smaller pilot screens candidates; it cannot prove
   a full136 lead. Preserve an independent validation cohort before tuning more.
5. Promote only after no observed correctness loss and a repeatable improvement
   in the intended metric without an unexplained tail/cost regression. If the
   effect is uncertain, retain the evidence and reject or revise the candidate.
   Do not accumulate speculative switches or stack unmeasured changes.
6. Validate promising winners on all 136 original tasks and a matched peer run
   when available. Repeat close comparisons. Keep repository-repair results
   separate from Polyglot. Scope any win to its model, workload and contract.
7. Commit a small reviewable change and report. Preserve failed experiments in
   the ledger. At the next iteration, read that ledger and choose the next
   measured bottleneck rather than restarting the audit.

Use `scripts/run-polyglot-loop.py --help` for the existing Luna runner: execution
requires `--execute`, and each cell requires a new output directory. That runner
is not a generic Qwen/GLM/peer runner. Use the recorded route-specific evaluator
contracts for other cells, never silently substitute a model or endpoint.
Use `scripts/summarize-polyglot-loop.py` for paired Luna cells and
`scripts/analyze-polyglot-full136.py` for the historical inventory. Explicit
`--task` selections support the full catalog; the default is only 12 tasks.

Each experiment report must include: hypothesis, source/binary/resource hashes,
exact commands and settings, matched task identities, all-attempt solve counts,
cumulative agent wall, median/p90, jointly solved deltas, failures/timeouts,
model/tool spans, calls, generated output/reasoning, cached/uncached input and
coverage, intended contract differences, limitations, keep/reject decision and
next experiment. Provider spans may overlap; do not sum them as disjoint time.

## Existing fixes and boundaries

- `7ad833c`: cache/tool/provider reliability work; `04a712f`: ambient clerks;
  `716c5a1`: full benchmark analysis; `028b319`: foreground timeout containment.
- The 120-second task-mode cap is per managed foreground process, not cumulative
  native-code preemption. Background jobs and fixed internal deadlines have
  separate policies. Preserve operator overrides and repair after timeout.
- Native OpenAI replay remains experimental and default-off. Earlier probes
  did not establish a native replay benefit. Do not conflate it with Qwen's
  plaintext reasoning continuation result.
- Qwen's four-call continuation probe showed 37.1% lower mean request wall,
  not a full-task win and not a cache hit. Any implementation needs explicit
  route/model ownership, transient private state and cancellation/route-switch
  tests before task-level measurement. Never replay private reasoning across
  provider routes or persist it through ordinary history/debug serialization.
- Preserve task prompts, graders, gold, verification quality and safety
  boundaries. Speed gained by weakening the benchmark is not an improvement.
- Existing focused checks passed; a green entire Rust suite has not been
  established by the last audit. Record actual gate outcomes, never inherit a
  blanket green claim. `/self` owns its own integration gate.
- Preserve unrelated local work, including `cockpit/scripts/launch-gentrim.sh`.
  Keep Git integration reviewable; this handoff does not authorize publishing
  benchmark contents, changing provider destinations or pushing branches.
