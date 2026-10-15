# H01 — iteration 4: distinguish generic replay, rebuilt requests, and receipt semantics

Native loop `1792029422798-3790707`, iteration 4/8; UTC 2026-10-15.
**Phase: bounded post-decision author/source analysis. Decision: BLOCKED / NO PROMOTION.**
No H02 registration, new production/test edits, product execution, or efficacy claim.

## Question and hypothesis

H01's original hypothesis remains unchanged: typed permanent endpoint-configuration containment should reduce admitted HTTP attempts for invalid endpoints from six to one without losing registered transient/recovery/cancellation behavior. Original registration, candidate, red/green logs, fixture confound, exhausted two-build budget, and blocked decision are preserved. This iteration is NOT an additional product arm or a repaired paired experiment.

A new exploratory question emerged while inspecting the remaining recovery path: does the recorded `retryable=false` mean *no later provider attempt*, or only *no ordinary replay of this request*? The analysis hypothesis was that a no-answer context overflow may be recorded generic-retryable=false yet take the separately bounded compact-and-rebuild branch. The alternative was another guard/schema invariant that prevents continuation. Source had already been inspected when this question was recorded; this is exploratory, not retrospectively preregistered efficacy evidence.

The timestamped scope and freeze are appended to the existing experiment index. Labyrinth's technical door `q.angel-3d0199f7046f545e561ac9ea` had prerequisites ready; its plan query returned no additional charted subdoors/attempts. No game door was pursued and no new controller/campaign was created.

## New finding: recovery is distinct from generic replay

Implementation evidence is retained in `source-evidence.json` with exact line ranges, full-file SHA256 and excerpt SHA256:

1. `cockpit/src/agent/harness/turn/classify.rs:37–54,69–99,133–143`: context overflow explicitly means the same serialized request cannot succeed and receives bounded compact-and-rebuild handling. Numeric HTTP 413 is permanent under the general 4xx classification; `retryable_provider_failure` returns false for permanent errors. It would be incorrect to infer that every context-overflow diagnostic string alone is classified permanent; the conditional cases use explicit HTTP 413.
2. `cockpit/src/agent/harness/turn/mod.rs:3031–3180`: `retry_allowed` is calculated and passed to `fail_policy_attempt` **before** context rebuilding. A no-answer, non-incomplete-stream, non-cancelled overflow with positive context budget and remaining recovery allowance can compact/fit history. Continuation requires changed **and** smaller history. It increments `overflow_recoveries`, not `provider_attempt`.
3. `turn/mod.rs:1655–1675`: overflow recovery has its own turn-scoped budget, default one and capped at two. Generic provider retry count is not that budget.
4. `turn/mod.rs:2665–2677`: the next main request-loop entry checks cancellation/deadline. This is a source guard, **not** a measurement of socket closure or cancellation latency. An opt-in synchronous model compaction path also exists (`harness/compact.rs:2068–2140`, `agent/compaction.rs:279–292`); its availability must not be omitted from a future physical-send measurement. The analyzer invoked neither compaction nor a provider.
5. `harness/rollout/recorder.rs:522–579` persists the passed `retryable` field in `InfrastructureFailure` and increments the attempt sequence. `harness/rollout/schema.rs:644–684` permits another same-step attempt after `ProviderFailedNoAction` **without consulting that boolean**. The partial-output branch, in contrast, requires `failure.retryable`.

Thus the proposed schema-validation blocker is **refuted by source**: a subsequent rebuilt no-action attempt is explicitly admissible even after generic-retryable=false. This does not demonstrate a product defect or an H01 regression. It identifies a consequential reporting boundary: consumers must not translate that boolean into “no more provider calls.” Independent review should decide whether the existing field is intentionally generic-policy metadata or needs more explicit recovery-scope documentation/instrumentation in a separately authorized change.

A retry-veto marker does not explicitly gate the context-rebuild branch. Conditional HTTP 413 plus `[x-should-retry: false]` can therefore take it under the other predicates. The retry helper describes the veto in terms of replaying the **same request** (`club/retry.rs:132–166`); a shrunk request is not identical. This is not evidence that a veto was ignored on an observed wire call, nor proof of duplicate POSTs.

## Single bounded analysis and evidence

Directory: `artifacts/harness-research/20261015-local/H01/iteration4-recovery-scope/`.

