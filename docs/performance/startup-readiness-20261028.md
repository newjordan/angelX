# Startup readiness without stale-build shortcuts — 2026-10-28

## New implementation

The full-feature cockpit now uses readiness instead of a fixed 10 ms sleep while
waiting for its startup bubblewrap compatibility trial. On Linux,
`compatibility.rs::wait_for_probe_exit` opens a pidfd and waits with `poll` until
the child exits or the remaining deadline expires. `Child::try_wait` remains the
only source of exit status; the existing owned-child registration, synchronous
startup ordering, two-second bound, kill/reap, classification, and confinement
policy are unchanged. An unsupported syscall/poll or interrupted wait uses the
old bounded sleep fallback. No new dependency, helper daemon, persistent cache,
or feature-reduced image was added. Descriptor ownership is RAII-managed.

`main.rs` also dispatches validated `--help`/`--version` before process ownership,
run-identity hashing prewarm, helper discovery, and compatibility probing. Leading
global validation and trailing-argument rejection still occur first. Ordinary
startup still initializes all of those systems; doctor still probes normally.
Cargo still checks freshness before a source-launcher inspection command.

## Actual measurements

Three warm 140x40 real-PTY observations per variant, same existing
`scripts/perf/measure-terminal-ready.py`, isolated runtime state, practice route,
no model calls. Media/browser/Glass remain compiled in; runtime Glass is disabled
for terminal measurement. Discovery/background activity is disabled equally.
These are small local samples, **not** cold setup, GPU-window first frame, or a
universal fastest-possible claim.

| Median latency | Before | Final candidate | Change |
|---|---:|---:|---:|
| Direct image first terminal output | 36.947 ms | 28.536 ms | -8.411 ms / -22.8% |
| Initial draft visible | 54.882 ms | 46.081 ms | -8.801 ms / -16.0% |
| Input echo | 0.597 ms | 0.622 ms | +0.025 ms; no improvement claimed |

Raw reports including executed image SHA-256 and all samples:
- `reports/performance/startup-prewarm-baseline-20261028.json`
- `reports/performance/startup-pidfd-candidate-20261028.json`

The baseline filename predates the final design; it is the original eager
polling image, not an asynchronous implementation. Candidate reports for the
rejected lazy proposal remain named `startup-lazy-*`; do not use them as the
shipped result. Final inspection-only medians over seven direct-image samples
were version **28.246 ms**, help **28.931 ms**, doctor **28.459 ms**, all exit zero;
raw receipt `startup-pidfd-inspection-20261028.json`. Earlier baseline exploratory
five-sample medians were 37.619 / 36.546 / 38.099 ms respectively (terminal output
only, not a persisted JSON receipt); do not treat these as paired statistics.

### Full source launcher, real edit and restoration

`scripts/perf/measure-dev-rebuild.py` with normal Cargo backend and all usual
features retained, no skip-web/no-build override:

| Operation | Actual wall time |
|---|---:|
| Prime | 397.4 ms |
| No-touch launch 1 | 250.0 ms |
| No-touch launch 2 | 245.0 ms |
| No-touch launch 3 | 254.0 ms |
| Semantic Rust edit → execute updated build-info | 16.175 s |
| Restore Rust source → execute restored build-info | 15.546 s |

Receipt `reports/performance/startup-pidfd-devloop-20261028.json` records actual
output with the marker present after editing and absent after restoration,
plus resource availability, video decoding true, native ABIs `[1,1,1,1]`, and
cartridges. `semantic_edit_verified`, `source_restored`, and
`restored_image_verified` are all true. The test does not rewrite no-touch
controls. This is freshness evidence, **not** proof of a Rust compile speedup:
these edit times are within the prior 13.9–20.3 second range. Cargo/launcher
cost still dominates warm source launch; the 28.5 ms direct-image number must
not be advertised as source-launcher latency. No Cargo check was bypassed.

## Verification and rejected approach

- Actual normal full-feature `bin/angelX build`: exit zero, 15.66 s Cargo build;
  `reports/performance/startup-pidfd-build-20261028.log`.
- Existing `interactive_terminal_smoke` lifecycle/focus test, strict PTY mode:
  **1 passed**, 0 failed; `startup-pidfd-pty-regression-20261028.log`.
- Existing compatibility classification plus three targeted wait regressions in
  the `process_ownership` integration target: **4 passed**, 0 failed;
  `startup-pidfd-owned-wait-tests-20261028.log`. Actual owned children are used.
  Tests check real exit 7, bounded wait on still-running child, and invalid PID
  fallback. The first integration wiring failed compilation due to missing
  parent `available()`; corrected with the same production Landlock probe.
  Both failed and corrected logs are retained, not relabeled as successes.
- Independent static review: no blocking regression found for final implementation;
  `reports/performance/startup-readiness-review-20261028.txt`.

A lazy detector proposal measured well but was rejected after independent
review: it moved the wait to first tool construction before cancellation/budget
handling and exposed the existing spawn/shutdown boundary from a tool worker.
Keeping eager startup and improving its actual wait avoids that tradeoff.
No asynchronous compatibility probe thread remains. Signal-interrupted poll
fallback and old-kernel fallback in a real restricted host are not separately
qualified; the invalid-PID fallback unit test does not claim that coverage.
The pre-existing timeout policy and potentially blocking stderr EOF/kill wait
are unchanged. macOS/other platforms retain polling and were not measured.

## Next work

The remaining warm source launch (~250 ms) is dominated by Cargo freshness and
launcher setup rather than terminal rendering. Keep exact source correctness.
The existing full-feature Blaze opt-in is still experimental: previously observed
stale-publication and build-script sandbox blockers have not been repaired by
this startup change. A new source-localized upstream readiness opportunity is
recorded in `blaze-upstream-opportunities-20261028.md` §5; it is unmeasured there,
not a claim of Blaze acceleration. Do not promote Blaze to the normal backend
until actual full-host edit/restore/failure qualification passes.
