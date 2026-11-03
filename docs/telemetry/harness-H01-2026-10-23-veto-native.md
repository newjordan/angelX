# H01 — real-harness veto provenance gate (2026-10-23)

## State

**CLOSED — qualified local mechanism support; NOT PROMOTED.**

Operator requested continuation after the isolated exact-function screen. This
experiment tests the preserved candidate in the actual Cargo-built cockpit test
image, with an independently authored, previously unexecuted string holdout.
No live product source has been edited. No integration, commit, push, release,
provider benchmark, remote job or game work was performed.

## Protocol and identities

Artifacts: `artifacts/harness-research/20261023-local/H01-veto-native/`.
Preregistration: `preregistration.json`. Build shape/filter amendment:
`design-provenance.json`. Exact oracle: `expected-test-set.json`.
Runner: `run-native.py`; evidence checker: `analyze-native.py`.

Base is the frozen tracked archive of
`d0e4ccba11044657f2119f07ad41853dd00d6e29`, not the dirty shared tree.
Parent candidate is the two-file patch retained by the earlier screen.
The source inventory records effective candidate files; the full snapshot and
its path dependencies are retained. This archive's inventory is an experiment
identity, not committed source-v3 native-release qualification. Embedded build
git identity must not substitute for that inventory.

## Hypothesis, controls and ablation

Candidate B should pass actual turn-level loopback recovery for a transient body
quoting `[x-should-retry: false]` while an actual veto response header still stops
retries. Baseline A removes only the production predicate change and retains all
prospective tests. B then A order is preregistered; one compile/test attempt each.
This deterministic experiment makes no timing or task-level performance claim.

Controls cover actual header veto, permanent auth/malformed/configuration errors,
transient 503/429/disconnect, quoted permanent-transport marker, Retry-After,
existing partial-stream/deadline guards, four direct socket cancellation tests,
and two pooled abort-isolation/socket-reuse controls (not pooled resend cancellation). The expected selected set is 13 real tests. A helper named
`h01_controls` is not a test and was excluded from the oracle before execution.

Candidate loopback expects 11 cases and 21 observed fixture POSTs, excluding
model-discovery GETs. Baseline is expected to fail at `outer_veto_body`, with two
POSTs rather than recovery after three; its loopback test then aborts, so its final
Retry-After case will not run. That partial baseline coverage is not equivalent
to a complete paired loopback cohort. Actual measurements below match those expectations. The baseline final Retry-After
case is unexecuted, not a matched success.

## Independent holdout

The first inspection-only independent-design attempt timed out without a verdict.
Its session receipt is retained; it is not a clean review. A second, contract-only
reviewer with no tools returned 15 rendered-string cases without viewing the patch
or previous fixtures. `independent-holdout.rs` preserves them semantically unchanged
(formatting compacted): genuine header-veto controls and transient-body negatives
with Unicode, empty details, repeated colons, embedded labels, LF and CRLF.
These are untouched **string-boundary** holdouts, not hidden benchmark tasks.
Exact model versions, tokens and cost were not exposed and are unknown.

## Budget and attempts

One 40-minute block; maximum two offline Cargo test compilations, jobs=2,
concurrency=1, CPUWeight=20, MemoryMax=12G, ANGEL_YOLO=0,
ANGEL_YOLO_SMART=0. Candidate outer timeout is 30 minutes; total experiment deadline
is recorded in the preregistration. Build failures or unexpected failures stop the
block; no unchanged reruns. Raw Cargo/test exits and outputs are retained.

Setup attempt: `git apply` through the artifact symlink exited 1 because Git
rejects crossing symbolic-link boundaries. No compilation or tests occurred in
that attempt. Copying the already-hashed candidate files into the isolated archive
completed the setup. `setup-attempts.json` records both operations.

The chosen command compiles the complete real test binary with
`cargo test --locked --offline --no-default-features --bin angel --no-run`, then
executes only the registered focused test set from a copied, hashed executable.
It does not build the standalone product/helper release image and is not the full
`check-cockpit-fast.sh --release` qualification path. The test-image compilation
is the development build gate; successful test execution is a separate requirement.

## Measurements

Actual raw receipts: `candidate-receipt.json`, `baseline-receipt.json`, Cargo
stdout JSONL/stderr and both `*-test.stdout.txt` / `*-test.stderr.txt` streams.
Paired driver audit: `results.json`; independent-audit status and coverage limits:
`results-audited.json`. Test images were copied from the exact Cargo JSON artifacts
before the second arm reused the compiler output path.

| Arm | Compile exit | Test exit | Passed / failed | Quoted-body loopback | Actual veto header |
|---|---:|---:|---:|---|---|
| Candidate B | 0 | 0 | 13 / 0 | 3 POSTs, recovered | 1 POST, terminal error |
| Treatment removed A | 0 | 101 | 10 / 3 | 2 POSTs, terminal error | 1 POST, terminal error |

