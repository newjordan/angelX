# Angel harness research program — protocol and launch prompts

Status: proposed research charter, prepared at the operator's request. No loop,
new standing goal, live-model trial, remote-rig job, or competition submission
was started by preparing this document. Dates here are UTC.

## 1. Objective and claim boundary

Make Angel exceptional at executing research/engineering tasks and at recording,
auditing, reasoning about, and presenting the resulting evidence. Demonstrate
improvements in verified outcomes, reliability, time, and cost through controlled,
reproducible experiments. "Best in the world" is the ambition, not an established
result. Any leadership claim must name the evaluated workloads, competitors,
model/route/effort, hardware, protocol, attempt rules, date, and uncertainty.

A pass on an easy cohort, an internal self-score, or a win on a selected failure
subset does not establish overall superiority. When credible matched peer runs
are unavailable, report internal improvements only. Official board results and
local evaluator results are different evidence classes.

Scope: technical harness and research workflow only. Preserve unrelated operator
work. Do not resume the old game/browser goal, add game content, change safety or
sandbox boundaries, publish, alter credentials/provider destinations, or expand
paid/remote resources implicitly. Explicit operator model/route choices win.

## 2. Starting evidence and preflight

Local offline preflight receipt:
`artifacts/research-preflight/20261015T015123Z/receipt.json`.
Human-readable result: `docs/telemetry/harness-preflight-2026-10-15.md`.

Read exact files rather than relying on old summaries:

- `docs/telemetry/handoff-angel-harness-2026-10-13.md` — latest follow-ups.
- `docs/telemetry/engineering-baseline-2026-10-13.md` — pilot and hypotheses.
- `docs/telemetry/research-angel-harness-2026-10-13.md` — methods and limitations.
- `docs/SPEED_LOOP.md` — existing experimental method and runner contracts.

Historical anchors, not measurements of the current checkout:

- Pilot: 12/12; 331.8 s cumulative task wall; 296.5 s model spans; 62 model
  calls; 432,698 input tokens, 55.1% cached. Terminal narration cost 41.3 s.
- Frozen TB2 steering baseline: Grok 4.7 low 13/20. Final sentinel v5 was
  5/7 on the selected fail-both subset, not a measured full-cohort score.
- TLS root union was fixed. Permanent-error retry handling remained follow-up
  work in the latest handoff; recheck current source before implementing it.
- Ordinary per-hop overhead was below 1% in earlier real-model measurements.
  Reopen micro-optimization only when current profiling warrants it.

The offline config-only runner is Luna-specific. Its success does not qualify
Grok/TB2 execution, prove route credentials valid, or measure current performance.
Existing TB2 adapter code lives in git-ignored artifacts: archive/hash the actual
adapter and its dependencies before using it. Do not silently treat it as shipped.

## 3. Research contract: register before changing code or running a cell

Use existing loop/research/campaign records as the native evidence authority.
Add a small append-only experiment index that links their IDs and physical
receipts; do not build another planner, learner, or orchestration layer just to
run this program. Store bodies locally in unique directories under
`artifacts/harness-research/<program-id>/<experiment-id>/<cell-id>/` and commit
concise, body-free reports under `docs/telemetry/`. Preserve failed candidates.

Every experiment must register:

1. ID, owner, timestamp, question, evidence motivating it, and predecessor.
2. A falsifiable hypothesis, predicted mechanism, alternatives, primary endpoint,
   practical minimum effect, and non-regression constraints.
3. Baseline/candidate source commits AND source digest scheme and value; exact
   diff; executable/helper SHA256; resource inventory SHA256; build flags/features,
   compiler; evaluator/adapter/task/cohort hashes. Capture dirty state; never
   normalize a shared tree. Freeze copies or isolated worktrees before measuring.
4. Model, provider route, effort, budgets, attempt count, deadlines, hop/output
   limits, concurrency, cache policy, run order/seed, and allowed interventions.
   Record effective controls, not merely intended environment variables.
5. Task selection, analysis unit, repeats/sample-size rationale, holdout boundary,
   exclusions, retry rules, infrastructure-failure classification, analysis,
   stopping rule, promotion criteria, and resource/spend authorization.
