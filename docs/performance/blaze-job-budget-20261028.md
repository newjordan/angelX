# Blaze upstream lead: one budget for compile-time work

2026-10-28 · iteration 14 · source-grounded proposal, **not a reproduced Blaze
scheduling defect and not an upstream publication**.

## New local evidence

The standalone angelX native reload publisher previously compiled four independent
C libraries serially. The final candidate limits independent compiler work to four
available slots, stages output until every compiler completes, and joins all workers
on failure. Seven alternating real-source pairs measured **908.6 → 348.7 ms** median
including publisher startup. Actual libraries were byte-identical across serial and
parallel builds and all four ABI functions executed. Unchanged reuse remained about
22 ms after moving pool imports off the cache-hit path. Detailed controls and limits:
[native reload report](native-reload-parallel-20261028.md).

This is evidence that bounded independent work can pay off in this workload; it
**does not** imply Blaze should add workers or run unrestricted parallel scripts.
The local regression specifically requires serial behavior when inheriting a parent
Cargo/Make jobserver. Otherwise a compiler nested under a build script could create
four children for every one parent slot and erase the benefit under real contention.

## Pinned source observations

Unmodified pin: `indexable-inc/blaze`
`b383b0bcf5da33816809b00a37e1c36a0e761efb`.
All following paths are relative to its source checkout.

- `crates/ra-jit/src/cargo/compile_time.rs:260–273`: `Driver` initializes `jobs`
  from `std::thread::available_parallelism()` and stores it as `u32`.
- Same file `:394–397`: compile-time task worker count uses a separate
  `available_parallelism().div_ceil(4).max(4)`, capped only by task count at spawn.
  The scoped workers join before the scheduler reports the first error (`:435–438`).
- Same file `:796`: one compiler configuration has `jobserver: None`. This is an
  observation at that configuration site, **not** proof that the entire daemon lacks
  jobserver integration.
- `crates/ra-jit/src/cargo/closure/kept.rs:339–350`: closure layout derives a separate
  CPU count for batched content fetches. I/O concurrency is not necessarily equivalent
  to compiler CPU slots and should not automatically share the same numerical cap.

These independent decisions motivate a budget-audit PR. Source-level inspection has
not established behavior under `-j1`, an inherited jobserver, cgroups, or nested Cargo.
No timing/correctness claim about those Blaze cases is made.

## PR-sized deliverable and acceptance matrix

1. Add isolated local fixtures with instrumented script/compiler children, a fixed
   tiny dependency DAG and deterministic start/end synchronization records. Measure
   actual simultaneous children, not only elapsed time or requested `NUM_JOBS`.
2. Compare explicit jobs=1 and jobs=2, a parent jobserver with one usable slot,
   nested scripts, restricted CPU availability, and concurrent independent requests.
   Record wait reasons (dependencies/tokens/work), physical starts and completions.
3. Audit whether task scheduling, script `NUM_JOBS`, compiler jobs, and nested Cargo
   consume one coherent budget. Separate CPU permits from I/O request fan-out. Borrow
   tokens or conservatively stay serial under a parent; never assume each layer owns
   every CPU. Avoid holding a token while waiting for a nested child to acquire it.
4. Inject an early child error while another child remains live. No early success,
   dangling compiler, or premature temporary-output removal. Join/cancel behavior must
   be explicit and preserve the old published generation.
5. Execute emitted results after original, edit, and restore. Include real hashes of
   input/output and the backend/fallback provenance. Correct output and bounded workers
   are gates; speed alone is not acceptance.
6. Repeat warmed bounded pairs only after correctness passes. A no-op/reuse case must
   not instantiate an idle worker pool. Retain slower or inconclusive results.

Potential benefit: predictable responsiveness for long-term dev on shared machines,
not just maximum throughput for one uncapped build. Do not claim that the displayed
four-worker minimum is a confirmed bug until the instrumented fixture observes an
actual violated effective budget.

## Current integration boundary

Blaze remains explicitly opt-in in angelX. Prior emitted-image restoration and
full-host sandbox/nested-browser incompatibilities remain open; this investigation
does not clear them or promote the daemon backend. No pinned compiler binaries,
Blaze checkout sources, dependency features, sandbox rules, or toolchain receipts were
changed. No remote service, provider request, GitHub issue, or PR was sent.

The wider Rust reload gate is blocked before compilation by the existing operator
lock mismatch documented in the companion report. The next safe upstream action is
an independently instrumented budget reduction after that local verification gate is
reconciled, not bypassing it or declaring whole-system acceleration.
