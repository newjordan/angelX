# Host-only codegen partitions: measured edit-loop reduction

## Decision

Keep `codegen-units = 1024` only in `[profile.fast.package.angelX-cockpit]`.
The fast profile's optimized dependencies retain 256 units. The release profile
retains one unit and thin LTO. No features, source-freshness checks, sandbox
checks, browser assets, or native objects are removed. The qualified default is
still Cargo; this change does not promote the experimental Blaze backend.

This is a **warm, local, semantic edit-loop improvement**, not a claim of globally
optimal compilation, faster clean builds, faster gameplay, or faster startup.
The target machine was the Ryzen 9950X3D, Linux x86_64, pinned rustc 1.95.0.

## Why this experiment

The existing executable already identifies its linker as LLD 22.1.2. A warmed
256-unit restore phase trace reported 6.442 s in `codegen_crate`, 4.563 s in
`codegen_to_LLVM_IR`, and 0.939 s in `run_linker`. Compiler phases overlap; they
must not be added together. A different linker is not the first lever here.
Finer host partitions can leave less LLVM work dirty after a small source edit.

`measure-dev-rebuild.py --compiler-phases` records rustc's `-Ztime-passes` through
an explicit, retained normal `RUSTC_WRAPPER`. It enables `RUSTC_BOOTSTRAP=1` only
for the host diagnostic invocation, not dependencies or the normal launcher.
This does not request a different optimization level, but diagnostic changes
can invalidate incremental compiler state. **The first diagnostic semantic edit
was 48.79 s and is not a valid warm baseline.** A warmed control and subsequent
uninstrumented runs are included below instead of silently dropping that cost.

## Measurements and retained receipts

All JSON names below are in `reports/performance/`, with `-20261028.json`
appended. Each receipt has adjacent exclusive per-step logs. Times are seconds,
except unchanged launch (median of three samples, milliseconds). The edit and
restore commands execute the binary and check the marker, not just Cargo's exit.

| Receipt stem | Prime | Semantic edit | Restore | Unchanged ms |
|---|---:|---:|---:|---:|
| `compiler-phase-baseline` (256, diagnostic) | 0.36 | 48.79 | 15.13 | 243.2 |
| `compiler-cgu256-control` (diagnostic) | 9.71 | 16.94 | 13.60 | 245.8 |
| `compiler-cgu1024` (diagnostic) | 50.23 | 12.67 | 11.96 | 236.7 |
| `compiler-cgu2048` (diagnostic) | 49.24 | 10.67 | 13.07 | 229.0 |
| `compiler-cgu1024-leaf-normal` | 0.25 | 12.24 | 11.42 | 246.2 |
| `compiler-cgu256-leaf-normal-control` | 14.33 | 18.83 | 14.34 | 227.1 |
| `compiler-cgu1024-final-normal` | 11.19 | 10.68 | 11.27 | 247.2 |

The uninstrumented leaf edit changes a JSON field in
`cockpit/src/platform/runtime_paths.rs`. Its edit-plus-restore cycle fell from
33.17 s at 256 units to 23.66 s, then 21.94 s, at 1024 units: about **29–34% less
elapsed time** in these local runs. Do not present the fastest individual sample
as a universal speedup. Warm cache history, source shape and system load matter.

The 1024-unit phase trace reported `codegen_crate` of 2.987/2.829 s for edit and
restore, versus 6.442 s for the initial warmed 256-unit restore. The final link
can become slightly more expensive as object count grows. This explains why
more units are not automatically better.

### A second semantic edit catches overfitting

The new `--case version` modifies the crate root's actual `--version` output,
checks the changed executable, restores the source, then checks the old output.
These are normal compiler invocations, without `--compiler-phases`.