6. Baseline and treatment arms; an on/off or removal ablation for the claimed
   mechanism. If multiple changes interact, measure components and combination
   or explicitly limit attribution to the stack.
7. Exact reproduction and verifier commands, expected failure before the fix,
   expected pass after it, and abort conditions.

Do not backdate a hypothesis, relabel an exploratory finding as confirmatory,
change the primary endpoint after looking, resample tasks to improve a score,
or select a later successful attempt as pass@1. Amendments are new timestamped
records, with their rationale and which observations preceded them.

### Minimal append-only index event (illustrative schema, not a fake result)

```json
{"schema":"angel.harness-research/v1","program_id":"...","experiment_id":"H01","event":"preregistered","timestamp_utc":"...","hypothesis":"...","primary_endpoint":"...","protocol_path":"...","protocol_sha256":"...","baseline_commit":"...","candidate_commit":null,"cell_ids":[],"native_receipt_ids":[],"decision":null}
```

Later append `started`, `measured`, `audited`, `kept`, `rejected`, `inconclusive`,
or `blocked` events instead of rewriting the original registration. Each cell
manifest must carry actual commands, identities, timestamps, measurements,
coverage, and exit/verifier status. Unknown values are null/unknown, never zero.
Hash raw logs/manifests; retain original receipts, including unsuccessful ones.
Log concise decision rationale and alternatives, not private chain-of-thought.
Redact credentials and keep task bodies/private artifacts out of public reports.

## 4. Controls, analysis, and promotion

- Current qualified baseline first. Same optimized build profile, features,
  hardware/evaluator, model/route/effort, task contracts, and budget in both arms.
  Earlier v5 and current HEAD are not automatically interchangeable controls.
- Separate deterministic/stub tests (mechanism) from live trials (task efficacy).
  A successful fault-injection test is not a live speed or solve-rate claim.
- Use paired tasks and interleaved A/B then B/A blocks; preregister randomized
  block order where practical. Match cache conditions; label cold/warm trials.
  Record host load and provider incidents. Do not overlap competing serving jobs.
- Start with screening, then an untouched validation cohort. Two paired pilot
  rounds are screening, not proof of a population-level improvement. Choose
  confirmatory sample size from observed variance and the registered minimum
  effect before the confirmatory run; avoid repeated significance peeking.
- Count every attempt, timeout, cancellation, and infrastructure failure. Report
  valid scored performance plus operational all-attempt outcomes. Preserve
  invalid cells with reasons rather than quietly dropping them or scoring them
  as model failures. Compare jointly solved tasks only as a secondary analysis;
  never hide lost solves in a faster solved-only median.
- Primary product metrics: verified solve rate; cumulative all-attempt task wall;
  p50/p90; timeout/failure classes; model/tool spans; calls; reported cached,
  uncached, output and reasoning token usage with coverage; actual cost per
  verified solve only when pricing/usage are available. Do not add overlapping
  timing spans or fabricate token-based costs for token-blind routes.
- Report paired deltas and uncertainty at the task/block level, not independent
  token/call samples. With a small cohort, show the raw task table and limits;
  no false-precision confidence or broad equivalence claim from zero losses.
- Promote only after the relevant regression/negative tests pass, observed solve
  losses are absent, the preregistered practical benefit repeats, and there is
  no unexplained tail/cost/reliability regression. Use independent holdout
  validation and evidence audit. "Inconclusive" is a valid outcome, not a win.
- Run the smallest relevant development verifier once per unchanged candidate.
  Benchmark replications answer a distinct variance question; broad test reruns
  are not a substitute for experimental replication. Run a broader qualification
  gate only where required by the registered integration/release contract.
- Stop on protocol drift, identity mismatch, exhausted authorized budget,
  integrity/safety failure, or infrastructure requiring operator intervention.
  Preserve diagnostics; name the exact unblock action. Never lower the grader,
  read hidden verifier material to tune answers, or weaken containment to win.
- No shell `timeout` wrapper around builds or benchmark boots. Honor declared
  task/evaluator deadlines and cancellation; preserve terminal receipts.

## 5. Initial hypotheses and required ablations

These are proposed registrations. Pin numeric thresholds, exact cells, and
resource limits before implementation/measurement; do not call them completed.

