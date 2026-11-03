# Blaze PR opportunity: a qualified callback boundary, not another global bypass

Date: 2026-10-28 · iteration 12 · local-only evidence, no upstream submission.

## New evidence

The cockpit's V8-heavy implementation is now an independent `angel-code-mode`
path dependency. The host owns policy, footprint scheduling, concurrency, and
worker identity; only synchronous borrowed callbacks and JSON/results cross the
boundary. This is a real application compilation boundary, not a reduced
feature profile or a different JavaScript engine.

To qualify a small piece of that shape for Blaze, the existing reproducible
freshness probe gained `--callback-dependency`:

```sh
python3 scripts/perf/repro-blaze-image-freshness.py NEW.json --callback-dependency
```

It constructs a tiny Rust library with this API:

```rust
pub fn run(invoke: &(dyn Fn(i32) -> Result<i32, String> + Sync)) -> Result<i32, String>
```

The library calls the borrowed trait object inside a joined scoped worker and
transforms the returned `Result`. The application captures a multiplier in its
closure. Only the **dependency implementation** changes (`n + 1` → `n + 7`),
not the root source or the public API. Sources are then restored. Every built
image is executed and checked, with source and image hashes recorded.

Observed results on the pinned installed compiler
`b383b0bcf5da33816809b00a37e1c36a0e761efb`:

| Step | Wall seconds | Expected and observed stdout |
|---|---:|---|
| prime | 0.609 | `callback-7` |
| unchanged | 0.015 | `callback-7` |
| implementation edit | 5.682 | `callback-13` |
| restore | 0.194 | `callback-7` |

All four builds and executions exited 0. The edit and restore logs do not have
fallback/timeout markers. Raw receipt:
`reports/performance/blaze-callback-dependency-iteration12.json`; four adjacent
logs retain actual compiler output. The isolated daemon was stopped afterward.
This is **not** qualification of the cockpit, V8's build script, V8 FFI, or a
universal Rust callback performance benchmark.

## PR 1: callback-boundary image freshness regression

Port the tiny fixture into Blaze's Cargo-wrapper regression suite. Preserve:

- a path dependency with the borrowed `dyn Fn + Sync` API above;
- scoped worker invocation with non-static closure captures;
- `Result` return values across the crate boundary;
- an implementation-only edit and restore with API/root source unchanged;
- executable-output assertions after all four states, not just Cargo exit 0;
- explicit recording/assertion of the chosen compiler route, so fallback is
  never silently called JIT success.

This fixture currently passes. It is a regression contribution, **not a newly
reproduced stale-image bug**. It supplies positive evidence for an incremental
migration boundary while keeping the earlier image-publication issues separate.

## PR 2: package-scoped, version-visible compatibility routing

Application modularization exposes a tooling gap: one incompatible build script
should not force every project script through the same compiler route.

Current source anchors in the pinned checkout:

- `crates/ra-jit/src/cargo/own_script.rs`: `BY_RUSTC` names the global
  `RA_JIT_SCRIPTS_BY_RUSTC` switch.
- `crates/ra-jit/src/cargo/wrapper.rs:124`: the switch controls `unit.by_ra_jit`
  for every script. The wrapper already reads `CARGO_PKG_NAME` in other paths.
- `<blaze>/docs/no-rustc.md`: documents the all-script diagnostic reference route.

Propose an opt-in package/version-scoped reference-compiler policy, with a
machine-readable route receipt carrying package, version, compile-vs-execution
stage, policy source, and reason. Empty or absent policy must preserve current
behavior. An unknown/malformed rule should fail with a diagnostic rather than
silently broadening to all packages. The existing global reference switch can
remain explicitly diagnostic.

Acceptance cases before recommending it to angelX:

1. Two build-script packages in one graph, only the named package/version takes
   the selected reference route; the other retains its ordinary route.
2. A nonmatching version, empty policy, and malformed rule each have explicit,
   deterministic outcomes.
3. Script compile routing does **not** imply that its execution sandbox is
   disabled. File-denial controls and actionable errors remain tested.
4. The v8 150 parent-of-OUT_DIR state layout remains a separate compatibility
   problem; selective rustc compilation alone must not be advertised as its fix.
5. Freshness and edit/restore executable assertions still pass with mixed routes.

This is a source-grounded design opportunity, not an installed/benchmarked Blaze
patch. No Blaze safety boundary was widened; the production Cargo default and
explicit Blaze opt-in/rollback remain unchanged.

## Why this is distinct from prior work

Earlier work isolated V8's build-script sandbox contract, native-library
replay, embedded bytes, feature selection and watch overlap. This iteration
qualifies a **borrowed host callback across a Rust path dependency**, and obtains
its actual application benefit by removing V8 typechecking/codegen/test linkage
from the cockpit's ordinary unit. It does not repeat the old parent-lock denial
as a new finding or count reference-compiler fallback as Blaze acceleration.
