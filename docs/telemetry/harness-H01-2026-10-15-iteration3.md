# H01 iteration 3 — revalidation completed; pooled-cancel assertion limits

## Hypothesis, phase, and decision

Native loop `1792029422798-3790707`, iteration **3/8**, program `20261015-local`.
H01 remains the original permanent-configuration containment hypothesis: typed
endpoint-configuration failures should terminate at one admitted HTTP attempt
without losing the registered transient/cancellation behavior. The baseline and
candidate remain the frozen uncommitted arms from iteration 0, based on commit
`665569ee46208ca82a2d28e10b93f5054594e283`. No new product treatment, H02,
independent audit, or experimental replication occurred here.

**Analysis decision: completed, with the dirty source correctly classified
UNBOUND. Product decision: BLOCKED / NO PROMOTION.** The exhausted two-build
H01 budget is unchanged. Original failed fixtures, analysis failure, reports,
patches, source snapshots, receipts, and decisions remain intact.

This author-performed block repaired iteration 2's analysis-only handling of the
source-digest script, completed source/binary/helper/resource revalidation, and
reviewed previously missed existing HttpClub pooled-replay tests. It does not
establish product acceptance, task efficacy, speed improvement, or leadership.

## 1. New verifiable finding: existing pooled tests narrow the missing evidence

The previous review found the vendored ureq replay path but missed the following
existing tests in **`tests/cockpit/club/http__tests.rs`**. They were read, not run.
Their source is hash-bound in `iteration3-revalidation/source-evidence.json`.

| Existing test | What its source actually asserts |
|---|---|
| `a_provider_closed_idle_connection_is_replaced_without_an_error` (1957–1983) | Linux, HTTP retries=0, FIN/RST variants, two connections and two recorded request strings, successful second response. |
| `a_request_lost_to_an_idle_close_is_resent_once_on_a_fresh_connection` (1985–2013) | Linux, HTTP retries=0, FIN/RST variants, two connections, three recorded request strings, equal lost/replayed bodies, successful second response. |
| `esc_during_the_resend_after_a_dead_kept_connection_ends_the_call` (2015–2066) | Cancellation triggered after the fresh request arrives; client returns within 2 s and does not yield nonempty Text; server read is compared with zero using `unwrap_or(0)`. |
| `esc_before_the_resend_sends_nothing_more` (2068–2114) | Cancellation triggered after the kept request arrives; client returns within 2 s and does not yield nonempty Text; a later nonblocking accept must return an error. |

These tests give an existing project entry point for a future authorized focused
run; another general replay fixture should not be invented before assessing them.
They do **not** resolve the paired H01 measurement gap by their presence alone.

### Concrete limitations missed by the earlier review

1. **A timeout can satisfy the during-replay socket-close assertion.** At line
   2041, `fresh.read(&mut byte).unwrap_or(0)` maps *every* read error to zero,
   including a timeout at the five-second socket read limit. The separate client
   promptness check at 2059 does not exclude the client returning promptly while
   the server socket remains open until that timeout. Thus the assertion is not
   specific evidence of EOF/socket shutdown. This is a logical false-positive
   path in the test oracle, **not an executed demonstration of a product bug**.
2. The early-cancel test at 2092 accepts *any* accept error, rather than requiring
   `WouldBlock`; it observes a bounded 300 ms window, not an unbounded no-replay
   property. Cancellation checks exclude nonempty Text, but do not require a
   particular error/cancellation class or outer `run_turn` terminal outcome.
3. The shared request reader (551–573) returns accumulated text after read
   error/EOF even if the declared body is incomplete. Request-vector length is
   consequently not an unconditional count of **complete POSTs**. The replay test
   also compares bodies, but does not explicitly assert each request's complete
   framing. This does not show that its actual requests are partial.
4. These are direct HttpClub calls (`stream_once`, 1947–1955), not a two-layer
   `run_turn` test with admission accounting/deadline outcomes. They use a 100 ms
   sleep after the seed stream rather than a pool-ready synchronization barrier.
   The production stream tail is drained on a detached thread (`http.rs:59–83`),
   so the sleep is an assumption about readiness, not a deterministic signal.
5. The existing replay test deliberately reads the kept request before closing
   it (`IdleDrop::AfterRequest`, 1929–1933). It is a fixture for *receipt followed
   by no response*, not evidence that the server did not receive/process it.
   The ureq response-replay branch at `unit.rs:334–341` only knows that no response
   arrived. Do not infer non-processing or exactly-once semantics from that.

The relevant implementation still guards each recycled replay with `!aborted()`
and recurses with `use_pooled=false` (`vendor/ureq/src/unit.rs:276–341`). The abort
handle remembers cancellation before attachment and shuts down its current socket
(`abort.rs:24–55`); HttpClub's flag-watcher is joined on guard drop
(`cockpit/src/agent/club/sse.rs:19–41`). These source guards are important counter-
evidence against claiming a demonstrated uncancelled replay. None was changed.

## 2. Corrected analysis, exact command, and result

Prospective scope/budget and candidate freeze were appended to the existing
append-only index before invocation. The analysis helper is saved in a **new**
attempt directory; iteration 2's failed helper and receipt are unchanged.

From the repository root:

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
  python3 -B artifacts/harness-research/20261015-local/H01/iteration3-revalidation/analysis.py
