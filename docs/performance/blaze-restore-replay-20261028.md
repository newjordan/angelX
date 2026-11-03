# Blaze cached build-script replay: avoid rewriting identical outputs

2026-10-28 · bounded source reduction; no upstream publication or installed Blaze changes.

## New concrete PR opportunity

Pinned Blaze `b383b0bcf5da33816809b00a37e1c36a0e761efb`,
`crates/ra-jit/src/cargo/script.rs:706–718`, `Kept::restore` replays every saved
OUT_DIR file with `fs::write`, even when the destination already holds the exact
bytes. This writes large artifacts again and changes their mtimes. The saved run
can contain up to 256 MiB (`MOST_KEPT` at line 466). This is distinct from watcher
deduplication, daemon polling, and artifact-publication correctness.

The proposed optimization checks a regular non-symlink file's length and exact
bounded contents before skipping **only its data write**. Missing or changed
files still write, failures still propagate, and executable permission repair
still runs. Do not skip by length/mtime alone, and do not skip build-script
stdout/fingerprint/timestamp publication: those have different Cargo semantics.

## Reproducible evidence

```
python3 scripts/perf/repro-blaze-restore-write.py reports/performance/blaze-restore-write-iteration10.json
python3 scripts/perf/repro-blaze-restore-write.py reports/performance/blaze-restore-changed-iteration10.json --fixture changed
python3 scripts/perf/repro-blaze-restore-write.py reports/performance/blaze-restore-small-iteration10.json --fixture small
```

Each command compiles the extracted method and one candidate with Rust 1.95.0
`-O`, without compiling or launching Blaze. The reduction changes only the error
context adapter (`anyhow` → stdlib IO) and method names. Frozen original/candidate
methods, a proposed patch, the harness source and compile/execution receipts are
beside each JSON. The patch is a method-level design sketch, not an apply-ready
or qualified upstream change. Six samples per configuration alternate order.
Tests cover missing outputs, changed lengths, same-length changed bytes, removed
files, executable permission repair, unchanged mtime, empty files and write errors.
All three runs printed `RESTORE_BYTES_MODES_ERRORS_AND_UNCHANGED_MTIME_VERIFIED`.

| Fixture | Timed work per sample | Original median | Candidate median | Result |
|---|---|---:|---:|---|
| Unchanged 12 MiB + small executable + empty file | 8 restores | 13.974 ms | 4.654 ms | 66.7% lower |
| Changed 12 MiB before every replay | 8 edits + restores | 28.494 ms | 32.058 ms | **12.5% regression** |
| Unchanged 1 KiB + small executable + empty file | 128 restores | 2.113 ms | 0.832 ms | 60.7% lower |

The changed fixture includes the same mutation cost on both sides. Reading to
compare then writing is additional work on a miss. These are warm page-cache
filesystem measurements, **not disk writeback throughput**, daemon cache-hit
rates, compile latency, or whole-angelX startup.

## PR acceptance and decision

Retain the candidate as a measured PR lead, **not an installed optimization**.
A blanket replay change is not justified by a hit-only microbenchmark. Before
merging upstream:

1. Add native Kept/restore tests to the pinned project's test suite, preserving
   byte contents, executable policy, errors and no-op mtime behavior.
2. Test stale files, mode changes, symlinks/non-regular paths and Windows. Keep
   replay path confinement/atomicity rules intact; this reduction does not
   establish those guarantees.
3. Run actual build-script cache hit/miss workloads and end-to-end edit/restore
   tests. Measure hit rate and include the changed-output penalty.
4. Verify downstream Cargo freshness after a byte-identical replay; unchanged
   data-file mtimes must not cause metadata/control output to remain stale.
5. Bound reads and memory, and account for remote/slow-filesystem comparisons.

angelX separately reuses compressed browser outputs only after bounded decoding
and exact comparison with **fresh nested-Cargo output**. That local change has
real package-build measurements; this source reduction makes no Blaze integration
claim. The experimental Blaze backend stays explicit until its previously
recorded full-host freshness and sandbox compatibility failures are resolved.
