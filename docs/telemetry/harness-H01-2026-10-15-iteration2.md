# H01 iteration 2 — retry provenance and an evidence-boundary finding

## Hypothesis, phase, and decision

Native loop `1792029422798-3790707`, iteration **2/8**; program `20261015-local`.
This is an **author-performed, exploratory post-decision source review**, not an
independent audit, new treatment, completed experimental cell, or H02 preregistration.
H01's original hypothesis remains: typed permanent endpoint-configuration failures
should stop after one admitted HTTP attempt without losing registered recovery and
cancellation behavior. The original preregistration, two builds, failing regression,
candidate, and prior decisions remain unchanged.

**Decision: BLOCKED / NO PROMOTION.** No new product build or product test ran.
The one analysis invocation failed; do not interpret its partial checks as a green
verifier. H02 remains unregistered. No leadership, production qualification, causal
performance, or broad reliability claim follows from this block.

## What is new

### 1. A plausible malformed-redirect objection was refuted for current HttpClub

The exploratory question was whether `ureq::ErrorKind::InvalidUrl` necessarily means
bad endpoint configuration. Generic vendored ureq can indeed construct that kind
while parsing a received redirect's `Location` (`vendor/ureq/src/unit.rs:185–189`).
But **HttpClub explicitly builds its agent with `.redirects(0)`**
(`cockpit/src/agent/club/http.rs:1580–1584`). ureq breaks out before reading or
parsing Location when that configuration is zero (`unit.rs:164–185`).

Therefore this particular path does **not** establish a regression in the current
candidate. This is a manual source-level refutation, not dynamic execution of a
malformed redirect and not evidence about arbitrary other ureq clients. Retain the
negative lead rather than broadening the classifier on an unsupported objection.

### 2. Admitted attempts do not capture the complete pooled transport replay path

The implementation, rather than diagnostic wording, establishes the distinction:

| Surface | State transition in current source |
|---|---|
| HttpClub | `send_with_retry_origin` creates one `AccountingAttempt` before `req.send_bytes(bytes)` (`http.rs:3027–3055`). |
| Payload | `send_bytes` dispatches `Payload::Bytes(data)` (`vendor/ureq/src/request.rs:232`); `Payload::Bytes` becomes a replayable `SizedReader` (`body.rs:74–79,106`). |
| Pool eligibility | `connect_socket` can return a recycled stream; abortable pooling is enabled on Linux (`unit.rs:392–405`, `stream.rs:ABORTABLE_POOLING`). A probe removes already-detected closed connections, so a crossing close is needed to exercise replay. |
| Prelude failure | A recycled socket can recurse on a fresh socket when the abort flag is not set (`unit.rs:276–291`). |
| Body failure | A recycled, replayable payload can recurse after a connection-loss error, guarded by `!aborted()` (`unit.rs:296–308`). |
| Response failure | A separate match arm replays bytes on recycled connection loss when `replay.is_some()` and `!aborted()`; this arm is distinct from the preceding idempotent-method guard (`unit.rs:327–343`). |
| Bound and cleanup | Each recursive call passes `use_pooled=false`, so that nested call cannot recurse through another recycled-stream branch. Accounting is finalized outside this nested send in `AccountingAttempt::drop` (`accounting.rs:395–408`). |

Thus a single **admitted** HttpClub call has an internal path that can write a
request again before returning to its accounting/retry loop. Whether a peer received
or processed the first request cannot be inferred simply from an absent response or
socket error. This is a **source-supported measurement/coverage gap**, not a measured
duplicate, a proven bound violation, or a candidate-specific regression. The H01
patch does not alter these vendored branches.

H01's server reads complete requests, increments its independent POST counter, and
responds with `Connection: close` (`tests/cockpit/harness/tests__run_turn.rs:11252–11295`).
It does not deliberately seed a reusable socket or force the crossing-close branch.
The existing H01 mock cancellation guards do not measure physical sends through
this nested pooled path. Existing ureq `connection_reuse`/`connection_reuse_with_408`
tests describe idle-pool assumptions, not paired H01 POST/cancellation measurements;
they were inspected, **not run**.

The new limitation narrows what H01's pilot can support. It does not invalidate its
recorded connection-close POST counts, nor show that the URL-configuration change
causes any different behavior here.

