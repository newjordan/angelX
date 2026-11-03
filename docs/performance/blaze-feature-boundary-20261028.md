# Blaze opportunity: feature-graph cache identity and link-closure diagnostics

2026-10-28 · pinned Blaze `b383b0bcf5da33816809b00a37e1c36a0e761efb`.
Follow-up to the [file-video startup change](video-playback-startup-20261028.md).

## New measured boundary

A manifest-only feature split removed unused FFmpeg capture initialization from
the cockpit's normal startup. No application Rust source edit or version marker
was required to change its eager dynamic-library closure (162 -> 120 entries on
this Linux machine). A whole-program/incremental compiler must therefore include
resolved dependency features and rustc cfgs in cache identity; source hashes and
unchanged exported signatures alone do not establish equivalent executable
behavior or link closure.

Added `--feature-dependency` to the existing bounded executable freshness probe.
It creates a tiny Cargo application and path dependency with fixed Rust sources:

```toml
[features]
default = ["capture"]
playback = []
capture = ["playback"]
```

The root selects `default-features=false, features=["playback"]`; the edit changes
only `default-features` to `true`; restoration returns it to `false`. A function
with cfg-selected implementation prints `playback` or `capture`. Every successful
build is executed and checked against the expected output. This checks feature
selection across the Cargo wrapper boundary, not FFmpeg FFI code generation.

## Observed result, not an upstream bug claim

| Step | Executed output | Build+wrapper wall time | Fresh |
|---|---|---:|---|
| Prime | playback | 508.14 ms | yes |
| Unchanged | playback | 18.41 ms | yes |
| Manifest feature edit | capture | 57.64 ms | yes |
| Restore | playback | 15.87 ms | yes |

All four builds exited 0 and executed correctly. Logs contain no fallback or
timeout markers. The existing probe uses the installed Cargo workspace wrapper
with Rust-backed build scripts, and saves stdout, executable hash, inode/link
counts and build log per step. This receipt establishes executable freshness
through that configured path; it does not independently attribute every compiled
unit to Blaze rather than Rust. Daemon shutdown returned 0. These are single
fixture observations, **not** whole-host compile speed or a warm-startup benchmark. No stale artifact was observed in this fixture. Neither this passing
case nor prior passing embedded-asset fixtures overturns the separately observed
full-host edit/restore stale-publication failure.

## Concrete upstream PR candidates

1. **Add this manifest-only feature edit/restore regression to wrapper tests.**
   Keep Rust bytes unchanged, assert outputs after every successful compile, and
   exercise restore replay as well as first edit. The result here supports a
   positive regression test; it does not justify claiming a current feature-key
   defect. A follow-up native-link fixture should also assert that a disabled
   feature's startup constructor/library disappears. That stronger native-link
   fixture has **not** been implemented or measured here.
2. **Add inspectable resolved-feature/link-cost evidence.** Report selected
   Cargo features/cfgs beside saved whole-program/image cache identity, and show
   why an image or replay was reused. Optional ELF/Mach-O dependency comparison
   would make unexpectedly retained device/filter stacks visible before profiling
   host compiler internals. This is an unimplemented diagnostic proposal, not an
   observed performance gain in Blaze.
3. **Reduce host FFI surface before compiler workarounds.** The playback-only
   integration no longer asks FFmpeg/bindgen for the unused device/filter/
   resampling API surface. This is a supported host dependency selection, not a
   patch to Blaze MIR. It does **not** remove `clang-sys`, demonstrate that its
   standalone `IncompleteExpr` is fixed, or qualify the V8 daemon. Keep the
   existing wrapper containment and opt-in policy.

The highest-priority upstream blockers remain synchronous image publication /
restore freshness and build-script sandbox fidelity. Those need their own exact
regressions before automatic Blaze selection is safe. See
[the earlier opportunity inventory](blaze-upstream-opportunities-20261028.md).

## Reproduce

```sh
python3 scripts/perf/repro-blaze-image-freshness.py NEW.json --feature-dependency
```

The result name must not already exist. Retained receipt:
`reports/performance/video-playback-boundary-iteration11/blaze-features.json`;
matching `blaze-features-{prime,unchanged,edit,restore}.log` files and a frozen
measured probe are adjacent. The receipt binds installed compiler hashes and
probe hash. No global toolchain configuration was changed.
