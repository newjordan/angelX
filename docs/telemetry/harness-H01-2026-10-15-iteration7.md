# H01 — iteration 7: capability-adaptation composition, source-only

## Hypothesis and phase

Native loop `1792029422798-3790707`, iteration **7/8**. Exploratory source-only question: is capability adaptation limited to one recursive re-entry across a call, or one successful transition **per learned capability**? Alternative: a shared gate, emitted-output guard, origin binding or body-carried condition prevents the two transitions composing.

The ready technical frontier was `q.angel-3d0199f7046f545e561ac9ea`. Native plan returned no charted subdoors/attempts; the route to the abstract-model claim was **explore**, not established or independently reviewed. No game work. This is an H01 evidence follow-up, **not a new production treatment or H02 registration**. The original H01 preregistration, failing regression, minimal candidate and consumed two-build budget remain unchanged. H01 is **BLOCKED / NO PROMOTION**.

`scope.md`, `outer-controls.json`, `analyze.py` and author-only `logical-review.md` were append-indexed before the sole analysis invocation. The iteration-6 failed script/traceback remain immutable. The new scope explicitly discloses prior manual observations; it does not retroactively preregister the already-observed learner branches. No new orchestration or campaign layer was added.

## Execution and exact evidence

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
  python3 -B artifacts/harness-research/20261015-local/H01/iteration7-adaptation-composition/analyze.py
