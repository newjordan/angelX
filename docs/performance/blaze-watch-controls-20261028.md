# Blaze watcher deduplication: non-overlap controls

This is a new limitation on the proposed watcher PR in
[the overlap reduction](blaze-watch-overlap-20261028.md), not a claim that the
upstream patch is already integrated or qualified.

## Question

The earlier reduction showed a large win when rerun directives overlap a tree,
files, symlinks and repeated directory roots. Does its per-path visited HashSet
also help a normal, non-overlapping watch set? A PR needs the negative controls,
not just the motivating best case.

## Method and explicit receipts

`repro-blaze-watch-overlap.py` now accepts `--fixture tree` (one root directory)
and `--fixture files` (512 distinct files, no directory roots). The default
`overlap` fixture is unchanged. Every fixture uses 64 subdirectories and 512
regular files. The symlinks present in the tree fixture are files, not directory
cycles. Compile the extracted upstream function and the same proposed patch
with rustc 1.95.0 `-O`; alternate baseline/candidate over six paired samples,
20 walks per sample, and assert identical sorted unique path sets.

```sh
python3 scripts/perf/repro-blaze-watch-overlap.py reports/performance/blaze-watch-tree-control-20261028.json --fixture tree
python3 scripts/perf/repro-blaze-watch-overlap.py reports/performance/blaze-watch-files-control-20261028.json --fixture files
```

Both completed with `WATCHED_REDUCTION_SET_EQUAL`. Explicit receipts are the
JSON files above plus their `-original.rs`, `-candidate.rs`, and `-candidate.patch`
siblings in `reports/performance/`. Extracted source SHA-256:
`2a5ad9a6dcc988d9b87f593f714609f469b94d2d49df21a96e092eb608d7e8c1`.
These are bounded function reductions, **not whole-Blaze compile measurements**.
The cached upstream checkout and installed Blaze executable were not modified.

## Results: negative controls matter

Median elapsed time in milliseconds **per 20 walks**, not per build:

| Fixture | Baseline paths | Candidate paths | Baseline ms | Candidate ms | Change |
|---|---:|---:|---:|---:|---:|
| Earlier overlap reduction | 1669 | 580 | 17.520 | 8.478 | -51.6% |
| One tree, no repeated roots | 577 | 577 | 6.574 | 7.672 | +16.7% |
| 512 distinct file directives | 512 | 512 | 4.497 | 5.212 | +15.9% |

Six-sample ranges for the controls:

- Tree baseline: 6.443–6.816 ms; candidate: 7.467–8.532 ms.
- Files baseline: 4.222–5.036 ms; candidate: 4.954–5.703 ms.

The candidate's unconditional owned-path HashSet adds allocation/hashing even
when no duplicate traversal can be saved. The absolute median penalty is about
0.055 ms per tree walk or 0.036 ms per file-list walk in this fixture. The
relative regression is real in these samples; it should not be hidden by the
larger overlapping-tree win. Sample order/cache/load are local limitations.

## PR recommendation

Keep watcher deduplication as a concrete upstream opportunity, but qualify it:

1. Include both new controls and a representative real build-script watch set
   in the upstream benchmark, alongside the overlap reduction.
2. Compare unconditional visited tracking against a root-aware/fast-path design
   before claiming a general performance win. Any alternative must preserve
   missing paths, both directive spellings, raw/relative names and symlink alias
   behavior; path-set equality does not establish ordering compatibility.
3. Include a separately bounded directory-symlink-cycle fixture if claiming cycle
   protection. The existing raw-path HashSet is not proof of inode-cycle safety:
   path strings may grow forever through directory symlinks. This iteration did
   not add a cycle or change confinement, and makes no cycle-safety claim.
4. Do not confuse a smaller watch-vector reduction with the independent stale
   linked-image or nested browser/V8 sandbox gates. Default Cargo remains right
   until those gates are fixed and independently requalified.

No upstream PR was submitted, no remote job/provider call ran, and no installed
compiler patch was applied. The useful deliverable is a reproducible measured
trade-off that should improve the eventual PR rather than a one-sided speed claim.
