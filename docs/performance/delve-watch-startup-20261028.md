# Delve build-input minimization and a cheaper Blaze traversal

## New production change

`crates/delve/build.rs` previously emitted 2,800 `rerun-if-changed` directives.
It recursively watched `assets/arcade` and `assets/art-slots`, then separately
named their frames, maps, masks and kit folders, sometimes repeatedly. Cargo
recursively walks directory watches. Blaze's `watched()` additionally attempted
`read_dir()` on every file reached through those directories.

The Delve build script now collects dependencies and emits **344 directives**
(a **87.7% reduction**). `build_watch.rs` removes duplicates and canonical
children of existing watched directories. It preserves missing paths, symlink
aliases and noncanonical spellings rather than guessing at their coverage.
External boss frames and portraits remain explicitly watched. `build.rs`, the
new helper and `arcade_gate.rs` are explicit inputs. The old one-argument
`arcade_gate::validate()` remains available for the library's existing tests;
the build uses `validate_watched()` to collect inputs before minimizing them.

All five generated files matched their pre-change SHA-256 values:
`slot_art.rs`, `boss_assets.rs`, `arcade_assets.rs`, `archivist.rgba`, and
`courier.rgba`. No sprites, palette gates, browser/media features, safety
settings, compilation profile or ordinary-launch freshness checks were removed.

## Warm source-launch measurement

Command: `python3 scripts/perf/measure-delve-watch-startup.py reports/performance/delve-watch-controlled-startup-iteration9.json`

The probe saves exact candidate bytes, temporarily selects the two HEAD Delve
build files as control, then restores the candidate and builds it. Configuration
order is candidate / baseline / baseline / candidate, eight timed samples after
a prime in each phase. No concurrent builds/editors ran. Both configurations use
the full-feature ordinary source launcher, Cargo stable 1.95.0, the existing fast
profile, and identical `angelX-cockpit 0.2.6` output. No no-build shortcut or
cached-launch substitution was used.

| Warm source-launch `--version` | Baseline, 16 samples | Candidate, 16 samples |
|---|---:|---:|
| Median | 230.16 ms | 224.45 ms |
| Range | 225.26–238.77 ms | 218.98–233.56 ms |

This is **5.71 ms / 2.5% less local median latency**, not a large startup win,
cold-boot result, interactive-ready measurement or proof of global optimality.
The transition prime builds cost 20.22 / 21.24 / 0.23 / 20.22 seconds, retained
in the receipt but excluded from the explicitly warm comparison. They are not
evidence of improved edit compilation time.

The original startup trace placed about 215 ms in Cargo and 27 ms in the
executable. Three-sample exploratory Cargo controls (`-j 1`, `--offline`, and
`--bin angel`) were not convincing enough to change launch policy. The dominant
remaining budget is Cargo graph/freshness work, not shell path resolution.

## Correctness gates and retained diagnostics

- Existing cockpit build-policy entry point:
  `cd cockpit && cargo test --locked --profile fast --test build_fast`:
  **19 passed**, including two new Delve path-minimizer cases. Receipt:
  `reports/performance/delve-watch-policy-fixed-iteration9.log`.
- Independent read-only review caught an omitted library-test callsite after
  changing the validator signature. The compatibility wrapper fixes it. The
  existing test, from the owning workspace,
  `cd crates && cargo test --locked --profile fast -p angel-delve --lib arcade_assets_and_exact_palette_pass_the_build_floor`:
  **1 passed**. Receipt: `reports/performance/delve-watch-api-workspace-iteration9.log`.
- Real Cargo invalidation:
  `python3 scripts/perf/measure-delve-watch-freshness.py reports/performance/delve-watch-real-invalidation-iteration9.json`.
  An approved map whose explicit watch was removed was edited; the generated
  match arm disappeared. Adding a nested slot created its match arm; removing it
  removed that arm. Restoration recovered the original generated hash. All five
  builds exited zero; map bytes were restored and temporary slot removed.
- `reports/performance/delve-watch-candidate-launches-iteration9.json` records
  the five-file equality and directive count. Final full source launches in the
  controlled comparison execute the restored candidate and exit zero.

Two failed attempts are retained, not hidden: the first implementation had a
moved-vector compile error; the first library-test command was issued from the
cockpit workspace, where Delve is not a workspace member. The subsequent fixed
build and owning-workspace test above are the conclusive gates. No broad suite
or game FPS loop was started.

## Blaze: real workloads exposed a better direction

The existing extracted-function reduction now accepts `--directives` plus
`--cwd`, replaying actual build-script output. Original function source is pinned
to Blaze `b383b0bcf5da33816809b00a37e1c36a0e761efb`; source and harness hashes are
in each JSON receipt. Installed Blaze and its checkout remain unchanged.

On the original Delve directives, the earlier unconditional visited-set patch
reduced 17,540 expanded paths to 3,293 and median time per 20 walks from 242.00
to 61.12 ms. On the minimized list it had no duplicates to remove and increased
50.92 to 77.26 ms. The latter samples were noisy and ran during a separate local
library test; they justify caution, not a precise universal regression estimate.

A materially different candidate uses `DirEntry::file_type()` to avoid opening
known nondirectory children. Explicit roots, symlinks and unknown entry types
still use `read_dir()`. It needs no per-path HashSet and preserves multiplicity.

| Fixture | Original ms / 20 walks | Entry-type candidate | Local change |
|---|---:|---:|---:|
| One tree | 6.893 | 3.538 | -48.7% |
| Distinct explicit files | 4.242 | 4.517 | +6.5% |
| Synthetic overlap | 17.421 | 11.591 | -33.5% |
| Real original Delve list | 225.575 | 123.034 | -45.5% |
| Real minimized Delve list | 48.975 | 28.992 | -40.8% |

Six alternating paired samples per fixture, 20 walks each, compiled with
`rustc -O`. Every extracted-function run exited zero. Initial controls assert
path-set equality and equal counts; the strengthened final symlink fixture
asserts equality of sorted **multisets** too. It covers an explicit file alias,
a discovered directory symlink, a dangling symlink and missing roots. Directory
cycles are intentionally excluded: no cycle-safety improvement is claimed.

Reproduce with a fresh output name:

```sh
python3 scripts/perf/repro-blaze-watch-overlap.py reports/performance/my-real-watch.json --algorithm entry-types-only --directives cockpit/target/fast/build/angel-delve-ddbee146b3322846/output --cwd crates/delve
python3 scripts/perf/repro-blaze-watch-overlap.py reports/performance/my-symlink-watch.json --algorithm entry-types-only --fixture overlap
```

Explicit receipts: `reports/performance/blaze-entry-only-*-iteration9.json`
and their extracted `.rs` sources; formatted upstream candidate:
`reports/performance/blaze-entry-types-only-candidate-iteration9.patch`.

This is a promising small upstream PR: fewer file-opening syscalls with less
semantic surface than path deduplication. File-only directives still pay a
small local penalty. TOCTOU behavior, filesystems without cheap entry types,
Windows, upstream tests and whole-build timing need independent qualification.
No upstream PR was submitted. These are traversal measurements, **not Blaze
compile speedups**. They do not resolve the separate stale-image and nested
build sandbox blockers; Cargo remains the default.