```

**One invocation, exit 0, 1.046 seconds externally measured, no timeout, no retry.** `analysis-command.json` records argv, cwd, selected environment overrides, start/end, exit and script/stdout/stderr hashes. Script SHA256: `1d044a0879c1b427845043c6717de4bd431e0d0f0ce1e0fc49bf046a5b43bd20`. The analysis recorded **281/281 checks** and **10 fixed abstract cases**. Most checks are artifact integrity, citations and source anchors, **not 281 product tests or successful retry cases**. `checks.json`, `case-table.json`, `source-evidence.json`, `baseline-comparison.json`, `fixture-coverage.json` and `result.json` retain the derivation.

### New result: per-capability depth, not a whole-call bound

The frozen baseline and current candidate have byte-identical bodies for all **nine** inspected functions: the reasoning/image rejection predicates, both sticky learners, both serialization gates, `chat_with_effort`, `chat_streaming_captured`, and `body_carries_reasoning`. Line-number-free function hashes and exact line ranges/full-file hashes are retained. This is a source comparison, **not** a runtime ablation or independent review.

Each learner separately refuses an already-populated sticky rejection slot. The callers try reasoning adaptation first, then image adaptation, rebuilding through the same club instance. Rebuilding suppresses the learned field but can retain the other field. Consequently, under the explicitly closed assumptions below, the following source-model result is supported in **both baseline and candidate**:

| Fixed model case | Learned transitions | Caller-body invocations |
|---|---|---:|
| Reasoning rejection, image rejection, then modeled success | reasoning → image | 3 |
| Image rejection, reasoning rejection, then modeled success | image → reasoning | 3 |
| One carried capability, its rejection, then modeled success | one transition | 2 |
| Neither field carried / already-learned reasoning / already-emitted stream | none | 1 |
| Captured origin, reasoning rejection | none | 1 |
| Captured origin, image rejection, then modeled success | image | 2 |
| First error names both fields, next names image, then modeled success | reasoning → image | 3 |

There are ten actual rows; the table groups three separately retained no-learning rows and two single-capability rows. Each row in `case-table.json` records inputs, options and exact expected result.

**These are not physical send counts.** The model deliberately excludes inner transport/rate-limit/ureq retries, stream-rule rebuilds, turn retries, truncation recovery, discovery and concurrent mutation. It assumes fresh carried reasoning plus image fields where specified, one stable club, nonpoisoned sticky locks and the OpenAI-effort dialect. Model “success” is a supplied symbol, not observed recovery. It is not a global call-count bound or proof about all dialects/routes.

### Contract and fixture consequence

The two synthetic errors include `[x-should-retry: false]`. Source checks confirm that `provider_status_label` preserves the `HTTP 400`/`HTTP 422` prefix, and the two capability-rejection predicates do not inspect the veto marker. Thus the hand-transcribed detector still admits capability learning. This **narrows the iteration-6 lead**: an unchanged-request veto is not necessarily a veto on a modified, capability-adapted request. Provider semantics must be specified before classifying this as a defect. Do not disable legitimate capability discovery on this evidence.

The selected existing `a_provider_retry_veto_is_sent_once` fixture covers **503 and 429**, not capability-reject **400/422**. The selected GLM image fixture checks buffered `chat`, a stripped second request and `requests.len() >= 2`; it does not establish an exact upper bound, a two-capability sequence, streaming cancellation, or captured-origin behavior. **Neither fixture was executed here. No whole-suite absence claim is made.** This points to a specific missing comparison, not measured regression or a candidate effect.

## Revalidation, controls and preservation

- **338/338** indexed artifact hashes matched before the analysis, including the entire prior 334-entry index plus the four prospective records. The frozen index bytes remained unchanged during the analyzer. Closeout/index receipts record later additions separately.
- **13/13** frozen identities matched: production files, test source, preserved patch files, release executable, sandbox helper, source-digest helper, native resource runner, protocol/preflight and retained test executable. Complete expected/actual SHA256s are in `identities.json`.
- Original and current native-runner resource inventories each contain **1,677 files**. Only the three frozen H01 production files differ. Native snapshot digest: `8448e2a7e9c608135adc09b9a99843bd8dbccc723f5a46b0e06aedae6282033f`; current: `a6515caabdd5ffee7a7a822e8d046bf89034fa4e44a884a6a3e02f92c7124307`. Resource bytes were not expanded or edited.
- HEAD: `7d711a031cc3fbdda9112c1be29a104289a06524`; clean-HEAD source digest: `af0a99586aa86e753916f591e107bbc9fb55c5a6765f700f25cecf18210eeeb3`. The dirty working-source helper returned expected **exit 3 / unbound**. This is not a candidate source binding. HEAD, tracked diff and cited source bytes were unchanged during the analyzer; closeout rechecks preservation separately.
- Identity-only release `--build-info --json` still reports executable SHA256 `b41f58c70a21cec95412b9bbc2a926cca46f53de54e811d1e4fcf0eca23bf9d4`, embedded source `5c384d7c1c456c2b00687dc570e19547fcebc8183041c401b8709ef5a27cb9bb`, and resources **unbound**. The release is source-stale and **not qualified** for these candidate bytes.
- The outer selected environment has **`ANGEL_YOLO=1`**, unchanged. The analysis and its read-only children explicitly used **both YOLO flags 0**. No live process caches or effective product retry controls were exercised: `product_effective_controls` is **null**. A Python socket audit hook is not a kernel/transitive network sandbox. No control was weakened.
- The analyzer used **12/14** allowed read-only child commands; two closeout git reads bring the total to **14/14**. All child stdout/stderr and command receipts are retained. The expected dirty-source exit 3 is not silently relabeled success. Initial directory discovery hit an excluded-path error; that negative observation remains in native tool history and the closeout notes.
- Initiated research activity: **0 builds, 0 product-test executions, 0 sockets, 0 physical sends, 0 provider/model calls**, no delegates, remote jobs, submissions, publication/settlement manifest or resource expansion. This says nothing about unrelated process activity. For each new case, physical sends, recovery and cancellation outcomes remain **null / unmeasured**.

## Decision, limitations and next action

**The narrower source-only acceptance check completed; H01 remains BLOCKED / NO PROMOTION.** The whole-call “depth one” interpretation is not supported under the two-capability model. This behavior is shared with baseline and is not caused by the H01 permanent-transport candidate. No production/test edits or extra verifier/build runs were made, and negative prior attempts were not repaired or erased. H02 remains **unregistered**, separate from this follow-up.

The original matched-fixture confound, EOF-versus-timeout cancellation-oracle weakness, dirty candidate binding and exhausted two-build budget still prevent causal product qualification. Author-run source checks are not independent audit, actual cancellation latency, recovered-request evidence, untouched-holdout validation, matched-peer performance or leadership evidence.

**Pause at the product-execution gate.** Next, obtain explicit finite authorization for a corrected EOF/timeout oracle and matched baseline/candidate deterministic H01 replay, with build/run ceilings, identical transient controls, physical POST counters, positive recovery and cancellation endpoints, and independent audit. No live-provider permission is implied. If capability-veto adaptation is pursued as a separate treatment, specify the modified-request contract and preregister its exact cases after H01's decision rather than silently adding another factor now.

Report: `docs/telemetry/harness-H01-2026-10-15-iteration7.md`.
Evidence: `artifacts/harness-research/20261015-local/H01/iteration7-adaptation-composition/`.
Append-only index: `artifacts/harness-research/20261015-local/index.jsonl`.