### H01 — permanent-error retry containment (first local candidate)

Hypothesis: classifying permanent failures and coordinating HTTP/turn-layer
retry budgets prevents whole-budget burns without reducing transient recovery.

- Arms: current behavior vs candidate classification/budget behavior. Exercise
  both layers to expose multiplicative retries, not only HTTP unit behavior.
- Cases: permanent certificate/configuration failure; invalid auth after any
  documented bounded refresh; malformed request with existing bounded adaptation;
  recoverable 429/503/disconnect; provider retry veto; cancellation; deadline.
- Primary deterministic endpoint: physical send count and terminal error class.
  A declared permanent case must stop at its registered bound (normally one
  attempt absent an explicit supported refresh/adaptation), not consume the
  deadline. Transient cases must recover within the registered bounded schedule.
- Negative controls: candidate off; unchanged success path; recoverable failures.
  Preserve actionable diagnostics, TLS verification, auth semantics, Retry-After,
  cancellation, and explicit operator choices. Use stubs/fault injection first.
- Do not invent a live speed percentage from synthetic error cases. A later
  task-cohort trial must measure operational failure/tail effects separately.

### H02 — green-terminal for explicitly bounded acceptance contracts

Hypothesis: removing one unnecessary narration call after complete proven task
acceptance reduces task wall/calls/cost without premature completion.

- Arms: identical binary/configuration with feature off vs on if supported;
  otherwise identical builds differing only in this treatment.
- Only opt-in bounded/headless tasks. No mutation after proof; receipt must bind
  current workspace and cover the declared acceptance contract. A passing partial
  test is not completion. No unresolved error, advisory, or deliverable remains.
  Preserve ordinary interactive explanations and explicit output contracts.
- Primary screening endpoint: >=10% reduction in cumulative all-attempt task wall
  on the pinned pilot over interleaved paired blocks, with no observed solve loss;
  calls/uncached tokens and p90 are secondary/non-regression measurements.
  The historical 41.3 s is an opportunity bound, not a forecast or result.
- Negative set: partial verifier coverage, stale receipt, red-then-green, mutation
  after green, missing artifact, outstanding error, acceptance-command tasks,
  and interactive answer requests. Any premature completion rejects the change.
- Validate promising results on harder tasks and untouched holdouts. Do not
  bundle cache/routing changes into this ablation.

### H03 — stable context and cache reuse (after profiling)

Hypothesis: one identified source of avoidable prefix/schema instability increases
uncached input/calls without improving task outcomes.

- Inventory request contributions and actual cache coverage first. Change one
  producer or lifetime policy, not a wholesale prompt rewrite.
- On/off ablation with matched cold/warm cache regimes and provider support.
  Measure uncached input and task outcomes, not cache ratio alone. Preserve
  deferred tool discovery, memory utility, error recovery, and route privacy.
- Register the practical minimum effect after profiling, before the candidate.
  Smaller prompts or higher cache ratios alone do not establish faster solves.

### H04 — evidence, reasoning, and presentation quality

Hypothesis: a receipt-linked reporting/decision workflow prevents unsupported
claims and improves auditability at acceptable measured execution cost.

- Before improving this workflow, freeze a small audit fixture cohort containing
  known cases: stale-green proof, partial verifier coverage, missing usage,
  duplicated attempt, missing attempt, infra-invalid cell, confounded ablation,
  and an unjustified subset-to-overall claim. Separate holdout cases; label these
  as research-method tasks, not coding benchmark solves.
- Baseline vs candidate receive the same evidence and output contract. Score
  against known facts with deterministic checks where possible and independent,
  blinded review for judgement claims. Do not rely on the author judging itself.
- Metrics: false promotion/false claim count, correct uncertainty/failure
  classification, receipt/identity/command completeness, numerical fidelity,
  independent reproduction success, reviewer correction count, review time,
  task wall, and tokens/cost. Register rubric and effect threshold before runs.
- Reporting requirements: question -> hypothesis -> design -> results ->
  ablation -> limitations -> decision -> next experiment. Every quantitative
  claim links to a physical receipt/table; state unknowns and failed hypotheses.