Candidate tests executed 11 loopback cases, 21 observed fixture POSTs, excluding
model-discovery GETs. Baseline executed 10 loopback cases, 18 observed POSTs, then
panicked at `outer_veto_body`; its final Retry-After case is **not executed**. The
candidate Retry-After interval was 1002 ms in its server log. No task-time or
network-throughput benefit is inferred from these observations; recovering a
transient failure intentionally adds the third POST.

Baseline failed exactly the three registered regressions:
`h01_loopback_retry_controls`,
`h01_retry_veto_requires_the_status_label_not_a_quoted_body`, and
`h01_independent_veto_holdout`. The other 10 selected tests passed in both arms,
including four physical direct-cancellation tests, two pooled abort isolation/reuse
controls, two standalone permanent-error tests, H01 endpoint containment, and H01's
six existing mock-stream/context/deadline guards.

Candidate's independent holdout passed all 15 assertions. **Baseline failed its
first assertion; its remaining 14 holdout items were not executed.** Do not report
15 individual paired failures/successes. The original direct regression similarly
stops on its first failure in the baseline. All unsuccessful output is retained.

Both full effective source inventories have 10,354 files and differ only in
`cockpit/src/agent/harness/turn/classify.rs`. Frozen baseline content after both
executions matched its inventory; two packaged candidate files match their
candidate inventory. The independent auditor verified those identities and the
ELF test artifacts, Cargo `profile.test=true`/`success:true`, and raw log hashes.

Identity anchors:

- Candidate test image: `65ffb6e8cb67775265b4a570413b63dc5f074c9602f4c346785c48ac8070af34`
- Baseline test image: `82efb5efe0e8cfd1ca6be8bc5f8d80b76512490d35b9614e475dd36262416eab`
- Candidate receipt: `76a74d3cd4e5e55f7f1f4ad094cf73f19a5c635983cee69671a105fd611246d4`
- Baseline receipt: `69d3b7bee86db60aa0003bae89bada3469f4caa2abeafabe7b4185d2e441a656`
- Final candidate patch with holdout: `5ffe254c9ad28303b7dbc303f4fa70af02f5a5a02acf10b1c71f385f05275e00`

The block stayed within its 21:43:46 UTC deadline. Two top-level Cargo test builds
and two test-image executions occurred, no unchanged reruns. Both jobs exited
(0 candidate, 101 intentional-red baseline); no experimental service remains.

## Independent audit

One independent read-only reviewer returned **Qualified PASS — local mechanism
support substantiated**, with no correctness blocker in the narrow producer-format
change. It read raw evidence, recomputed identities and inspected producer and
consumer source. Delivered response: `independent-audit-delivered.txt`; provenance:
`independent-audit-receipt.json`. The returned answer was truncated in its final
identity table; substantive verdict/findings were received, but omitted tail text
is unknown, not an implied blanket authorization. This supports only the reported
local mechanism claim. It is not release, task-effect, or leadership approval.

The holdout author and auditor were separate spawned seats. Requested club=self,
reported route label=openai. Exact model, effort, token usage, and cost were not
exposed and remain unknown; no zero-total-model-spend claim is made.

## Deviations and evidence limitations

- Initial `git apply` into the artifact symlink path failed before compilation.
  Exact hashed candidate files were copied into the isolated snapshot instead;
  setup attempts are retained in `setup-attempts.json`.
- Existing build.rs invokes nested Cargo for an embedded browser/wasm asset.
  This was a required build dependency, not resumed game work. It clears the
  environment, so **the parent's offline/jobs settings do not establish those
  controls in nested builds**. CPUWeight/MemoryMax and the outer timeout covered
  descendants. One top-level arm ran at a time, but two top-level invocations is
  not a claim of two total Cargo processes. Package-network absence and exact
  nested compiler concurrency were not measured. See `build-control-limitations.json`.
- One checker parser initially named historical invalid endpoints rather than the
  two actual frozen endpoints. It was corrected to `not-a-url` and
  `ftp://127.0.0.1:1`, one admitted attempt each, without changing source, tests,
  outcomes or rerunning either arm. See `analysis-amendments.json`.
- Development test images are real full-crate compilations, not standalone product
  binary/helper/resource release qualification. No provider route/model task was
  exercised, no live TLS handshake measured, and pooled resend cancellation was
  not selected. Build timing is diagnostic only. No untouched task cohort, matched
  peer, cost/efficiency or solve-rate result exists.

## Decision and remaining gates

**NOT PROMOTED or integrated.** The source-level mechanism now has repeat support
across the prior exact-function screen and this actual-harness gate, with an
independent holdout author and independent qualified evidence audit. Preserve the
tested patch and images; do not repeat these unchanged gates as busywork.

The next decision is a separately scoped integration/native-qualification gate
for this two-file patch, with operator-approved integration scope and committed
source/executable/helper/resource identity where required. Full release qualification,
independent confirmation across representative workloads, untouched task holdouts
and matched peers remain prerequisites for broader product/leadership claims.
Historical row425 report hash mismatch remains unresolved and is not repaired by
these receipts. No claim is made that this candidate is running in the operator's
cockpit; the live classifier remains baseline.