Exact invocation, after logical review and hash freeze:

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
python3 -B artifacts/harness-research/20261015-local/H01/iteration4-recovery-scope/analyze.py
```

- Exactly one invocation: **exit 0**, no timeout, **1.519 s** wrapper wall; analyzer result reports **1.492 s**. These are slightly different accounting boundaries, not benchmark timings.
- **187 author-analysis checks** satisfied, including per-file checks. These are NOT 187 product tests or independent cases.
- **12 read-only child commands**, each with argv, cwd, start/end, exit, environment overrides, stdout/stderr and hashes. Working-source exit **3** was admitted only with the documented dirty/unbound diagnostic; it was not normalized to clean-source exit 0.
- **161/161 indexed artifact entries** matched: the 159 preexisting entries plus this iteration's scope and invocation freeze. The index prefix was unchanged during analysis.
- **13/13 identity entries** matched: six original candidate source/test/patch entries, six preflight binary/helper/script/protocol entries, and the candidate test executable. The executable was hashed, **not run**.
- Original **1,677-file** resource snapshot and current **1,677-file** inventory were rehashed using the existing runner's `tree_manifest`/`json_digest` scheme, not a replacement serialization. Exactly the three original H01 production files differ from the original snapshot. Tracked diff bytes and HEAD were unchanged across analysis.

### Conditional branch table — not Rust execution

Fixed assumptions: no answer/incomplete stream; positive context budget; one recovery available; no intervening error; history shrinks where stated. The table is a hand-transcribed, source-guarded model. It does not execute extracted Rust, fixtures, or a server.

| Conditional input | Recorded generic retryable | Rebuild continuation |
|---|---:|---:|
| HTTP 503 | true | false |
| HTTP 413 + context_length_exceeded | false | true |
| HTTP 503 + retry veto | false | false |
| HTTP 413 + context_length_exceeded + retry veto | false | true |
| Permanent transport-configuration marker | false | false |
| HTTP 413, already cancelled | false | false |
| HTTP 413, history unchanged / shrink failed | false | false |

This table checked the exploratory branch distinction only; it is not a new verifier for H01 product acceptance. The existing context-rebuild fixture is cited in `source-evidence.json` but was not executed. No new Rust tests were authored to evade the exhausted execution budget.

## Current identities, resources, and controls

- Current HEAD: `10d75eb64a95f670ef87d323234b1e65b6c39378`.
- Committed HEAD `angel-source-v3`: `7cb6f6985f44f97b85fe6fe8e627ca71193e3c8252bb164f0053547dec001dac`.
- Current dirty working source: **UNBOUND**; candidate commit remains null. Exact candidate bytes still match their original identity, not a release-qualified commit.
- Release binary SHA256: `b41f58c70a21cec95412b9bbc2a926cca46f53de54e811d1e4fcf0eca23bf9d4`; self-reported executable hash matches physical file. Its built-in source remains `5c384d7c1c456c2b00687dc570e19547fcebc8183041c401b8709ef5a27cb9bb`, **not current HEAD**. Built-in resources remain unbound.
- Sandbox helper SHA256: `1bebf47d10e03b4144788f5e78b058914d73222afbf47b4bb9031cad5b0d9996`.
- Snapshot inventory digest: `8448e2a7e9c608135adc09b9a99843bd8dbccc723f5a46b0e06aedae6282033f`.
- Current inventory digest: `a6515caabdd5ffee7a7a822e8d046bf89034fa4e44a884a6a3e02f92c7124307`.
- Host observation: load averages 5.52/5.17/4.18; RAM 94,166 MiB total, 64,905 MiB available; workspace filesystem 117G available. No serving jobs, limits, caches, resources, or unrelated processes changed. No GPU use required/measured.
- `outer-controls.json`: outer tool shell **still inherits ANGEL_YOLO=1**; other allowlisted values are mostly unset, not proven disabled. Analyzer parent and all recorded children explicitly use both YOLO flags 0. This does not qualify the production acceptance/sandbox controls, and existing fixture helpers' behavioral overrides remain a limitation. No credential values were read or logged.

## Physical sends, recovery, cancellation, and negative results

| This iteration | Outcome |
|---|---|
| Builds / Rust product-test executions | 0 / 0 |
| New sockets / physical network sends / provider calls | 0 / 0 / 0, by no-socket analysis execution design; not a product wire measurement |
| Newly measured product recovery / cancellation | **NOT MEASURED / NOT MEASURED** |
| Remote jobs / submissions / publishing / resource expansion | 0 / 0 / 0 / 0 |
| Product source/test changes | none |

Earlier H01 physical POST counts and recovery/cancellation outcomes remain earlier receipts, not this iteration's measurements. The original changed loopback fixture prevents a clean paired efficacy inference; pooled replay and the weak timeout-as-EOF cancellation oracle remain unresolved. A successful source analysis resolves neither.

Exploratory reconnaissance errors are retained in native tool receipts and described in `logical-review.md`: missing guessed paths, ignored-path listing rejection, and no additional Labyrinth plan. Initial analysis draft errors (diagnostic spelling, guard shape, dirty stderr suffix) were corrected from actual implementation **before** the sole invocation; no failed invocation was discarded. A post-analysis display command created `outer-controls.json` and printed host data, then exited 1 attempting to display a nonexistent `identity-checks.json`; it did not change the completed analyzer result. This negative receipt remains in native history and is explicitly not treated as a green verifier. Prior failed analysis attempt and both original Cargo arms were not edited.

## Decision and next action

**Keep the evidence; do not promote the candidate. Pause at the execution/authorization gate.**

The analysis passed its local integrity/branch checks and narrows a previously unexamined receipt/recovery distinction. It supplies no independent audit, repaired paired regression, product cancellation measurement, live task benefit, untouched holdout, matched peer result, or leadership evidence.

The next useful step requires independently reviewing this interpretation, then recording a prospective finite authorization for the unchanged-fixture baseline/candidate replay and, if approved, the pooled-replay/cancellation and context-rebuild wire/receipt cases. Freeze controls, effective compaction mode, identities, exact send accounting, outcomes, build/execution ceilings and stop rules before executing. Do not silently replenish H01's consumed budget or relabel the original selected green as pass@1. H02 remains separate and unregistered; no live/remote/submission/publication authorization is present.

Evidence: `scope.md`, `logical-review.md`, `invocation-start.json`, `invocation-receipt.json`, `analysis.stdout.log`, `analysis.stderr.log`, `result.json`, `checks.json`, `conditional-branch-table.json`, `source-evidence.json`, identity/index/resource maps and per-command receipts in the directory above. Append-only links are in `artifacts/harness-research/20261015-local/index.jsonl`. This report is local, body-free research evidence, not external publication or independent verification.
