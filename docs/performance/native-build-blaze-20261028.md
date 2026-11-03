# Bounded native builds and Blaze qualification — 2026-10-28

> Follow-up: native ownership has since moved from the host build script into
> `cockpit/native` with its own Cargo fingerprint. Paths below describe the
> parallelization measurement at that time; see [the later isolation and
> executable edit/restore result](native-fingerprint-20261028.md).

## New result

The cockpit's Cargo build script now runs its four independent native C compiles
concurrently **within Cargo's job budget**. It borrows at most three immediately
available jobserver tokens, uses its implicit slot for one worker, and holds each
extra token until all compiler children have exited. `--jobs 1`, unavailable tokens,
and unsupported nonblocking jobservers fall back to serial work without waiting
for a token. Native source order in the archive, flags, compiler wrappers, and
compiler/archiver environment tracking are unchanged. Compilation errors prevent
archiving; every spawned compiler is waited for even after another one fails.

Implementation: `cockpit/build_parallel.rs`, `cockpit/build.rs`, and the
`jobserver` build dependency in `cockpit/Cargo.toml`. Jobserver 0.1.34 was already
in the dependency graph; no extra tool installation is needed. The helper itself
is registered as a Cargo build-script input. This introduces no timestamp cache,
no background publication, no source-freshness bypass, and no feature reduction.
Release build flags are unchanged; release native compiles can benefit too.

## Measurements

| Measurement | Observed result | Scope |
|---|---:|---|
| Native phase, serial median | 904.82 ms | Seven alternating serial/parallel pairs |
| Native phase, four slots median | 319.67 ms | Same four C sources, compiler, flags, machine |
| Ratio of medians | 2.83x | Native compile phase only; 585.15 ms saved |
| Object output parity | All identical | SHA-256 of all four objects in every pair |
| Source launcher, unchanged | 238.3 / 238.3 / 247.0 ms | Full application, `--version`, Cargo freshness check |
| Semantic host edit | 15.902 s | Executed build-info contains new semantic marker |
| Host source restore | 14.407 s | Executed build-info no longer contains marker |
| Usable terminal, median | 276.63 ms | Three warm 140x40 PTY runs through source launcher |
| Typed input echo, median | 0.641 ms | Same offline practice PTYs |

The native microbenchmark uses the **production scheduler** with synthetic
available permits, not four unrelated background shell jobs. It checks bitwise
object equivalence, alternates order, and retains every duration and hash.

Do not interpret 2.83x as a startup or Rust leaf-edit speedup. Cargo does not run
the build script on ordinary unchanged launches or all Rust edits. The previous
host edit observation was 19.136 s, but comparing that to 15.902 s here does not
isolate this native change: these are single warm observations with different
cache/host conditions, and that Rust edit need not rerun native compilation.
The new result attributable to this change is the paired native-phase result.

Terminal timing is an offline **measurement fixture** with scanning, LSP and
background/discovery activity explicitly disabled. These fixture controls have
not been installed as product defaults. It is not a cold boot, a glass/window
measurement, or a live provider latency claim. Initial cache refresh in the
full launcher probe was 51.829 s; terminal fixture priming was 12.750 s. The
warm samples do not include those refreshes.

## Verification and retained evidence

- `cargo test --offline --profile fast --manifest-path cockpit/Cargo.toml --test build_fast`:
  **16 passed**, including empty queue, token-free serial operation, overlap with
  extra permits, failing compiler, spawn failure, child reaping before permit
  release, browser bundle round trip, and existing build-policy checks.
- After touching `cockpit/build_parallel.rs`, the same target with
  `--offline --locked --jobs 1` rebuilt and **16 passed**. This exercises an
  actual single-job Cargo build-script refresh, not just a fake token callback.
- `node --test tests/scripts/native-launch.test.mjs`: **35 passed**, including
  synchronous current-source launch, refuse-stale build failure, toolchain path
  handling, experimental Blaze opt-in, private image copying, and queued builds.
- Existing `tests/python/test_build_native_modules.py`: **4 passed** on the host
  unittest runner. This is supplementary coverage of the unchanged reload builder;
  the harness explicitly labels this receipt as untrusted/unlabeled, not an
  independent acceptance verdict.
- `python3 scripts/perf/measure-dev-rebuild.py reports/performance/dev-rebuild-native-parallel-20261028.json --backend cargo`:
  all six steps exited 0, semantic marker observed, original source restored and
  restored executable behavior verified.
- `python3 scripts/perf/measure-native-build.py reports/performance/native-parallel-paired-20261028.json`:
  all 14 compilesets passed, all object hashes matched.
- `python3 scripts/perf/measure-terminal-ready.py reports/performance/terminal-native-parallel-launcher-20261028.json --launcher`:
  three PTYs reached the draft and echoed typed input.
