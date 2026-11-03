# V8 compilation boundary: subsecond focused edits, honest host-launch limits

Date: 2026-10-28 · iteration 12 · working-tree implementation installed.

## Change

`cockpit/src/agent/code_mode.rs` used to contain V8 handle/generic machinery,
JavaScript wrapper generation, callbacks, isolate lifecycle, heap guard and
watchdog alongside host policy and scheduling. It is now a small host facade;
V8-only implementation lives in `cockpit/code-mode/src/lib.rs` as the
`angel-code-mode` path dependency. Cargo gives it its own object and fingerprint.

- Host callers keep the same `run` signature and `Outcome` fields.
- Policy, tool authority and effect gating remain in the cockpit. V8 gets no
  filesystem, network or dispatch authority from this extraction.
- Host batch scheduling, malformed-schedule serial fallback, concurrency,
  declaration-order results and scoped worker identity propagation remain in the
  facade. The runtime calls a borrowed synchronous batch callback.
- The construction limit is a callback evaluated **before every batch**, not a
  value snapshotted at run start. Disabling an allowance during an active run is
  reflected in the next batch.
- V8 heap termination, watchdog cleanup, console limits, error handling,
  microtask handling and `External` lifetime remain within the same synchronous
  call. Isolate lifetime still cannot outlive the borrowed callbacks.
- V8 stays linked, with the same version. No feature was removed to look faster.
  Release and working optimization settings were not changed.

## The new fast long-term development loop

```sh
# V8 wrapper/bridge only: no cockpit test-harness relink.
cargo test --manifest-path cockpit/Cargo.toml --locked --offline \
  --profile fast -p angel-code-mode --lib

# Host scheduler and tool-policy integration:
env -u ANGEL_YOLO -u ANGEL_YOLO_SMART -u ANGEL_CODE_MODE_EFFECTS \
  cargo test --manifest-path cockpit/Cargo.toml --locked --offline \
  --profile fast -p angelX-cockpit code_mode -- --test-threads=1

# Normal current-source launch still checks Cargo, not ANGEL_NO_BUILD.
./bin/angelX --version
```

The standard `scripts/check/check-cockpit-fast.sh` gate now explicitly runs the
independent library tests after the pinned cockpit test image, and again in its
release-qualification path. Cargo does not automatically run dependency unit
tests with `--bin angel`; this preserves the moved coverage. Filters are forwarded
unchanged. A library failure fails the gate and cleanup still removes only its
owned image.

## Focused edit/test/restore measurements

The reusable probe `scripts/perf/measure-code-mode-boundary.py` reads explicit
immutable baseline sources from `reports/performance/v8-boundary-iteration12/`.
It applies a **semantic implementation edit** to the V8 compile-error text,
executes a test asserting the new text, restores it and tests again. It records
commands, hashes, logs and checks that exactly one test ran. The baseline is the
original full cockpit test harness; the candidate runs the independent runtime
library. It never obtains a speedup by skipping the assertion or merely returning
Cargo exit 0.

Initial two rounds, with order reversed in round two:

| Layout | Edit seconds | Restore seconds |
|---|---:|---:|
| original, round 0 | 84.885 | 21.626 |
| independent runtime, round 0 | 0.569 | 0.549 |
| independent runtime, round 1 | 0.545 | 0.538 |
| original, round 1 | 23.389 | 25.431 |

All 12 prime/edit/restore samples verified. The first original edit was a large
outlier and is retained, not silently discarded. Combined edit/restore medians
were **24.410 s vs 0.547 s**. This measures the developer's focused unit-test
workflow, including avoiding the large test-harness link. It is not a fair
comparison of equivalent all-cockpit test-suite sizes or whole-product startup.

After the review fixes, another full baseline/candidate pair verified:

| Layout | Edit seconds | Restore seconds |
|---|---:|---:|
| original | 25.613 | 22.364 |
| final independent runtime | 0.538 | 0.559 |

That final pair's two-sample medians are 23.988 s and 0.548 s, approximately
**43.7× faster for this narrow edit/test loop**. Source restoration was checked.

Raw receipts:

- `reports/performance/v8-boundary-targeted-iteration12.json` (12 samples).
- `reports/performance/v8-boundary-final-targeted-iteration12.json` (6 samples).
- Adjacent logs; `v8-boundary-summary-iteration12.json` summarizes the first run.

## Full current-source launcher pilot

