# H01 iteration 5 — retry-counter attribution and evidence gate

**Decision: BLOCKED / NO PROMOTION.** H02 remains unregistered. No leadership, efficiency, safety or operational improvement claim is made.

## Hypothesis and phase

H01's permanent-configuration containment candidate is unchanged. This iteration is a bounded **author/source-analysis follow-up**, not another product arm or independent review. Question: can a terminal transport diagnostic understate admissions after mixed rate-limit and ordinary failures? Alternative: counters reset/synchronize, or 429 attempts do not enter accounting.

Protocol and preflight were reread, including the rule that a validated analysis or justified rejection is useful work and does not require manufacturing product churn. The native Labyrinth frontier lists technical door `q.angel-3d0199f7046f545e561ac9ea` as prerequisites-ready. The local plan query provided no additional subdoors. Old game doors were not pursued. No new campaign/orchestration service was introduced.

Prospective scope, exact seven cases, alternative/removal control, closed-model enumeration, binary criteria, budgets and stop rules are frozen in [`scope.md`](../../artifacts/harness-research/20261015-local/H01/iteration5-mixed-budget/scope.md), appended to the existing index before the analysis. This is expressly **exploratory**: it does not amend or backdate H01's original efficacy registration. Both original preregistration and previous failed/confounded runs remain intact.

## Identities and effective controls

- Current HEAD: `7d711a031cc3fbdda9112c1be29a104289a06524`. Prior iteration 4 HEAD was `10d75eb64a95f670ef87d323234b1e65b6c39378`. This is **cross-iteration source drift**, not a new candidate commit. HEAD and full tracked binary diff stayed identical within the bounded analysis. No product, test or unrelated edits were made by this iteration.
- Frozen H01 baseline: `baseline-http.rs`, `baseline-retry.rs`, `baseline-classify.rs` and original test prefix. Baseline registration HEAD: `665569ee46208ca82a2d28e10b93f5054594e283`.
- Candidate: dirty working-tree patch, **no candidate commit**. Production patch SHA-256: `fec24d740181d895f695ccec8d7fb14766d68f289c66392ee08716d9290ea3d6`. Candidate tests patch: `7dfa5b9b7a34afc17aaad77054ac80448ec6893c6aa68e759871eea00f704e99`.
- **220/220 indexed artifact hashes** matched (217 incoming entries plus outer-controls/scope/script entries added before execution). **13/13 frozen identities** matched, including candidate source/test/patches, original baseline artifacts, candidate executable, release binary and helper. Exact list/hashes are in `identities.json`.
- Release binary SHA-256: `b41f58c70a21cec95412b9bbc2a926cca46f53de54e811d1e4fcf0eca23bf9d4`; helper: `1bebf47d10e03b4144788f5e78b058914d73222afbf47b4bb9031cad5b0d9996`. Release source pin is `5c384d7c1c456c2b00687dc570e19547fcebc8183041c401b8709ef5a27cb9bb`; current clean-HEAD digest is `af0a99586aa86e753916f591e107bbc9fb55c5a6765f700f25cecf18210eeeb3`. They do **not** match. Working-source digest exits **3** because build inputs are dirty; working source remains **UNBOUND**.
- Original resource snapshot: **1,677 files**, digest `8448e2a7e9c608135adc09b9a99843bd8dbccc723f5a46b0e06aedae6282033f`. Current inventory: **1,677 files**, digest `a6515caabdd5ffee7a7a822e8d046bf89034fa4e44a884a6a3e02f92c7124307`. Exactly three differing resources are the frozen H01 production files. Snapshot and current inventory are separately retained; this does not bind resources to the binary (build-info says `unbound`).
- Outer inherited `ANGEL_YOLO=1` remains visible in `outer-controls.json`; analysis and its children explicitly used `ANGEL_YOLO=0`, `ANGEL_YOLO_SMART=0`, `PYTHONDONTWRITEBYTECODE=1`. Outer state was not changed. No acceptance/runtime-required configuration was qualified. These are analysis environment controls, **not proof of effective product execution controls**.

## Concrete new result: transport diagnostic is not a total-admission counter

Source evidence in `source-evidence.json` binds each excerpt to whole-file SHA-256 and line range:

1. `http.rs:2934-2936` initializes ordinary and rate-limit counters once outside the retry loop.
2. `http.rs:3103-3127` selects and increments **only one counter** according to status. Transient 429 consumes the rate-limit budget, not the ordinary budget.
3. `http.rs:3036-3054` creates an accounting attempt before each `req.send_bytes`; `accounting.rs:112-117,395-400` records even attempts without usage on drop. This is admission bookkeeping, not proof that bytes reached a server.
4. `http.rs:3135-3158` increments the ordinary counter on eligible transport retries; terminal transport diagnostics still format `ordinary_attempt + 1`, omitting previous rate-limit retries.
5. The preserved baseline has the **same reporting mechanism** at `baseline-http.rs:3103-3153`. H01's configuration guard did **not** introduce it.

Fixed source-model cases with ordinary=1 and rate=1 (each evaluated for baseline and candidate):