```

The only exit-code exception is at the working-source digest call: exit 3 requires
empty stdout and the precise documented dirty-input diagnostic grammar. It records
`state=unbound, source_sha256=null`; it never substitutes clean HEAD's digest.
Other subprocesses require exit 0. Exit 0 source output requires 64 lowercase hex
digits plus newline and empty stderr. Fixed in-memory cases reject exits 1/2/64,
malformed exit-3 output, and empty/malformed exit-0 output. **9/9 analysis-parser
cases passed**; these are not nine Rust/product regression tests. The saved
historical exit-3 data reproduces the old helper's rejection without rerunning it.

One invocation completed with **exit 0**, **1.092664 s** wrapper wall
(**1.067064 s** analyzer wall), no timeout and empty stderr. The analyzer recorded
**166 passed integrity/lexical checks**; these counts include resource-path checks
and are not independent trials or evidence of product behavior. Thirteen read-only
subcommands and their argv/timestamps/exits/stdout/stderr hashes are in cumulative
`commands-*.json` receipts. There was no Cargo or test-executable invocation.

The logical review and script diff were saved before execution. Exclusive-create
output files preserve all prior attempts. The full tracked diff bytes, HEAD, git
status, reviewed source hashes and preserved preflight identities were unchanged
during analysis. A timeout or unexpected command failure would have retained logs
and stopped without a successful result. No rerun was needed or performed.

## 3. Current identities and resources

| Item | Revalidated result |
|---|---|
| Current HEAD | `10d75eb64a95f670ef87d323234b1e65b6c39378` |
| Current committed source (`angel-source-v3`) | `7cb6f6985f44f97b85fe6fe8e627ca71193e3c8252bb164f0053547dec001dac` |
| Dirty working source | **UNBOUND**; digest command exit 3, no stdout digest |
| Release binary SHA256 | `b41f58c70a21cec95412b9bbc2a926cca46f53de54e811d1e4fcf0eca23bf9d4` |
| Release's embedded source | `5c384d7c1c456c2b00687dc570e19547fcebc8183041c401b8709ef5a27cb9bb`; **does not match current HEAD** |
| Sandbox helper SHA256 | `1bebf47d10e03b4144788f5e78b058914d73222afbf47b4bb9031cad5b0d9996` |
| Candidate test executable SHA256 | `206107ee2177913cb9e2c8d0de942e95638b59921fb1828c8abd9f550b7cc898` (hashed, not executed) |
| Candidate identity manifest SHA256 | `32af54ab5bd1e1d7162ed4504dec1b0e3425b62e9a0b25cc7290fc44a6876832`; all **6** source/test/patch identities matched |
| Index at analysis entry | **97/97** referenced hashes matched; prefix SHA256 `d34ac8b253fa575d09cd6c70e5474d724b3e0cb5857d6b04375bfb20430c7b93` |
| Original snapshot | **1,677** files; native runner JSON digest `8448e2a7e9c608135adc09b9a99843bd8dbccc723f5a46b0e06aedae6282033f` |
| Current resources | **1,677** files; digest `a6515caabdd5ffee7a7a822e8d046bf89034fa4e44a884a6a3e02f92c7124307` |

The original snapshot still matches. Current resource differences are exactly the
three H01 production files (`http.rs`, `retry.rs`, `turn/classify.rs`); no additional
resource drift was found. Resource inventory is the existing runner's inventory,
**not every build input**: do not infer whole-checkout identity from it. Binary and
helper were preserved, not rebuilt or made current-source-qualified. Build-info
resources remain unbound. Host observations are saved, not timing-isolation claims.

## 4. Counts, controls, and limitations

- **0** new builds, **0** product-test runs, **0** physical sends/discovery GETs,
  **0** provider calls, **0** remote jobs, submissions or external publications.
  Recovery/cancellation outcomes this iteration: **not measured**. The source
  assertions' expected counts are not substituted for observed network counts.
- Outer shell still inherited `ANGEL_YOLO=1`; the analyzer and every subcommand
  explicitly used `ANGEL_YOLO=0`, `ANGEL_YOLO_SMART=0`. Source parser review supports
  their off/guarded interpretation, but this is not an emitted headless runtime,
  production acceptance or sandbox-containment qualification. Those are unknown.
- `root_turn_env` continues disabling multiple behavioral gates for H01 fixtures
  (`tests__run_turn.rs:9898–9916`). No safety/control defaults were altered here.
- This was an author analysis, not independent audit. The original baseline
  network fixture confound and missing identical-fixture paired replay persist.
  No untouched holdout or peer comparison was run; H02 stays unregistered.
- Product source, existing tests, binaries, unrelated work and previous evidence
  were preserved. Only the new local analysis/evidence and this report were added.

## 5. Evidence and next action

Append-only index: `artifacts/harness-research/20261015-local/index.jsonl`.
New local bodies: `H01/iteration3-revalidation/`, including scope, logical review,
analysis source/diff, truth table, raw logs, command/invocation receipts, candidate/
preflight/index checks, resource maps/differences, source citations, coverage
assessment and result. These artifacts are git-ignored and are not available in a
fresh clone. The final closeout links their hashes and this report; original
negative results remain retained. No settlement deposit or external publication
was requested.

**Pause at the product-execution/independent-audit gate.** Proposed next step,
not authorized here: preregister a separate finite same-fixture baseline/candidate
replay and independent review, first examining the existing test entry points
above. A future pooled-cancel oracle should distinguish EOF from timeout/error,
validate complete requests, synchronize pool readiness, and record both admissions
and server-observed POSTs with terminal classes. Changing those tests would be a
new candidate requiring prospective budget, review and its smallest existing
project verifier. Do not change transport replay policy merely to reduce counts,
rebuild the release implicitly, or begin H02/live/remote work at this gate.
