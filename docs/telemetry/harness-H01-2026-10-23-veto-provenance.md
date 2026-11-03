# H01 — retry-veto provenance screen (2026-10-23)

## Decision

**Hypothesis supported on the registered classifier fixtures; NOT PROMOTED.**
A frozen-HEAD baseline reproduced an error-content false positive. The isolated
candidate removed it on the same fixtures without losing the genuine-veto controls.
No live cockpit code was changed, no branch was integrated, and no game work resumed.
This is neither full product qualification nor a leadership, solve-rate or speed result.

## Question and hypothesis

Can a transient provider response body containing `[x-should-retry: false]`
incorrectly stop the turn-level retry policy? H01's earlier replay covered a quoted
permanent-transport marker, but not this separate veto marker.

Hypothesis: `is_permanent_provider_error` trusts the veto marker anywhere in the
error string, whereas HttpClub's actual header veto is in the locally generated
`HTTP <status> <veto>: <detail>` envelope. Restricting this single condition to that
envelope should preserve true vetoes and remove body-content false positives.

## Preregistration and identities

- Operator selected the controlled harness program in this session. Native research
  worker status was idle. One local experiment, concurrency one, no live benchmark
  calls or remote jobs. This was not an unattended `/loop` or `/goal` run.
- Baseline: `d0e4ccba11044657f2119f07ad41853dd00d6e29` on
  `durability/backup-peer-watch`. The active workspace was already dirty; selected
  retry source files matched HEAD. The tracked HEAD archive excluded unrelated
  uncommitted work. Current HEAD is an exploratory control, **not** a newly qualified
  release baseline.
- Protocol: `artifacts/harness-research/20261023-local/H01-veto-provenance/preregistration.json`.
  Exact-source-screen registration: `screen-registration.json` in that directory.
  Both precede the baseline execution. The screen registration resolved the original
  registration's Cargo-or-screen branch before running either arm.
- Full tracked archive inventory: `baseline-inventory.json`; effective candidate
  inventory (same base plus exactly two edited files): `candidate-inventory.json`.
  Archive/toolchain receipt: `snapshot-receipt.json`.
- Candidate is an isolated, uncommitted patch, **not a candidate commit**. The custom
  inventory binds these experiment bytes; it is not source-v3 release qualification.
- Cap: two compile/test invocations, one per arm, CPUWeight=20, MemoryMax=12G,
  ANGEL_YOLO=0, ANGEL_YOLO_SMART=0, stop by 20:27 UTC. The actual driver block began
  about 20:05 UTC; the initial stated 20-minute wall ceiling is earlier (about 20:25)
  than the written 20:27 deadline. Execution and closeout finished before both.
- Host: the workstation (Ryzen 9 9950X3D, RTX 3090). Initial load was 24.64/23.30/28.08. No timing comparison is made.

## Design, controls and ablation

A = exact production classifier and pure helper functions from frozen HEAD.
B = same helpers and fixtures, with the single veto recognition condition narrowed.
A is the treatment-removal ablation. Same Rust 1.95.0, edition 2024, opt-level=0,
sequential baseline-then-candidate order; one compilation and one execution each.
No randomized/interleaved replications: this is a deterministic screening check,
not a confirmatory experiment. The analysis script checks verbatim extraction,
fixture/helper/runner identities, exact test sets, raw exits, logs and executable hashes.

The four tests cover:

1. Genuine header vetoes: eight transient status codes × three detail strings.
2. Quoted body markers: eight transient codes × three body shapes (plain text,
   JSON message, embedded canonical-looking error).
3. Retry admission with no emitted answer, recoverable partial stream, and committed
   answer (the latter must not replay).
4. Existing auth/quota/TLS/permanent-configuration and transient controls.

The candidate also stages a direct unit test and a new `outer_veto_body` case in
`h01_loopback_retry_controls`, expecting three POSTs/two failures before recovery.
**Neither staged full-harness test was run. Expected POST counts are not measurements.**

## Results

See `results.json`, the paired `*-receipt.json`, `*-test.stdout.txt` and
`*-test.stderr.txt` beside the protocol. All failed baseline output is retained.

| Arm | Compile exit | Test exit | Tests passed / failed | Quoted-body false permanents | Genuine-veto cases retained |
|---|---:|---:|---:|---:|---:|
| Frozen HEAD | 0 | 101 | 2 / 2 | 24 / 24 | 24 / 24 |
| Isolated candidate | 0 | 0 | 4 / 0 | 0 / 24 | 24 / 24 |

Baseline failed exactly `quoted_body_marker_is_not_a_header_veto` and
`retry_decision_preserves_partial_stream_rules`; existing guards and genuine header
veto controls passed. Candidate passed all four tests with identical fixture bytes.

`analyze.py` returned **PASS** for the expected RED/GREEN pairing and receipt integrity.
This is a same-driver evidence audit, **not independent audit**. Patch applicability
check returned exit 0. There were no reruns, Cargo builds, live benchmark requests,
remote jobs, GPU jobs, or physical-network measurements in this screen.

## Review and limits

One `code_review` call reported route label `openai`, clean with no required fixes;
its supplied-diff review is saved in `static-review.txt`. Exact model identity,
effort, tokens and cost were not exposed and remain **unknown**, not zero. This
review call is distinct from the zero benchmark-provider-call budget; there was
model inference, so no claim of zero total model spend is made. This is not a
promotion-grade independently executed evidence audit. Review correctly noted that
the status-envelope producer was outside its supplied diff. The driver inspected
HttpClub's producer, but only a real integration run can validate its full contract.

Limits: pure extracted-function screening does not exercise full `run_turn`, actual
provider error redaction/wrapping, real header veto production, physical resend or
cancellation, complete Cargo integration, outer-loop behavior, or release artifacts.
The staged loopback test is unexecuted. TLS/auth behavior is only guarded on the
registered string fixtures. No untouched holdout, matched peers, task solve-rate,
performance uncertainty, model usage efficacy, cost benefit, or leadership evidence
was collected. Historical row425 report-hash discrepancy remains unresolved.

## Next experiment and stop

The two-arm local screen is closed at budget. Keep the candidate isolated. Before
any integration, preregister and execute the real focused Cargo build/test gate,
including the staged loopback case and genuine header controls on the same candidate,
plus independently authored malformed/wrapped-envelope holdouts and an independent
evidence audit. Confirm final executable/helper/source/resource identity if seeking
native qualification. Do not widen the policy to outer loops or repeat unchanged
closeout probes. No promotion, commit, push, deployment, or live service was performed.

Artifacts: `artifacts/harness-research/20261023-local/H01-veto-provenance/`.
Program ledger: `artifacts/harness-research/20261023-local/index.jsonl`.