| Case | Modeled admitted attempts, baseline / candidate | Diagnostic, baseline / candidate | Modeled terminal outcome |
|---|---:|---:|---|
| transport → transport | 2 / 2 | 2 / 2 | Transport failure |
| **429 → transport → transport** | **3 / 3** | **2 / 2** | Transport failure |
| 503 → 429 → success | 3 / 3 | none | Recovered answer |
| 429 → 429 | 2 / 2 | none | HTTP 429 exhausted |
| veto 503 | 1 / 1 | none | Veto |
| permanent configuration | 2 / 1 | 2 / 1 | Baseline transport exhaustion / candidate configuration stop |
| 429 → cancel before next iteration | 1 / 1 | none | Cancelled before next admission |

**Every cell is a hand-authored abstract model, not Rust execution, physical POST counts, observed recovery, or observed cancellation.** This analysis rejects the specific use of that diagnostic as an exact total-admission oracle. It does not demonstrate missing accounting, excess network sends, or a product cancellation failure. The baseline removal control establishes the limitation is preexisting, not attributable to H01.

For each ordinary/rate budget pair 0..2, both arms enumerated terminal prefixes over success/429/503/transport/veto/permanent. Across **18 closed-model budget/arm settings**, the maximum modeled admissions equaled **1 + ordinary retries + rate-limit retries**. Non-configuration traces matched across arms. These are finite checks of this model only, not exhaustive product validation. Real defaults are ordinary=1 and rate=5 (`retry.rs:6-7,107-112`); H01's fixture uses 1 and 1. Extrapolating the model gives a per-inner-call bound of 7 at defaults, but **7 was neither a tested product bound nor a task/turn bound**. The outer turn loop may be unbounded when `ANGEL_PROVIDER_RETRIES` is unset (`turn/mod.rs:3194-3199`). No new default-budget efficacy claim is made.

## Exact check and receipts

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
  python3 -B artifacts/harness-research/20261015-local/H01/iteration5-mixed-budget/analyze.py
```

This was the sole invocation in the new immutable attempt directory. The script uses exclusive output creation; reproduce only into a **new** directory, never overwrite this attempt.

- Invocation **exit 0**, **1.551 s** including launcher overhead; internal wall **1.522 s**.
- **240/240 analysis checks**, **14 fixed arm/case checks**, **12 read-only child commands**, within the frozen 60 s / 14-command ceiling. These are not counts of solved product tasks.
- Each child has exact argv/cwd/environment overrides/start/end/exit/stdout/stderr hashes in `command-01.json` through `command-12.json`, with raw stdout/stderr logs. Includes `git` identities/diffs, source-digest commands, release `--build-info --json`, load/memory/disk inventory and tracked-script listing.
- `analysis-invocation.json` records the launcher receipt; `checks.json`, `fixed-model-cases.json`, `closed-model-enumeration.json`, `result.json` and `source-evidence.json` retain results and coverage.
- Python analysis process denied socket audit events and restricted subprocess executable names. This is **not a kernel network sandbox** and does not instrument children transitively; zero network claims are based on the closed local command/code paths, not independent syscall observation.
- **0 new builds, product-test executions, socket-using commands, physical sends, provider calls, remote jobs, submissions or external publications initiated by this block.** Product recovery and cancellation outcomes remain **unmeasured/null**. Prior physical-send receipts are unchanged; this block adds no new send evidence.

## Negative results and limitations

- H01's two-build budget is already consumed. No authorized replenishment, same-fixture Rust replay, corrected cancellation-oracle run, independent reviewer, live provider call, or remote/holdout/peer evaluation was performed. No model delegate/campaign was launched to evade this gate.
- HEAD changed since iteration 4 and the release/source/resource identities remain unqualified as above. This precludes claiming a fresh matched product execution, even though the frozen local artifacts still match.
- Separate counter logic and reporting were guarded lexically; enumeration executes a **Python transcription**. It omits outer adaptations, context rebuilds, token refresh, discovery, compaction, pooled replay, deadlines and active-socket cancellation. It cannot validate those mechanisms.
- Two post-analysis **output-inspection shell commands failed with FileNotFoundError** after guessing optional identity-output filenames. The actual analysis had already completed and was not rerun. `negative-observations.json` preserves the failures and native history retains exact tools/results. No absent logs/results were fabricated.
- The preexisting same-fixture baseline confound and cancellation EOF/timeout oracle weakness remain unresolved. No combined H01 success inference is possible.
- This is author analysis only, not independent audit, H04 workflow evaluation, safety proof, capability-discovery validation, or comparative leadership evidence.

## Decision and next action

**Keep the diagnostic-oracle limitation as source-supported evidence; do not promote or broaden H01.** Preserve original candidate and all negative/failed attempts. H02 remains separate and **unregistered**.

Pause product execution at the recorded gate. Request a finite supplemental budget and matched-source scope for the existing H01 deterministic loopback tests (plus corrected EOF/timeout oracle), with exact physical POST, admission, recovery/cancellation and resource/control receipts and an independent audit. If the diagnostic is fixed later, register it as a separate follow-up factor after H01's decision rather than silently stacking it into H01. No live authorization is implied by a local replay authorization; provider/remote/peer trials require their own finite ceilings.
