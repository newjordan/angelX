# H01 corrected local replay — 2026-10-15 UTC

## Current disposition

**KEEP — LOCAL MECHANISM QUALIFIED, independently audited.** The original baseline/candidate experiment and all seven follow-up analyses remain retained. This report covers a prospectively amended, matched local replay, not a live task benchmark or release qualification.

## Why execution resumed

The operator explicitly directed bounded local correction/replay without another permission gate. The earlier two-build limit was an internal experiment limit, not an external safety boundary. The next action was a prospective amendment and concrete execution, not more analysis-only iterations. No live benchmark-provider calls, remote jobs, submissions, publication, credential changes or game work were authorized or performed by this replay. Separate model inference for independent review is not benchmark-provider traffic or evidence of zero total model spend.

## Problems found and preserved

1. Original job wrappers exited zero even when Cargo exited 101. The new replay records actual child status and makes a separate explicit experimental verdict. An intentional baseline RED remains raw exit 101, never a green product-test result.
2. Original baseline and candidate network fixtures differed. Neither old result is relabeled as a matched network comparison.
3. A candidate-only helper reference prevented applying the exact old tests to baseline production. Corrected shared fixtures use baseline-compatible behavior checks instead.
4. Independent review found the original candidate's marker-anywhere classifier could treat an arbitrary transient HTTP response diagnostic as permanent. The corrected candidate requires an anchored, locally generated transport-error envelope with a numeric attempt count. A negative 503-body marker control crosses the inner retry budget into outer-turn recovery.
5. The actual cancellation false-positive oracle was `unwrap_or(0)` in the pooled-resend test, not the direct cancellation helper. It now requires `Ok(0)` and bounds accept/read. The direct cancellation tests already required EOF and are unchanged.
6. A second independent review found the first replay's baseline-RED classifier too permissive and its saturated peer timing could admit pre-cancellation closure. That replay is retained as preliminary only. V3 checks exact tests, failures, endpoint counts, POST/outcome counts and timestamp ordering, with cancellation observed at client return and peer EOF.

## Preregistered experiment

Common base: `7d711a031cc3fbdda9112c1be29a104289a06524`. Baseline production matches the original H01 baseline bytes. Frozen baseline and candidate share the exact corrected fixtures and all other tracked files; only the three declared production files differ. Frozen sources contain 3,284 tracked-file inventory entries each.

- Original amendment/source/binaries/logs: `artifacts/harness-research/20261015-local/H01/replay-20261015T052307Z/`.
- Corrected oracle v3: `artifacts/harness-research/20261015-local/H01/replay-20261015T053341Z-oracle-v3/`.
- Append-only index: `artifacts/harness-research/20261015-local/index.jsonl`.
- Evidence bodies are local/git-ignored and are not silently available in a fresh clone.

Hypothesis: permanent endpoint configuration stops at both retry layers, reducing admitted attempts from six to one without losing transient recovery, cancellation, error visibility or Retry-After handling. Invalid endpoints make **zero physical network sends in both arms**; the admission reduction is not a network-throughput or task-speed improvement.

Design: two frozen debug/no-default-feature test images, offline/locked compilation, identical compiler/build jobs, YOLO flags off; A/B/B/A targeted executions on a shared host. Synthetic loopback HTTP only. Compile/test wall time is diagnostic, not a treatment-effect metric. V2 had two builds/four runs; concrete independent-review diagnostics required a separately preregistered two-build/four-run v3 correction, without changing the production treatment. No blanket budget expansion or broad repeated suite.

Selected tests: three H01 tests, four existing direct physical cancellation tests and one corrected existing pooled-resend cancellation test. This is a deterministic local mechanism screen. There are no task-level solve-rate, live TLS/proxy handshake, cost, cache, untouched holdout, competitor or world-leadership results.

## Treatment and oracle identities

SHA256:

- Production patch: `667f687d2dbe347bbfedbf3f70ff507edac2ae63cb3eac50cbb87e031e6e69b0`.
- V3 fixtures patch: `55009757374c6784b0db037e64204b6f1d3a3c7335de5e586688109640a26982`.
- V3 replay runner: `667fee0625f7e7bcd7b01e85ada23a37da65c71fae7328f9ce04ff74f7de7b55`.

The debug test images are bound to retained source inventories/patches/executable SHA256, not claimed as source-v3-qualified release executables. Existing release binary predates this candidate and is not used to represent it.

## Independent review

Three separate read-only reviewer contexts found the original production and evidence defects, then checked their correction. Final v3 static review (`hnd_8i`, retained as `independent-static-review.md`) reported **STATIC QUALIFIED**, independently verified inventory/diff boundaries and the three hashes above, and explicitly withheld measurement qualification while builds were in progress. Same-model independent contexts are not cross-vendor corroboration. No reviewer executed the benchmark.

## Measured result and decision

| Run | Raw exit | Tests | not-a-url admissions | ftp admissions | Retry-After interval | Pooled peer EOF |
|---|---:|---|---:|---:|---:|---:|
| A1 | 101 | 7 pass; intended endpoint failure | 6 | 6 | 1001 ms | 7 ms |
| B1 | 0 | 8 pass | 1 | 1 | 1001 ms | 7 ms |
| B2 | 0 | 8 pass | 1 | 1 | 1002 ms | 7 ms |
| A2 | 101 | 7 pass; intended endpoint failure | 6 | 6 | 1001 ms | 7 ms |

All ten loopback controls matched across arms/repetitions: success/auth/malformed/veto one POST each; inner 503/429/disconnect two; outer 503 and transient marker-body case three; nonzero Retry-After two. Only auth/malformed/veto did not recover, as intended. Exactly 18 model-fixture POSTs per run, 72 total; discovery GETs excluded. Each run also passed all four direct physical-cancellation tests, corrected pooled cancellation, and mock retry guards. No unexpected failures or count mismatches.

Final independent measurement audit (`hnd_8l`, retained as `independent-measurement-audit.md`) inspected raw outputs/receipts, executable bytes, hashes and inventories, then returned **LOCAL MECHANISM QUALIFIED — no unresolved blockers within the preregistered local scope**. Audit was a separate context, not independent execution or another vendor. Returned text was truncated after its source-inventory section began; the explicit verdict and all raw-result/integrity statements above were visible, and unseen text is not inferred.

**Decision: retain the corrected working candidate as locally mechanism-qualified. No release promotion.** Final `decision.json` is a separate post-audit event; raw `result.json` remains unchanged with its historical pending-audit state. The marker bug was corrected based on independent inspection and a negative network control; there was no separate dynamic build/ablation of the original flawed candidate, and no such attribution is claimed. Baseline versus corrected candidate is the measured treatment comparison.

## Remaining gates

Release/full-suite integration and task-level validation remain distinct future gates; H02 remains a separate experiment. No source/test mutation occurred during v3 measurement; final drift/hash checks passed. The candidate was measured as a frozen working patch; committing those audited bytes does not qualify the older release executable or constitute release promotion. No claim of task-speed, cost reduction or world leadership follows from this local result.

## Portable evidence

Capsule: kept with the private research records, SHA256 `f482ad440c1ae92b3c0b8caadf62fb0be6b3956958993cfff666e2ced5d85854`. The archive includes preregistrations/amendments, both frozen patches, exact runner, all source inventories, raw build/test outputs, receipts/assessments, independent reviews and final decision, with a checksummed member manifest. Large debug binaries and entire source worktrees remain local; their hashes are in image records and the common source base is recorded. This is an evidence capsule, not a self-contained executable distribution. A clean checkout of the common base plus patches reproduces source identities; compiler/dependency availability and hardware remain external requirements. Older confounded/preliminary record copies are included as separate prior-evidence members without modifying their originals.