The pre-existing `measure-dev-rebuild.py --backend cargo --case build-info`
changes an actual host `resources` result, executes `--build-info --json`, then
restores and executes it again. Video and browser builds remain enabled; the
window is disabled for the headless CLI measurement in every arm. This is warm
CLI/source-launch timing, **not terminal readiness or cold setup**.

| Measure | Before extraction | First candidate | Final reviewed source |
|---|---:|---:|---:|
| 3 warm launches, median | 268.5 ms | 219.7 ms | 223.9 ms |
| semantic host edit | 14.110 s | 14.594 s | 11.966 s |
| restore | 13.572 s | 12.251 s | 11.869 s |

The first candidate did not improve the host edit, and its initial compilation
cost **51.931 s** (new crate, optimized object and new host dependency graph).
The final-source prime took 10.250 s using existing dependencies. Those costs
must not be hidden behind warm-launch results.

The final sample is encouraging, but one edit/restore per arm on a shared host
is **not a statistically established host speedup or record startup claim**.
The demonstrated large gain is the independent V8 unit-test loop. No cold-cache,
clean-target, GUI-first-frame, provider/setup or cross-platform record is claimed.
Receipts: `v8-boundary-{baseline,candidate,final-source}-iteration12.json` under
`reports/performance/`, with adjacent logs.

## Verification and independent review

Final relevant pre-existing Cargo entry point (normal policy, single test thread):

```sh
env -u ANGEL_YOLO -u ANGEL_YOLO_SMART -u ANGEL_CODE_MODE_EFFECTS \
  cargo test --manifest-path cockpit/Cargo.toml --profile fast --offline --locked \
  -p angelX-cockpit -p angel-code-mode code_mode -- --test-threads=1
```

- **20 runtime tests passed**, including syntax, returns, host errors, console
  bounds, async completion, heap termination, timeout, identifier filtering,
  construction limit and policy revocation between two batches.
- **43 host/integration tests passed**, including barriers, order, concurrency,
  identity propagation, effect gating, nested budgets and graph-tool behavior.
- **6 standard-gate script tests passed**, including the new independent-library
  failure propagation/cleanup check. Existing gate fixture checks still ran.
- Actual source launches executed the edited and restored output successfully.
- The reviewer identified stale-per-run cap sampling and lost standard-gate test
  coverage; both were fixed. Callback lifetimes, identity and Cargo inventory
  were otherwise judged sound. That was a static review, not independent timing.

The initial Cargo run inherited operator YOLO/effect overrides and failed six
policy-assumption tests (54 passed). Its receipt remains. Clearing those overrides
for the test process yielded green results; no product policy was weakened.
Existing unrelated compiler warnings and the browser wasm-bindgen availability
warning remain. No full-suite/release/platform qualification is claimed.

## Probe concurrency/cancellation incident and correction

A redundant final timing retry was briefly started while a launcher probe was
still active; it temporarily selected baseline sources. Both jobs were stopped
and their incomplete receipts are retained as **invalid timing evidence**:
`v8-boundary-reviewed-{targeted,candidate}-iteration12.json`. They are excluded
from all tables. SIGTERM originally bypassed the Python probe's `finally`.
The six production candidate files were reconstructed from the immutable snapshot
and the measured candidate hashes, and all six matched exactly before continuing.

The targeted probe now turns SIGTERM into an exception, kills its separately
owned compiler process group, and executes guarded restoration. A deterministic
2-second timeout control then verified all six source hashes unchanged, no restore
conflicts, `sources_restored=true`, and timeout exit 124. Receipt:
`v8-boundary-cancel-restoration-iteration12.json`. Subsequent measurements were
serial. Do not run these source-swapping probes beside editors or other builds.

## Blaze outcome and next work

The installed Blaze callback dependency fixture passed prime, unchanged,
implementation edit and restore with executed-output checks. Its edit was
5.682 s and restoration 0.194 s. This qualifies the reduced borrowed-callback
shape only; full V8 and full cockpit Blaze remain unqualified. The default
Cargo backend and opt-in rollback remain unchanged. See
[callback evidence and two concrete upstream PR opportunities](blaze-callback-boundary-20261028.md).

Next useful work is a repeated interleaved host-edit test with compiler-phase
receipts, then another genuinely expensive leaf boundary only if those receipts
justify it. Do not substitute another global cache bypass or feature omission
for current-source execution.
