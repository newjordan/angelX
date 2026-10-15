# H01 — post-decision identity and evidence audit, iteration 1

**Hypothesis:** typed permanent transport-configuration errors must terminate at both HTTP and turn retry layers without losing bounded transient recovery.

**Phase:** post-decision local integrity analysis; no new treatment or experiment. Native loop `1792029422798-3790707`, iteration 1/8. **Decision: blocked / no promotion.** Keep the existing uncommitted candidate, retain negative results, and pause at identity, audit and authorized-budget gates. H02 remains unregistered. This author analysis is not independent review, experimental replication, a performance benchmark or a leadership claim.

## New findings

1. **The runtime bundle is no longer current-source qualified.** HEAD advanced from `665569ee46208ca82a2d28e10b93f5054594e283` to `10d75eb64a95f670ef87d323234b1e65b6c39378`. The committed changes are four embedded `crates/web/shell/` files (72 insertions). They are unrelated work and were left untouched. The angel-source-v3 clean-commit digest is now `7cb6f6985f44f97b85fe6fe8e627ca71193e3c8252bb164f0053547dec001dac`; the preserved release binary still reports `5c384d7c1c456c2b00687dc570e19547fcebc8183041c401b8709ef5a27cb9bb`. The default dirty-tree digest correctly exits **3 / unbound**. Do not bind this dirty workspace to the clean-commit digest.
2. **The original snapshot survives; current resources differ.** All 1,677 snapshotted files still hash to the original native-runner digest `8448e2a7e9c608135adc09b9a99843bd8dbccc723f5a46b0e06aedae6282033f`. The same current inventory hashes to `a6515caabdd5ffee7a7a822e8d046bf89034fa4e44a884a6a3e02f92c7124307`, with exactly three changed entries: H01's `http.rs`, `retry.rs` and `turn/classify.rs`. Auxiliary resources do not include `crates/`; their digest cannot substitute for the source-v3 embedded-source identity.
3. **The tested candidate and raw evidence are intact.** Four candidate implementation/test files, both retained patches, and the candidate test executable match their frozen hashes. All 22 index entries present when this analysis began match their referenced bytes. The original index prefix and pre-H01 test content remain intact. Release binary and sandbox-helper hashes still match preflight. Git status was unchanged across the analysis.
4. **Native process completion is not the test outcome.** The baseline wrapper exited 0, but its embedded verifier receipt correctly reports Cargo **101**. Both native process receipts agree with archived receipts. The baseline native proc log is a 7,145-byte preview, not the full 12,307-byte raw log; the complete archived baseline log is present and hash-matched. The baseline test executable occupied the candidate's current Cargo deps path and was overwritten; its hash is recorded, but a separately frozen baseline executable was not retained.
5. **Control coverage is narrower than production qualification.** Recorded test-child overrides disable both YOLO flags; the source parser resolves those values to Guarded and test builds reread env. However, the existing `root_turn_env()` fixture helper also disables several behavioral/completion/recording gates and clears task acceptance/deadline variables. These test-only settings did not alter production controls, but the fixtures do not qualify production acceptance/safety policies. No emitted effective headless runtime/sandbox receipt exists. The analysis result's environment allowlist describes its overridden child, not the outer worker. A separate allowlist-only shell observation still found inherited `ANGEL_YOLO=1` (`ANGEL_YOLO_SMART` unset); no task was launched under it. Source/control details, that observation and limitations are retained separately.

## Evidence and unchanged H01 outcome

The earlier invalid-endpoint regression remains **6 -> 1 admitted attempts**, terminal ProviderError and zero possible physical sends for both URL/scheme cases. This is not a six-to-one network-send reduction.

Re-extraction of the unchanged candidate raw log gives 1 POST each for success, auth failure, malformed request and retry veto; 2 POSTs each for recoverable 503, 429 and disconnect: **10 POSTs plus 14 discovery GETs**. Cancellation/deadline/context-rebuild guards passed as mock-call assertions; physical cancellation and nonzero Retry-After recovery remain unmeasured. The paired network inference remains **inconclusive** because the original baseline discovery-GET fixture failed and was repaired before the candidate run. That negative result is retained, not scored as a product failure or removed.

No new Cargo build or test run was performed: H01's two registered compilations were already consumed. No candidate source was changed. The existing smallest product verifier remains the previous registered `cargo test ... -- h01_ --test-threads=1 --nocapture` (baseline 1 passed/2 failed; candidate 3 passed). An audit-script exit is not a new green product verifier.

## Exact local analysis receipt

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
  python3 -B artifacts/harness-research/20261015-local/H01/iteration1-audit/analyze.py
```

One invocation; exit 0, 1.316 s inclusive wall (diagnostic only). Analysis log SHA256 `e36ec73436c3906c8c6cc13453f60fe8d821912b3053f327225ee364d856dbbf`. Exact subprocess argv/env/status, raw outputs, inventories, per-case log line references and findings live in `artifacts/harness-research/20261015-local/H01/iteration1-audit/`. This is a one-shot local analysis, not a new orchestration layer. Index: `artifacts/harness-research/20261015-local/index.jsonl`. Artifact bodies are locally retained/git-ignored, not silently available in a fresh clone.

The baseline/candidate raw-log SHA256 values remain respectively `43bcfc3c8f9b8519f6f4de3324e48f123183fef51b7c067381c55b1336707e0a` and `8ece0ddc990136c39558200864ee2febfa68ab389fd622f24131e9c71d61d5e6`. Candidate test executable remains `206107ee2177913cb9e2c8d0de942e95638b59921fb1828c8abd9f550b7cc898`.

## Limitations, stop and next action

No independent reviewer, same-fixture replay, untouched holdout, matched peer comparison, optimized candidate build, physical cancellation measurement, task solve rate, token/cost coverage or effective live-route controls are established. No H02 treatment, live provider call, remote/model job, submission, external publication, new service or resource expansion was requested. No settlement manifest/deposit was made because publishing is not authorized. Earlier reports/sources remain unchanged.

**Next action:** obtain authorization for an independent exact-byte review and a fresh finite-budget H01 same-fixture replay, with explicit build/execution ceilings and frozen baseline/candidate executables. Reconcile current source/binary/resource identities and require emitted effective controls before any live cell. A replay should retain the corrected discovery fixture in both arms and separately preregister physical cancellation and nonzero Retry-After coverage. Keep H02 separate pending H01 audit/disposition. Do not rebuild or reset the shared tree to conceal the identity mismatch.
