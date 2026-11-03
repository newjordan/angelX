# Blaze worker readiness: remove polling, without redefining readiness

2026-10-28 · iteration 20 · bounded Linux x86-64 protocol pilot, not an installed change.

## Concrete upstream PR opportunity

Pinned Blaze `b383b0bcf5da33816809b00a37e1c36a0e761efb`,
`crates/ra-jit/src/cargo/daemon.rs:118,716–730`, retries a checked Unix socket
connection and sleeps **20 ms** after each failed attempt while its worker lives.
A listener published just after a failed attempt can wait almost one polling
interval before the parent notices. This experiment measures that readiness
residual with real child processes; it does not measure compiler throughput.

**New integration constraint:** `main.rs:9525–9580` publishes the listener **before**
`synthetic_write`, memo-capacity setup, warm-up reporting and the request-acceptor
thread. Socket publication is not completion of warm-up, a successful compilation,
or publication of a fresh executable. A PR must name its readiness boundary:

- `ListenerPublished`: the old checked-connect boundary; requests can queue while
  warm-up finishes. The pilot models this boundary only.
- A later `AcceptingRequests` / `Warm` signal is a different behavior and requires
  end-to-end request/reply measurements. Waiting until warm-up to signal could
  lose existing queueing/overlap. Do not infer its latency from this experiment.

## Measured pilot

Preregistered cases: six alternating-order pairs at requested publication delays
0, 1, 3, 7, 11, 17, 21 and 37 ms; **96 measured trials**. Both arms spawn the same
owned child, synchronize HELLO/GO, use the pinned private socket publication and
peer checks, read a semantic generation reply, and reap it. Baseline drains the
same readiness frame after timed connect return; the notification arm waits for
it before making one checked connect. Timer excludes process launch and excludes
request/reply, child wait and cleanup.

| Requested delay | Polling elapsed median | Notification elapsed median | Polling residual median | Notification residual median |
|---:|---:|---:|---:|---:|
| 0 ms | 20.080 ms | 0.055 ms | 20.038 ms | 10.294 µs |
| 1 ms | 20.076 ms | 1.108 ms | 18.980 ms | 10.454 µs |
| 3 ms | 20.074 ms | 3.116 ms | 16.975 ms | 11.241 µs |
| 7 ms | 20.079 ms | 7.115 ms | 12.972 ms | 15.970 µs |
| 11 ms | 20.073 ms | 11.126 ms | 8.973 ms | 17.453 µs |
| 17 ms | 20.082 ms | 17.131 ms | 2.955 ms | 21.866 µs |
| 21 ms | 40.134 ms | 21.150 ms | 19.034 ms | 21.410 µs |
| 37 ms | 40.119 ms | 37.167 ms | 3.026 ms | 28.178 µs |

Pooled residual medians: **14.923565 ms → 0.015349 ms**, a **14.908216 ms / 99.897%**
reduction in this fixture. All preregistered numerical gates passed. Parent
thread CPU medians were **23.610 µs → 16.271 µs**; notification is not busy polling.
Candidate successful trials used one checked connect; polling used two or three.
Each fixture arm had four extra live parent FDs before the timed connection and
returned to its initial FD count. This is *not* evidence of zero production FD
cost: both arms deliberately include the experimental notification/cancel pairs.

A timestamp is taken immediately **after** the bind/publication method returns,
not atomically with the kernel publication. Signed residuals retain this possible
observation race; none were negative in this run. The delay grid and synchronized
phase are controlled inputs, **not a production readiness distribution**. These
are pilot medians, not significance tests, an additive per-request guarantee,
whole-build speedups or full-angelX startup improvements.

## Controls, defects and acceptance conditions

- Functional reduction: **exit 0**, 18 control invocations (12 scenario/method
  controls plus six fresh-worker cycles), including semantic generation replies,
  private modes, exit before readiness, wrong magic/generation, truncated frame,
  EOF, missing listener after valid frame, never-ready, trickle and cancellation.
  Every tested owned child was reaped and FDs returned to baseline.
- Existing upstream socket regressions compiled under the same minimal Linux
  ABI adapter: **10 passed**, exit 0, no project/Blaze build. These protect the
  transport shared by both arms; they do **not** test the new readiness protocol.
- First extraction attempt failed before compiling: textual anyhow adaptation
  did not match indentation. Its source/receipt is retained. The corrected
  second attempt uses a fresh directory. No failed run was overwritten.

Read-only review identified requirements that prevent integration of this sketch:

1. **One frame, many callers.** Upstream workers receive repeated/concurrent
   requests. One owner must consume the frame and publish a sticky generation-
   scoped result to all waiters; requests must not each read the one-shot channel.
   Stop/restart/replacement must invalidate it. Test simultaneous first callers,
   repeated calls, failed generations and spare-worker reuse.
2. **Frame-wait deadline only.** The sketch has an absolute notification-read
   deadline and explicit cancellation, but the following checked connection is
   synchronous. Neither bounds a blocked/full-backlog connection or a scheduler
   pause after a complete frame. Do not promise an operation-wide deadline.
   Require late-ready, cancellation races and bounded-connect controls upstream.
3. **Preserve logs and lifetime wiring.** Actual worker stdout/stderr are log files;
   stdin EOF controls worker lifetime. The reduction repurposes stdout for a
   private pair. A real protocol needs a dedicated inherited channel with correct
   close-on-exec/inheritance and cross-platform rules, not altered stdio or trust
   in log text. Preserve private paths, peer checks and ownership locks.
4. **Preserve terminal diagnostics and recovery.** Candidate EOF/invalid frame/
   connect errors are reduction IO errors, not the baseline worker-log diagnostic.
   It immediately fails a missing listener rather than retrying while alive.
   Preserve errors-beside/log reporting, cancellation, child reaping, pending-plan
   handling, and clear failure behavior for malformed/stale notifications.
5. **Do not signal fresh artifacts.** Listener readiness must not bypass dependency,
   source-image or artifact-generation validation. The known full-host stale-image
   qualification failure is separate and remains unresolved.
6. **Measure actual endpoints.** After native protocol/lifecycle tests pass,
   compare spawn→listener, spawn→accepting, request→reply and edit→fresh-executable
   times, CPU, FD/process footprint and warm-spare reuse. Include warm, cold and
   failure workloads; measure whether warm-up masks the saved polling interval.

## Reproduction and retained evidence

- Preregistration: `reports/performance/blaze-ready-preregister-iteration20.md`.
- Driver/template: `reports/performance/blaze-ready-reduction-iteration20.{py,rs}`.
- Corrected run: `reports/performance/blaze-ready-reduction-r2-iteration20/`:
  `receipt.json`, `verification.json`, `measurement.json`, frozen upstream
  methods/module, minimal adaptation patches and compiled reduction.
- Failed extraction: `reports/performance/blaze-ready-reduction-iteration20/`.
- Existing tests: `reports/performance/blaze-ready-socket-controls-iteration20.py`
  and the correspondingly named directory with `tests.json`/`receipt.json`.

Both drivers use exclusive output creation: the retained names cannot be silently
rerun over evidence. Choose a new output name for a new measurement. They compile
standalone Rust only and never execute Cargo, Blaze or angelX.

**Decision:** retain as a measured upstream PR design lead, not an apply-ready
patch or installed optimization. No commits, pushes, upstream submissions,
application/provider test requests, cache deletion, or installed compiler changes
were made. A separate read-only model review informed the limitations above.
The existing protected-file mismatch still blocks full-host qualification; do not
reseal it. Automatic Blaze selection remains unsafe until the separate freshness
failure and lifecycle qualification are resolved.