- Execute H04 as its own treatment, not an unmeasured reporting tax bundled into
  H01/H02. Improve only necessary tooling; avoid self-referential machinery.

## 6. Benchmark ladder and comparative claims

1. Focused deterministic regression/negative tests for mechanism correctness.
2. Existing balanced 12-task Polyglot pilot for speed/cost regression screening.
3. Existing fixed TB2 steering-20 for task efficacy; keep holdouts untouched.
4. Frozen research-method fixture cohort for evidence/decision/presentation.
5. Matched peer harness runs and repeated independent validation for scoped
   leadership claims. Document route differences rather than disguising them.

Do not chase a selected seven-task score as if it were the full cohort. Do not
rerun old failed tasks indefinitely or credit an inactive mechanism for a flip.
No platform is selected by this charter: ZERO SUBMISSIONS at preparation. If an
operator later authorizes a real platform, bind its rules and current leader,
submit eligible distinct candidates, retain submission IDs, and inspect official
terminal results. A local receipt is never an official board score.

## 7. Per-iteration output and resource discipline

Each loop iteration ends with: experiment/phase; candidate and baseline IDs;
changed factor; exact verifier or cell receipt; observed metrics and coverage;
keep/reject/inconclusive/blocked decision; unresolved confound; next action.

Preregistration, a validated analysis, and a justified rejection are research
progress; do not manufacture code churn to look productive. No passive status
polling, repeated identical searches, or speculative feature stacking.

Start with a bounded eight-iteration local development block, not endless mode.
Live/remote trials require a recorded ceiling for attempts/spend and access scope.
If no ceiling is authorized, perform useful local deterministic work, then pause
at the live gate and ask for the missing authorization. Do not expand resources.
Do not reboot/prune/broadly kill anything or read credentials into logs. Use low
concurrency on shared hardware, check load before timings, and preserve all work.

## 8. Paste-ready controls

The old standing goal was still the game/browser goal at preflight. Check
`/loop status` and stop/pause an old unrelated loop before replacing the goal.
Preparing these prompts did not execute them. Use the workshop to confirm the
bounded loop's token/time settings; the commands below do not grant unlimited
provider spend. No universal `/goal cmd` is supplied: a passing smoke check would
not prove this research objective. Pin experiment-specific verifiers instead.

### Goal

```text
/goal Make Angel a demonstrably leading research and engineering harness in verified task execution, reliability, efficiency, evidence recording, decision quality, and presentation. Follow docs/telemetry/harness-research-protocol-2026-10-15.md. Treat this as controlled research: preregister falsifiable hypotheses, controls, ablations, metrics, budgets and stop rules; retain every attempt and exact identities/receipts; promote only independently audited, repeatable improvements; validate on untouched holdouts and matched peers before leadership claims. Preserve correctness, safety, capability discovery and unrelated work. Technical harness work only; do not resume the game goal. Unknown or inconclusive evidence is not success.
/goal criteria Receipt-complete preregistered experiments; no observed solve loss or premature completion; repeatable practical benefit under matched controls; ablations support attribution; holdout and independent evidence audit pass; all comparative claims are scoped and reproducible; failures and uncertainty remain visible.
```

### Initial loop

```text
/loop iterations=8 Read docs/telemetry/harness-research-protocol-2026-10-15.md and docs/telemetry/harness-preflight-2026-10-15.md. Execute a bounded scientific research block, not the old game goal. Revalidate current source/binary/helper/resources and effective controls; preserve unrelated work. Create the append-only experiment index and preregister H01 with controls, exact cases, thresholds, analysis and budget. Inspect current retry paths, author a failing deterministic regression, implement one minimal candidate, and run the smallest relevant verifier. Record physical send counts, recovery/cancellation outcomes, hashes, commands and all receipts; retain negative results. Keep H02 separate and preregister it only after H01's decision. Use existing native research/campaign records; no new orchestration layer. No live provider calls, remote jobs, submissions, publishing, safety weakening or resource expansion without recorded authorization and a finite budget; if missing, finish safe local work then pause at that gate. End every iteration with hypothesis, phase, evidence, decision, limitations and next action. Do not claim leadership from a pilot or selected subset.
```
