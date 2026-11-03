# Blaze upstream PR opportunity: validate explicit unwind codegen queries

## Proposed change (not installed)

Pin investigated: `b383b0bcf5da33816809b00a37e1c36a0e761efb`.
Candidate: r5 upstream patch (`reports/performance/blaze-unwind-iteration25/candidate-r5-upstream.patch`, not published).

`crates/ra-jit/src/answers/own.rs` unconditionally places `force-unwind-tables` in `IGNORED_CODEGEN`. This is not safe even for `--print=cfg`: rustc validates boolean values and rejects disabling unwind tables on targets that require them. The emulator instead returns its ordinary cfgs with successful status and empty stderr.

A minimal fix removes that option from the unconditional list, adjusts its fixed array size, and permits it only when the existing `yes(value)` parser returns `Some(true)`. All disabling and malformed forms fall through to `None`. **Do not synthesize diagnostics:** the existing normal answer path asks rustc (or uses a clean successful cached rustc answer). Own-only mode must refuse. Valid enabled forms remain on the no-rustc own path.

Scope: only the unwind parser and one in-module regression. No target database modification, new dependency, engine rebuild or host integration.

## Reproduction on the pinned toolchain

```sh
RUST="$HOME/.rustup/toolchains/nightly-2026-10-27-x86_64-unknown-linux-gnu/bin/rustc"
JIT="$PWD/cockpit/target/tools/blaze/b383b0bcf5da33816809b00a37e1c36a0e761efb/ra-jit"
"$RUST" --print=cfg --target x86_64-unknown-linux-gnu -C force-unwind-tables=bogus
RA_JIT_ANSWERS=own "$JIT" rustc-wrapper "$RUST" --print=cfg --target x86_64-unknown-linux-gnu -C force-unwind-tables=bogus
```

Observed rustc: exit 1, incorrect value diagnostic. Installed own answer: exit 0, cfg stdout, empty stderr. Values `0`, `1`, and empty value also fail validation. Invalid first occurrence followed by `yes` is still an error in rustc; last-wins simplification must not suppress it. Disabled then enabled is valid in the measured rustc; conservative refusal of the whole query correctly delegates it.

The three Windows target families also reject disabling forms. This previously known target constraint was the starting point; malformed values, spelling/repetition behavior and a measured correction are the new results.

## Local validation and exact limits

- Frozen exact complete `answers.rs`, `own.rs`, answer JSON and unstable parser, compiled offline as a small Rust crate on Rust 1.95.0.
- New assertion fails on unchanged production source (test-only binary exits 101 on `force-unwind-tables=no`); candidate passes all 11 module tests; unchanged baseline passes its 10 tests.
- **68/68** selected unwind queries match pinned rustc's **status, stdout and stderr** in normal mode. Baseline fabricated success in 38. Candidate refuses 48 in own-only mode and answers 20 itself; all 38 error queries are refused.
- **345/345** first upstream-harness grid queries match in normal mode with **zero exclusions**, including 10 warning and six error references. Only the six Windows error triples change from baseline normal mode. Candidate refuses 31 grid queries in own-only mode; this is not 345 own-answer successes.
- Two of six unrelated controls (`force-frame-pointers=bogus`, Linux and Windows) remain false successes. Explicitly out of patch scope, and a separate upstream opportunity. No claim of general codegen-option validation.
- **5,169 same-author integrity assertions** check exact source delta, test-only causality, binary hashes, raw per-arm triples, selection completeness, installed-to-reduction own equivalence, protected identity and patch applicability. These are not 5,169 independent behavioral cases.
- Read-only separate-context review supports bounded PR consideration after catching an earlier compile error. Full engine is not qualified.

Raw receipts and failed attempts: experiment tree (`reports/performance/blaze-unwind-iteration25`); report (`reports/research-82abbb708cda5ae57589ee8a17aeaaaa-25.md`). These files are kept with the measurement and are not published.

### Explicit reduction substitutions

The Cargo dispatcher, wrapper discovery, native CPU detection and atomic cache-write helper are support stubs. Native CPU support panics if selected. Unrelated Cargo queries return no own answer. The cache publication substitute uses `std::fs::write`; production atomic publication and concurrent locking are not validated by this reduction. Toolchain detection, argument parser, embedded release data, cache key/read policy and rustc process fallback are the exact answer modules. All 74 installed calibration own triples match the frozen baseline reduction in the differential rerun.

### Performance cost, not a speedup claim

Valid Linux `force-unwind-tables=no` now refuses own answers and can incur one compiler query on a cold clean cache. Small unoptimized reduction samples (process-inclusive, not a release benchmark): baseline cold **8.678 ms**, candidate cold **15.321 ms**. Twelve warm samples: baseline median **7.789 ms**, candidate **7.448 ms**. Enabled `yes` remains own-answered; corresponding warm medians **7.736/7.916 ms**. One cold sample and 12 warm samples cannot establish a meaningful speed win or regression distribution. Invalid/warning compiler answers are deliberately not cached and can cost repeated rustc launches.

## Acceptance gates before upstream merge / automatic host use

1. Run full upstream crate and engine builds, tests and no-rustc/integration harness on the exact patch; test the real wrapper, daemon/spare and Cargo caller paths. The local patch is **unapplied** to the upstream checkout and installed binaries.
2. Validate on supported rustc releases, hosts and targets, including native CPU, alternate print kinds, repeated forms and aliases. The local reference is just nightly-2026-10-27 on Linux.
3. Exercise populated-cache upgrade, clean cache, concurrent answer publication and diagnostic reruns. Prior fabricated own answers return before persistent cache insertion, but compiler metadata-based cache identity has broader pre-existing limitations.
4. Consider a subsequent target-aware implementation to keep valid disabled Linux queries in own mode without weakening malformed-value validation. Do not enlarge this patch without new tests.
5. Resolve the existing angelX protected-file seal failure through the operator, not by editing or resealing around it. Current `loop_ctl.rs` equals HEAD but differs from the recorded seal. No host build/promotion was run.
6. Measure end-to-end cold/warm startup and edit/reload after correctness and authorization gates. This iteration cannot claim record-time startup.

No PR opened, commit, push, remote workload, host reconfiguration or safety-control changes.