| Receipt stem | Prime | Root edit | Restore | Unchanged ms |
|---|---:|---:|---:|---:|
| `compiler-cgu2048-root` | 0.25 | 48.18 | 12.08 | 240.8 |
| `compiler-cgu2048-root-warm` | 0.24 | 16.39 | 12.12 | 234.5 |
| `compiler-cgu256-root-control` | 45.35 | 12.34 | 9.70 | 235.3 |
| `compiler-cgu1024-root-control` | 48.19 | 11.16 | 10.80 | 243.2 |

The initial root edit's 48.18 s cost is retained; subsequent tests warmed its
changed code path. Switching profiles also caused 45–50 s prime builds. The
warmed root cycle at 2048 was 28.51 s versus 22.04 s at 256: **reject 2048**.
The 1024 root cycle was 21.97 s, effectively tied with 256. Therefore 1024 is a
bounded local compromise, not a universal winner on every edit.

## Runtime guard, not a runtime benchmark

Existing PTY measurement entry point:

```sh
python3 scripts/perf/measure-terminal-ready.py reports/performance/compiler-cgu256-terminal-20261028.json
python3 scripts/perf/measure-terminal-ready.py reports/performance/compiler-cgu1024-terminal-20261028.json
```

Three samples each, direct binary PTY scope (not the source launcher):

| Median | 256 units | 1024 units |
|---|---:|---:|
| First output | 27.63 ms | 26.91 ms |
| Initial draft visible | 45.12 ms | 43.33 ms |
| Input echo | 0.60 ms | 0.65 ms |

This small smoke check found no obvious terminal-readiness regression. It does
not measure browser rendering, graphics performance, a full agent turn, or CPU
throughput. Source-launch unchanged samples are ~227–247 ms across controls and
candidate (one final outlier was 346 ms); **no unchanged-startup gain is claimed**.
The 1024-unit executable was 262,304,880 bytes versus the original observed
262,245,296 bytes, while 2048 grew to 282,844,344 bytes. Those size observations
are not controlled clean-build comparisons.

## Reproduction and invariants

```sh
# Use a new receipt name each time. No concurrent source editor/build.
python3 scripts/perf/measure-dev-rebuild.py reports/performance/my-leaf.json
python3 scripts/perf/measure-dev-rebuild.py reports/performance/my-root.json --case version
# Optional host diagnostic, not a normal dev setting:
python3 scripts/perf/measure-dev-rebuild.py reports/performance/my-phases.json --compiler-phases
```

Each probe uses the actual source launcher, Cargo freshness checking, full
browser-enabled fast build, no provider calls, no `ANGEL_NO_BUILD`, and no
`ANGEL_LAUNCH_BINARY` substitution. The marker must appear only in the edited
image and disappear after restoration. Timeouts kill and reap the owned process
group before source restoration; concurrent source changes are not overwritten.
All eleven recorded cycles above exited zero with both executable marker checks
and source restoration recorded true. Final source hashes:

- `cockpit/src/platform/runtime_paths.rs`: `e1ea1bf8006c10c1e0022fb73a8f5fd48f59fc587ca558fa43dbe6616675df8a`
- `cockpit/src/main.rs`: `58f4e393afd4191a633994669a61d0871b30d30b5fd91e3cf312eb871ba12b52`

Independent review was attempted but timed out without findings. It is **not**
a review pass. No broad suite was rerun for this profile-only production change;
the actual full-feature builds, semantic execution/restoration and focused PTY
guard are the gates used here.

## Limitations and next decision

The machine is disk-constrained and already has many incremental namespaces;
no global cache cleanup, dependency rebuild or toolchain flag change was used
to manufacture a faster result. First compilation/profile-switch cost remains
material. There is no randomized, multi-host, broad edit corpus here.

Rollback the one package override if a broader edit corpus or runtime workloads
show a net regression; do not increase all dependencies to 1024 units. Further
work should profile representative host edits and independent crate boundaries,
not extrapolate this small leaf edit to the whole application. Blaze adoption
still needs its separate nested-build and emitted-image correctness gates.
