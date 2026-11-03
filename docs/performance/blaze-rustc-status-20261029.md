# Blaze toolchain probes lose child exit status

2026-10-29 · iteration34 · upstream PR opportunity; installed Blaze unchanged.

## New bounded finding

Pinned Blaze `b383b0bcf5da33816809b00a37e1c36a0e761efb`,
`crates/ra-jit/src/cargo/state.rs:1070–1073`, `rustc_says`, accepts an
`Output` from `answers::answer` and converts its stdout to `Some(String)` without
checking its exit status. `cargo/toolchain.rs:254–260` consumes that stdout as a
compiler version or sysroot. A failed compiler shim can therefore supply
plausible metadata which these consumers accept.

This is distinct from prior environment/cache-key bugs: `answers.rs:182–196`
already retains a real executed child's nonzero status and only caches
successful, stderr-free answers. The loss is **after** that boundary. Synthetic
and cached answers have synthetic success status and are outside this reduction.

## Evidence and limits

Kept with the measurement, not published:

- Frozen source-extracted reduction: `reports/performance/repro-blaze-rustc-status-iteration34.py`
- 7-case raw/consumer receipt: `reports/performance/blaze-rustc-status-iteration34/result.json`
- Apply-ready status-guard patch, unapplied: `reports/performance/blaze-rustc-status-iteration34/candidate.patch`
- Separate-context review: `reports/performance/blaze-rustc-status-iteration34/review.md`

One stdlib-only Rust 1.95.0 compile extracts the original helper plus
`version_of` and `sysroot_of`; an adapter executes a controlled, finite shell shim
with `Command::output`. This bypasses the answer engine/cache, not the helper or
consumer logic. Seven scenarios use 21 shim invocations (raw observation plus
baseline/candidate consumer execution per scenario). No Blaze or angelX build.

| Scenario | Original consumer | Status-guard candidate |
|---|---|---|
| Exit 0, plausible version | accepts version | same |
| Exit 23, plausible version + failure stderr | **accepts version** | `None` |
| Exit 0, plausible sysroot | returns path | same |
| Exit 23, plausible sysroot + failure stderr | **returns supplied path** | empty `PathBuf` |
| Exit 23, empty version | `None` | same |
| Exit 0, malformed version | `None` | same |
| Exit 0, empty sysroot | empty `PathBuf` | same |

All seven matched their declared reduction expectations. Two demonstrated
acceptance of failed-probe metadata in the original and rejection of that
metadata with the guard. **The JSON's `NONE` sysroot label is a harness rendering
of an empty `PathBuf`, not real error propagation by `sysroot_of`.** Version
`None` is a real `Option` result. Do not conflate the two.

The patch was checked with `git apply --check` against the unchanged pinned
checkout; it was not applied or compiled in the full upstream crate. Separate
read-only review agreed on the executed-child status mechanism and identified
the important incomplete sysroot failure behavior below.

## PR-sized completion, not a claim of a complete fix

1. Keep the status check, but preserve structured spawn/status/stderr diagnostics.
2. Change sysroot discovery to propagate explicit failure, including **successful
   empty output**, rather than silently producing an empty relative path.
   `with_library` joins `lib/rustlib/src/rust/library` onto the result; an empty
   sysroot can otherwise resolve relative to the working directory. The relative
   working-directory trap is a source-level review finding, not dynamically
   qualified here. Inspect `rust_lld` and other `rustc_says` callers too.
3. Extend actual upstream answer/toolchain-settlement coverage: success,
   nonzero, signal, spawn failure, successful empty/malformed output, named and
   unnamed compiler, working-directory traps, synthetic and cached answers, and
   actionable diagnostics. The reduction is not that integration gate.
4. Keep successful selection and fallback behavior intact across supported hosts.
   Measure actual startup only after correctness review; no speed benefit is
   claimed for this guard.

No issue or PR was published. No installed compiler, daemon, launcher defaults,
Cargo features/freshness behavior, protected source or seal changed. This result
does **not** explain or clear the earlier full-host stale-restore failure, nor
qualify automatic Blaze selection. It is a small evidenced correctness PR
opportunity needed for reliable rapid startup, not a reason to relax gates.