- Independent read-only review found no blocker; it correctly noted that
  nonblocking jobserver token acquisition can be unsupported on macOS, where
  this implementation conservatively stays serial. No macOS/Windows run is claimed.
- Final normal source launch after the fixture HOME was removed rebuilt the host
  in 9.79 s and printed `angelX-cockpit 0.2.6`. It left the normal development
  environment warm. Three subsequent `--version` launches took **249.4 / 239.8 /
  244.0 ms**, all exit 0 (`native-parallel-final-warm-launch-20261028.json`).

Full Cargo commands used the installed toolchain's absolute Cargo/Rustc paths
and the launcher's `ANGEL_BUILD_RUSTC`/`ANGEL_BUILD_PROFILE=fast` metadata. Existing
compiler warnings remain; no warning-clean or full-suite claim is made.

Raw files under `reports/performance/`:

- `native-parallel-cargo-tests-20261028.log`
- `native-parallel-single-job-tests-20261028.log`
- `native-parallel-launcher-tests-20261028.log`
- `native-parallel-paired-20261028.json`
- `dev-rebuild-native-parallel-20261028.json` and its six logs
- `terminal-native-parallel-launcher-20261028.json`
- `native-parallel-review-20261028.txt`
- `native-parallel-final-source-launch-20261028.log`
- `native-parallel-final-warm-launch-20261028.json`

The first benchmark receipt hashes the build script before the subsequent
one-line helper-input tracking addition; the scheduler's recorded digest is
unchanged. The single-job run and real launcher probe cover that final tracking
addition. Earlier iterations' evidence was not rewritten.

## Blaze: a new negative result, not a fix

A new bounded offline tool, `scripts/perf/repro-blaze-image-freshness.py`, runs
prime/unchanged/edit/restore through the installed pinned compiler's Cargo
workspace wrapper and executes each reported-success output. It records source
and binary SHA-256, inode, hardlink count, logs, and daemon-stop receipt. It uses
its own temporary daemon/cache/project and stops that daemon before cleanup.
It never edits the application or installs a global Cargo wrapper.

Two cases passed all four semantic checks:

1. Tiny standalone Rust program (`blaze-tiny-freshness-v2-20261028.json`).
2. Tiny program including output from a rustc-built build script
   (`blaze-tiny-build-script-freshness-v2-20261028.json`).

The earlier receipts without `v2` also passed semantic checks, but their
“unchanged” step rewrote the same source bytes. The corrected no-touch control
takes about 19 ms in both fixtures; old results are retained, not overwritten.
The final probe records its source digest and explicitly disables remote work.

These are **negative reproductions**: neither reproduced the full-host stale
restore recorded in `dev-rebuild-baseline-20261028.json`. They show that a
workspace wrapper, saved-image hardlinks (2–3 links observed), and a simple build
script are not alone sufficient to trigger that failure. They do not prove the
hardlink/settler implementation safe, establish a root cause, qualify a clean
daemon-host build, or measure any application speedup from Blaze. Logs report
Cargo compilation without positive daemon/fallback provenance; daemon latency
or correct daemon execution must not be inferred.

Automatic Blaze promotion remains blocked. `ANGEL_BUILD_BACKEND=auto` stays on
Cargo; explicit Blaze remains experimental. We did not weaken the sandbox or
silently run cached images. See [upstream opportunities](blaze-upstream-opportunities-20261028.md)
for focused PR-sized work, evidence, and acceptance gates.

## Recommended long-term local workflow

- Normal launch: `bin/angelX`. It performs the synchronous freshness check before
  launching the same complete application. An unchanged tree avoids compilation.
- Host edits: use the existing `fast` profile (the launcher default); retain
  optimized dependencies, incremental artifacts and the installed pinned
  toolchain. Avoid `cargo clean` and ad-hoc changing target paths/toolchains/flags
  during the edit loop: those invalidate reusable work.
- Prewarm intentionally: `bin/angelX build --background`; ordinary launch still
  refuses to substitute yesterday's image for current source. Check completion
  before expecting an instant next launch.
- Existing opt-in native module workflow: `bin/angelX build --modules` with
  `ANGEL_NATIVE_MODULES=1` at application launch; unchanged libraries are already
  content-checked and atomically published. This iteration changes Cargo's
  static native compilation only, not that independent Python reload builder.
- Ship/qualify with `bin/angelX build --release`. Do not use working-profile
  timings as release-runtime or graphics-performance evidence.

No finite machine test proves “fastest possible.” Remaining major work is
independently fingerprinting native/browser build-script phases, reducing the
Rust host's compilation/link scope, and qualifying Blaze's fallback/publication
path without losing semantic freshness. No acceptance command is bound to this
loop, so this report records progress rather than declaring the objective complete.
