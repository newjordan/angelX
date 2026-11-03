# Blaze PR opportunity: idempotent Unix build-script interception

Date: 2026-10-28. Upstream inspected: `b383b0bcf5da33816809b00a37e1c36a0e761efb`.
Status: **reproduced with the installed pinned binary; candidate passes an extracted-method control suite; not integrated or submitted upstream**.

## Why this is on the startup/dev-loop critical path

The earlier real-host reload probe stopped with identical shell wrappers at `build_script_build` and `build_script_build.real`. That observation alone did not establish the nesting mechanism. Iteration 21 narrows it to a tiny compile-only fixture, without the host, V8, FFmpeg, browser packaging, daemon, or script execution. This removes a compatibility obstacle to measuring a genuinely daemon-linked host edit; it does **not** establish a startup or compile-time speedup.

## Reproducer and result

The local installed `ra-jit` and adjacent `ra-jit-rustc` at the inspected pin were exercised with actual rustc 1.95.0. A fresh private temporary target was used for each case. Source:

```rust
fn main() { println!("cargo:rerun-if-changed=build.rs"); }
```

The controlled flags include `RA_JIT_ONLY_ASKS=1`, `RA_JIT_EVERY_CRATE=1`, `RA_JIT_SCRIPTS_BY_RUSTC=1`, empty `RA_JIT_CHAINED_WRAPPER`, and `RA_JIT_BUILD_SCRIPTS=native`. The harness extracts the `EVERY_CRATE` variable name from pinned source rather than assuming it. Compiler arguments name `build_script_build`, emit a linked binary, and use Cargo's build-script target layout. The exact commands and clean-environment handling are retained in the scripts/receipts below.

| Compile-only invocation | Exit | Linked output | Saved `.real` |
| --- | ---: | --- | --- |
| Direct rustc | 0 | ELF, 4,336,960 bytes | absent |
| `ra-jit rustc-wrapper rustc …` | 0 | generated shell, 265 bytes | original ELF, byte-identical to direct-rustc output |
| `ra-jit rustc-wrapper ra-jit-rustc rustc …` | 0 | generated shell, 265 bytes | **identical 265-byte shell, not ELF** |

No resulting wrapped script was executed in this installed-binary control. Thus the result demonstrates executable destruction and false successful compilation, not a measured recursive execution or CPU hang. All inspected binary/application/protected identities remained unchanged.

## Causal source path

* `cargo/wrapper.rs:72–104`: recognizes the two wrapper names. Under `ONLY_ASKS`, nevertheless chooses the second wrapper as the compiler and removes `ONLY_ASKS` for that child.
* `cargo/wrapper.rs:110–114,190–205`: that inner child takes the outside-crate path with `EVERY_CRATE` set; a successful rustc link is intercepted.
* `cargo/wrapper.rs:223–285`: outer `for_load` also intercepts the successful link.
* `cargo/script.rs:49–68`: interception unconditionally renames `linked` to `.real`, then writes a generated wrapper. Its second invocation therefore replaces the real executable with the first wrapper.
* `src/bin/ra-jit-rustc.rs:8–18,35–39`: the adjacent wrapper execs the same installed `ra-jit rustc-wrapper`, explaining why the same frontend generates both wrappers.

This is specifically the answer/discovery (`ONLY_ASKS`) nesting path. It is not a claim that every ordinary two-wrapper Cargo build corrupts its build scripts, nor that the full earlier host failure has been replayed.

## Candidate scope

The retained method-level patch:

1. Constructs/quotes wrapper text before mutating any paths, so errors during preparation do not first rename the executable away.
2. Reads only the expected wrapper length plus one byte, not the entire linked executable.
3. For an exact same-frontend wrapper, checks that `.real` is a nonempty executable regular file, not a symlink or another recognizable generated wrapper, then returns without rewriting either path.
4. Refuses recognizable foreign-frontend, missing, nonexecutable, symlinked, or already-corrupted saved states without mutation; diagnostic requests recompilation.
5. Otherwise preserves the original rename/write/permission behavior.

It does not skip `ra-jit build-script`, sandbox execution, fetch checks, replay checks, or the actual build script. No automatic target repair, policy bypass, or host launcher change is proposed.

Candidate: directly applicable upstream patch (`reports/performance/blaze-script-wrap-reduction-r2-iteration21/candidate-upstream.patch`, not published).

## Local controls

The exact original and candidate Unix method bodies were extracted, with real cached `anyhow 1.0.104` and `shlex 2.0.1`; no dependency/error/quoting shims. Cargo ran offline. The tiny delegation fixture used to exercise generated shell semantics is **not** Blaze's confined executor and is not confinement evidence.

The suite exited 0: **8 passed, 0 failed**. It covers baseline repeated-call destruction, candidate repeat stability (8 consecutive repeats, bytes/inodes/mtime/mode), argument and quoted-path preservation, exit-code/output propagation, a legitimate shell script, a real recompile replacing the previous image, missing `.real`, recursive corruption, nonexecutable/symlinked `.real`, and changed frontend path. Some tests cover several related checks. No existing upstream test entry point was found for this specific repeat-interception behavior; this is a newly authored reduction, not an upstream regression-suite pass.

An initial candidate extraction failed to compile because the guard preceded the `lines` declaration. That version and receipt remain unchanged. The corrected extraction moves the destructive rename after shell construction and the guard.

## Remaining acceptance gates / recommended PR shape

This candidate is a bounded serial correction, **not merge-ready hardening**:

* Rebuild the pinned upstream binary with the candidate and repeat the exact installed-wrapper control; both linked output and `.real` must retain valid execution semantics. This has not been done.
* Add the regression to upstream's actual compiler-wrapper/build-model harness, including the recognized `ra-jit-rustc-first` spelling and real Cargo answer discovery. Do not count the local delegation fixture as the sandbox runner.
* Review whether suppressing the redundant interception at `for_load` is a better architectural fix. Method idempotence is useful defense in depth but does not establish the best nesting design.
* Consider non-UTF-8/error-preparation controls, interrupted writes, frontend migrations and generated-prefix ambiguity. Exact generated text at a different frontend intentionally refuses to migrate.
* The read/check/rename/write sequence is **not atomic or concurrency-safe**, and an error after rename retains upstream's recovery limitations. Do not advertise transactionality, race resistance, crash consistency, ownership/authenticity validation, or complete corruption detection.
* Windows uses a different hardlink path and was neither changed nor tested.
* Only after operator reconciliation of the existing angelX protected-source seal and the remaining full-host script boundaries, repeat a semantic host edit against paired Cargo. Until then, no full-host compatibility, reload latency, or record-time claim is warranted.

## Evidence

Kept with the measurement, not published:

* Preregistered method experiment: `reports/performance/blaze-script-wrap-preregister-iteration21.md`
* Offline reduction runner: `reports/performance/blaze-script-wrap-reduction-r2-iteration21.py`
* 8-test raw receipt: `reports/performance/blaze-script-wrap-reduction-r2-iteration21/tests.json`
* Reduction identities and outcome: `reports/performance/blaze-script-wrap-reduction-r2-iteration21/receipt.json`
* Installed-wrapper control runner: `reports/performance/blaze-script-wrap-installed-control-iteration21.py`
* Installed-wrapper identities and outcomes: `reports/performance/blaze-script-wrap-installed-control-iteration21/receipt.json`
* Earlier protected-source gate diagnosis: `reports/performance/operator-lock-diagnosis-iteration16.json`
