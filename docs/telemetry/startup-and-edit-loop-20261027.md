# Startup and edit loop — 2026-10-27

## Measured terminal readiness

Measured at dirty checkout `acb6562b0`, using the **existing release image**, not
an image rebuilt from the current worktree. Image SHA-256:
`856df416e8fec0fdffec7c972a2579010442442ce88f93616b5c1c7b93a542a3`.

A real PTY at 140 columns × 40 rows was opened for each run. The process received
`--driver practice --draft ready-probe-329174`. Readiness means that draft was
observed in terminal output. After observing it, the probe sent
`echo-probe-853619` and measured until that text was observed. Each process group
was terminated and reaped after the measurement; no model calls were made.

| Route | Run | First output (ms) | Draft visible (ms) | Input echo (ms) |
|---|---:|---:|---:|---:|
| Direct release image | 1 | 32.59 | 54.34 | 0.43 |
| Direct release image | 2 | 33.00 | 44.15 | 0.33 |
| Direct release image | 3 | 33.42 | 44.44 | 0.32 |
| `bin/angelX --term` | 1 | 94.67 | 104.27 | 0.35 |
| `bin/angelX --term` | 2 | 94.02 | 103.92 | 0.43 |
| `bin/angelX --term` | 3 | 95.45 | 105.05 | 0.33 |

Median draft readiness: **44.44 ms direct**, **104.27 ms via launcher**.
This is a warm/local terminal observation, not a cold-cache, Glass-window or
configured-provider startup benchmark. It is also not an edit-to-activation
measurement or evidence that newly edited Rust has been loaded.

The same launcher/PTY measurement was repeated during the real low-priority
fast-profile compile. The matching fast-profile Rust compiler PID `2184888`
was present at the beginning of every sample:

| Run | Draft visible during build (ms) | Input echo (ms) |
|---:|---:|---:|
| 1 | 108.72 | 0.32 |
| 2 | 102.49 | 0.31 |
| 3 | 102.66 | 0.30 |

Median readiness during compilation: **102.66 ms**. These three observations
show that this cached/practice terminal path remained responsive during this
build; they are not a general system-load or Glass latency benchmark.

### Isolation

Each run used temporary HOME, XDG config/cache/state, workspace and session
folders. Environment settings were:

```text
TERM=xterm-256color LANG=C.UTF-8
ANGEL_DRIVER=practice ANGEL_NO_BUILD=1 ANGEL_BACKGROUND_BUILD=0
ANGEL_GLASS=0 ANGEL_SCAN=0 ANGEL_SCAN_INTERVAL_SECS=0
ANGEL_BAG_PROBE=0 ANGEL_HYDRA_DISCOVER=0 ANGEL_HYDRA_PUBLISH=0
ANGEL_LSP=0 ANGEL_EXPERIENCE=0 ANGEL_DOSSIER=0 ANGEL_ROUTE_MEMORY=0
ANGEL_TUI_MOTION=off ANGEL_IMAGE_PROTOCOL=halfblocks
ANGEL_OVERWATCH_CMD=true ANGEL_COMP_MODE=1
```

## Background build correctness

The existing launcher runs a pinned working image while the existing Python
helper owns one low-priority build lease per checkout. Two issues were fixed:

1. An explicit `build --background` request arriving during compilation was
   discarded. It now replaces one pending profile request. The current worker
   services that request after either success or failure. Ordinary interactive
   opens do not create follow-up requests, and never erase an explicit request.
2. A newly scheduled worker left the preceding `succeeded` status visible until
   it started. The scheduler now atomically publishes `queued` before detaching
   the worker; a failed spawn publishes `failed` and propagates the error.

The pending request and worker-to-idle transition share a short-lived request
mutex. The mutex is not held during Cargo. The worker releases its build lease
before releasing that mutex on exit, so a request racing with exit either joins
the current worker or starts a new worker; it is not stranded.

Verification:

- Regression before the queue fix: both targeted tests failed because only one
  build ran, not the required two.
- `node --test tests/scripts/native-launch.test.mjs
  tests/scripts/check-cockpit-fast.test.mjs`: 25 passed before the status fix.
- `node --test tests/scripts/native-launch.test.mjs`: 21 passed after the status
  fix, including the new stale-status regression. These are local runtime test
  results, not independently audited evaluator rewards.

No new daemon, file watcher, Node formatting worker or Rust dependency was added.
The launcher and helper changes apply without recompiling the cockpit.

## Remaining compile and bootstrap boundaries

- A real `nice -n 10 env CARGO_BUILD_JOBS=2 bin/angelX build --profile fast`
  was started separately from launch. It compiled both `angelX-cockpit` and
  `angel-delve`, and was still running after 5 minutes 55 seconds. This lower
  bound is an observation of a dirty-worktree update, not an incremental
  single-edit baseline or a completed build time.
- Cargo's live Delve compiler invocation used `opt-level=3`,
  `codegen-units=256`, incremental compilation and nice level 10. The host has
  an `opt-level=1` override. Lowering dependency optimization needs runtime
  measurements; it is not automatically a safe startup fix.
- Browser packaging is already skipped before registering browser-specific
  source watches in the fast profile (`cockpit/build.rs::delve_web`).
- LSP subprocesses already start on demand unless explicit prewarming is
  requested. MCP discovery still connects/lists configured servers during
  registry construction; the first-frame impact with configured servers has
  not been measured here. Moving it requires preserving provider registration,
  unload, cancellation and tool-search semantics.
- Host Rust edits still require compilation and a new process. Existing content
  and native-module reload facilities are not a general Rust hotloader.
- No Blaze executable or existing Blaze runtime integration was found in this
  checkout or on PATH. No speculative runtime was added. Delve's independent
  non-Rust remake remains outside this change.

The next structural target is the Delve-to-host dependency edge, not arbitrary
file splitting or moving native report formatting into another process.

## Disk inspection

`cockpit/target/debug/incremental` reported 246 GiB, but the debug target is a
symlink to `<build-cache>/angelX-cockpit-debug`, outside this workspace. The
cleanup guard refused to delete through that boundary. **No cleanup occurred.**
The working release/debug/fast executables and active fast build were retained.
The shared debug cache is a cleanup candidate only with access to its actual
storage scope and coordination with its other users; it is not reported here as
reclaimed space.