## Bounded attempt and its failure

Scope and budget were recorded before execution in
`artifacts/harness-research/20261015-local/H01/iteration2-provenance/scope.md` and
the append-only index. Budget: **one** no-network Python source/evidence analysis,
60 seconds; subprocess ceilings 30 seconds; zero Cargo builds, product tests, model
children, provider calls, remote work, submissions, external publication, or expanded
resources. A logical state/cleanup review was saved before invocation.

Command (repository root):

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
  python3 -B artifacts/harness-research/20261015-local/H01/iteration2-provenance/analysis.py
```

The saved invocation receipt records exact argv, working directory, allowlisted
inherited controls, explicit overrides, timestamps, exit, wall duration, and log
hashes. The analysis exited **1** after approximately **0.050 seconds**. Its third
subcommand, `bash scripts/check/cockpit-source-digest.sh`, returned **3**, documented
by that script as dirty build inputs/unbound. The analysis helper incorrectly
rejected every nonzero exit, including this anticipated state, instead of classifying
it as unbound. This is an **analysis error**, not an infrastructure excuse or a Rust
candidate failure. No retry, overwrite, or retrospective budget amendment was made.

Before stopping, the analysis saved matching checks for all preexisting indexed
artifacts and all six candidate source/test/patch identity entries. It recorded HEAD
`10d75eb64a95f670ef87d323234b1e65b6c39378` and current git status. It did **not** reach
its release/helper/resource revalidation, host observations, static-anchor checks,
source-evidence output, after-status comparison, or intended result output. That
planned `result.json` was not produced. The separate `closeout.json` describes the
actual partial outcome; do not substitute the unexecuted script's intended fields
for evidence. Binary/helper/resource conclusions in iteration 1 are historical
receipts, not newly established in iteration 2.

## Measurements, controls, and preserved evidence

- New experiment physical sends/provider calls: **0** (analysis did not open sockets).
  Discovery GETs: **0**. No recovery/cancellation experiment was executed this iteration.
- Pooled-path physical POST count, recovery, and cancellation outcome: **not measured**.
- New Cargo builds: **0**; new product test executions: **0**. The previous two-build
  H01 budget remains consumed. Product-verifier status this iteration: **NOT RUN**.
- Inherited outer shell still reports `ANGEL_YOLO=1`; the analysis and launched
  children explicitly set `ANGEL_YOLO=0` and `ANGEL_YOLO_SMART=0`. These flags are not
  proof of effective production acceptance/sandbox controls. No behavioral safety
  gate or production configuration was changed by this iteration.
- No product source was edited; no game/browser work, model consultation, credential
  change, release replacement, git commit, promotion, new service, or submission.
- The append-only index is
  `artifacts/harness-research/20261015-local/index.jsonl`. Earlier reports, source
  snapshots, patches, logs, and receipts are retained unchanged. New raw evidence
  and the failed helper are under `H01/iteration2-provenance/`; local bodies are
  git-ignored and are not available in a fresh clone.

`source-provenance.json` records exact line ranges, file hashes, and source excerpts
for the manual trace. It is a citation bundle, not a substitute verifier. The
closeout inventory hashes every retained new attempt file and this report; no
secrets or provider-response bodies are included.

## Next action and authorization gate

Pause before another build, product execution, or experiment. The original paired
baseline/candidate network comparison and independent audit remain outstanding.
If authorized, first repair the analysis helper in a **new immutable attempt** to
recognize only the documented digest exit 3 as unbound, preserving this failed
attempt. A proposed finite authorization is one corrected local analysis <=60 s,
and (separately) at most two matched offline/debug Cargo builds at two jobs and
<=20 minutes each, with one execution per arm, using identical preregistered
fixtures. This is a **proposal, not granted budget**. Existing fixtures' safety/
acceptance limitations must remain explicit; no live route is authorized.

The pooled replay lead needs its own prospective test design before execution:
deterministically seed and prove actual socket reuse, trigger the crossing close,
count server-observed complete POSTs separately from admissions, and coordinate
cancellation without a timing lottery. Do not assume a socket exception proves
non-processing, and do not change replay policy merely to obtain fewer counts.
No pooled-path experiment, H02 treatment, or expanded budget is registered here.
Independent replay/audit and later untouched holdouts/matched peers are still
required for stronger claims.
