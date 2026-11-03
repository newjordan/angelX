# Blaze PR candidate: stop traversing the same build-script input twice

2026-10-28. Local measured reduction; no upstream publication or installation change.

## Why this is a separate opportunity

The pinned `indexable-inc/blaze` revision
`b383b0bcf5da33816809b00a37e1c36a0e761efb` has
`crates/ra-jit/src/cargo/script.rs:415`, `watched(stdout, cwd)`.
For each `cargo:rerun-if-changed` or `cargo::rerun-if-changed` directive it
independently expands the directory tree. Directives naming the same directory,
a subdirectory, or an explicit child therefore repeat traversal. At line 309,
these paths are appended to sandbox-observed reads. Later
`crates/ra-jit/src/cargo/keyed.rs:185–187` sorts/deduplicates reads, **after** the
expansion cost has occurred. Deduplicating output alone cannot recover that time.

A visited set at the beginning of the `pending.pop()` loop avoids repeat
expansion of exactly the same `OsString` spelling, across all directives in that
call. Do not canonicalize: distinct symlink and `..` spellings must retain their
own tracking. Missing paths must still be included once. This does not change
sandbox permissions, cache-validation rules, the daemon, or artifact publication.

## Executable reduction

```
python3 scripts/perf/repro-blaze-watch-overlap.py <new-receipt.json>
```

The probe reads the exact function from the local pinned checkout, records its
SHA-256, compiles it alongside the two-line visited-set variant with stable
Rust 1.95, and compares sorted distinct path sets. It does not compile/start a
Blaze daemon, load V8, download dependencies, or invoke any provider.

Fixture: 512 regular files in 65 directories, repeated root directives,
explicit files, missing paths, a Unix symlink and a `..` spelling.
The actual directory fixture is disposable; the full compiled Rust harness,
source/function hashes and observations are retained.

Measured receipt:
`reports/performance/blaze-watch-overlap-reduction-20261028.json`.

| Metric | Pinned function | Visited-set variant |
|---|---:|---:|
| Expanded output entries | 1,669 | 580 |
| Distinct path spellings | 580 | 580 |
| Median for 20 walks (6 alternating-order rounds) | 17.520 ms | 8.478 ms |

The distinct path sets were equal, including missing/alias spellings. This is
**51.6% less traversal time on the selected overlap fixture**, roughly
0.876 → 0.424 ms per function call. It is not a measured end-to-end Blaze
speedup. Cold filesystem, nonoverlapping lists, larger trees, Windows and
concurrent mutation were not measured. The set adds allocations; nonoverlap
benchmarks are necessary before claiming universal benefit.

## PR-ready artifacts, not a submitted PR

- `reports/performance/blaze-watch-overlap-candidate-20261028.patch`: minimal
  upstream function edit, kept separate from the installed pin.
- `reports/performance/blaze-watch-overlap-reduction-20261028.rs`: full generated
  baseline/candidate harness used for the receipt.
- `scripts/perf/repro-blaze-watch-overlap.py`: repeatable reduction.

Before upstream merge, extend the existing
`script::tests::what_rerun_if_changed_names_is_read_every_file_of_a_directory`
with overlapping root/subdir/file directives; retain edits, missing-path,
addition/removal, aliases and sandbox-key behavior. Run the existing ra-jit
unit target under its pinned nightly and benchmark nonoverlap as well as
representative real build-script output. That upstream integration gate is
**pending**, not substituted by this isolated harness.

## Relationship to angelX's measured change

angelX now minimizes its browser-package Cargo watch directives when a real,
canonical ancestor directory already provides recursive freshness coverage.
That removed 2,146 entries (2,860 → 714) while preserving aliases and external
inputs. This upstream proposal is different: Blaze should tolerate redundant
but valid directives from any package without traversing identical spellings
repeatedly. The two optimizations compose; neither authorizes automatic Blaze
promotion. Existing full-host publication/sandbox blockers still apply.
