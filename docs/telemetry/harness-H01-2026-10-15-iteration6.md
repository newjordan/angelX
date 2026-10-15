# H01 — iteration 6: failed adaptation-scope analysis, gate preserved

## Hypothesis and phase

Native loop `1792029422798-3790707`, iteration **6/8**. This was a prospective, exploratory **source-only** check: do caller-level request adaptations share the inner HTTP retry admission budget and cancellation/deadline guard? Alternative: no rebuild path exists or the budget/guard is shared across all rebuilds. `scope.md` and `analyze.py` were index-hashed before the one execution. This is not a new treatment, independent audit, H02 registration, or live trial. H01 remains **blocked / no promotion**.

Selected frontier: `q.angel-3d0199f7046f545e561ac9ea` (technical research only). The established route to the abstract-model claim was closed, and native plan returned no charted doors/attempts; the source question was explicitly exploratory, not an established passage or tier promotion. No old game work was resumed.

## Exact execution and negative result

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
  python3 -B artifacts/harness-research/20261015-local/H01/iteration6-adaptation-scope/analyze.py
```

One invocation, **exit 1**, **1.008 s**, no timeout. Failure: citation `vendor/ureq/src/abort.rs:1-58` exceeded the actual **57-line** file. **194 checks recorded: 193 true, 1 false**. These are mostly integrity/citation checks, not product outcomes or 193 successful retry cases. Preserve the false check, original script, stderr traceback, command receipt and empty stdout. No correction/retry under the one-shot rule. Analysis script SHA256 `b494b2327445ac92a37ad2792fc0c4f73b9c22d599fd3696643242f420dcca3c`. Stderr SHA256 `8038d2b20d868d619dafcf262d1fd20690bf2dd72c1b2537f5fd439a7961f9f5`.

The analyzer stopped **before** saving its source-evidence bundle, baseline comparison, case table, later semantic assertions or `result.json`. Those files were not fabricated. `closeout.json` records the failed result; `manual-source-notes.json` is separately labeled source quotation from the manual reads, not a passed replay. Existing native loop tool history retains reconnaissance errors; `negative-observations.json` summarizes them without relabeling them product failures.

## New, still-unverified lead

Source review exposes a specific gap worth a future separately controlled case:

- `http.rs:3071-3133` snapshots `x-should-retry` and carries the veto marker through `provider_status_label`. `retry.rs:161-167` still constructs a string beginning `HTTP 400` for a vetoed 400.
- `http.rs:818-850` accepts `HTTP 400`/`HTTP 422`/`api error:` by prefix when the body names a reasoning/image field; these predicates do not inspect the veto marker.
- `http.rs:2792-2855` can learn a new rejected capability. `http.rs:3763-3777` can then rebuild/re-enter the streaming request (only without emitted output; reasoning learning also requires `origin.is_none()`). This suggests a vetoed capability rejection may lead to another **adapted** request even though inner unchanged-request retry was vetoed.
- This is **conditional source reasoning, not a measured provider-veto violation or physical-send count**. Carried-field state, fresh capability state, origin/identity gates, cancellation and route semantics matter. A provider may distinguish modified request adaptation from retrying the same request; that contract needs explicit specification. Do not disable valid capability discovery on this evidence.
- `http.rs:4112-4139,4314-4326` also has a separately bounded stream-rule loop around new inner calls. Cancellation is passed to each call; the entry guard is at `2937-2939`. Deadline-to-cancel watcher source is `turn/verify.rs:511-575`; abort watcher source is `club/sse.rs:5-43`. Guard presence does not measure cancellation latency or prove race-free suppression.

The stored H01 production patch changes only the permanent-transport branch in HTTP, the classification helper and the turn classification marker; it does not target this lead. The planned automated baseline/candidate function comparison **did not run**, so do not count a completed ablation here. These quotations are in `manual-source-notes.json` with whole-file/excerpt hashes. No follow-up fix or new H01 factor was added.

## Revalidation completed before failure and integrity closeout

- **283/283** preexisting indexed artifact hashes matched before analysis; the index-before bytes were unchanged at closeout (before the new closeout events).
- **13/13** frozen source/patch/test-executable/release/helper identities matched before failure and again at closeout. Current HTTP SHA256 `f002d05760948add37e9e6ee5902aa720cfcdd6234e41b4627c8af67eac31398`. See `identities.json` and `closeout.json` for every actual expected/observed hash.
- Original snapshot and current native-runner resource inventories each contain **1,677 files**; snapshot digest check passed and the only content deltas are the three frozen H01 production files. Raw map SHA256s: snapshot `d2b009dbb32e84001f465f48ea0107e84cfc0652a24476b5685b5a64f8876a65`; current `99c35ca63f2327eab36d2903da7e437eb7f492097657724fce61b5fb50bda022`. These raw-file hashes are not mislabeled as the native spaced-JSON content digest.
- HEAD `7d711a031cc3fbdda9112c1be29a104289a06524` and all tracked diff bytes were unchanged during the block. Current clean-HEAD source digest `af0a99586aa86e753916f591e107bbc9fb55c5a6765f700f25cecf18210eeeb3`. Dirty working tree source helper returned expected **exit 3 / unbound**; the clean-HEAD digest is **not** the candidate digest.
- Release identity-only `--build-info --json` ran (no product tests). Release remains source-stale/resources-unbound, hence **not qualified**. Existing H01 baseline/candidate exact diff and original compile flags remain in the preserved preregistration/patch/build receipts; no new build profile/toolchain was measured.
- Outer selected environment had `ANGEL_YOLO=1`; it was preserved, not silently described as off. Analyzer and its child commands explicitly used both YOLO flags `0`. Live process caches/effective retry knobs were not exercised. Python socket audit hook is not a kernel/transitive network sandbox. No safety control was weakened.
- Analyzer used **10/14** read-only child commands. Two more closeout git reads: **12/14 total**, all command stdout/stderr/start/end/exit hashes retained. One analyzer invocation; no retry, builds, product-test runs, provider calls, remote jobs, submissions, external publication, settlement manifest or resource expansion initiated. No claims about unrelated process activity.

## Decision, limitations, next action

**FAILED exploratory analysis; retain evidence and keep H01 BLOCKED / NO PROMOTION.** The preregistered source-only acceptance threshold was not met. Manual leads do not repair that result. Case physical-send counts, recovery outcomes and cancellation outcomes are **null / unmeasured**; zero newly initiated sends is a research-activity count, not a case outcome. No live improvement, task efficacy, independent validation, holdout/peer comparison or leadership claim.

The original same-fixture transient confound, cancellation EOF-versus-timeout oracle weakness, dirty candidate binding and exhausted two-build budget remain. H02 is **unregistered** and separate. No new production/test edits, extra verifier runs or control changes.

**Pause at the product-execution gate.** Ask for finite authorization to correct the existing fixture EOF/timeout oracle, replay matched baseline/candidate deterministic H01 cases and obtain an independent audit, with explicit build/run ceilings, physical POST counters and recovery/cancellation endpoints. No live-provider authorization is implied. If the veto/adaptation lead is pursued later, preregister a distinct contract/case after the H01 decision rather than stacking it into the current candidate. A repaired source analyzer would likewise require a new timestamped scope/run allowance, not replacement of this failed artifact.

Evidence directory: `artifacts/harness-research/20261015-local/H01/iteration6-adaptation-scope/`. Append-only native evidence link: `artifacts/harness-research/20261015-local/index.jsonl`.
