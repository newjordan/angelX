# Short-text startup admission and disabled Blaze cache keys — iteration 37

## Retained angelX change

`bin/angelX` now handles empty or short (<=1,024-byte) printable ASCII launch text with Bash builtins. Valid `--draft`, `--resume`, `--driver`, `--model` and `--effort` values no longer start Python for admission. Non-ASCII, control characters and larger values still use strict UTF-8 and Rust-compatible Unicode whitespace validation. Selectors still reject edge spaces; initial/resume text still preserves them. Function-local C collation does not leak to the application. No Cargo freshness check, feature, compiler error, image-copy rule or build-backend policy was removed.

Review exposed a pre-existing validation defect: inherited `PYTHONOPTIMIZE=1` or `2` disabled the Python fallback's assertions. Those checks now use explicit rejection. The new ASCII path intentionally does not require working Python; Unicode/long input still fails closed if its interpreter fails.

## Measured result — narrow, real-Cargo pilot

Twenty-four predetermined AB/BA pairs, after one prime per arm, used an offline dependency-free tiny crate and the actual source launcher:

```sh
bash <launcher> --term --draft hello --driver practice --model local --effort high
```

Four short text admissions per launch. Cargo ran on every attempt, completed the `fast` profile and executed a stdout sentinel. Stable source, lockfile, configuration, tool and image identities were checked. There was no no-build shortcut or fabricated compiler success.

| Measure | Result |
|---|---:|
| Baseline median | 79.697 ms |
| Candidate median | 53.904 ms |
| Median paired reduction | 25.728 ms |
| Paired reduction / baseline median | 32.283% |
| Candidate faster | 24/24 pairs |

All preregistered pilot thresholds held: >=2 ms, >=5%, >=20/24 wins. **This is not a full cockpit startup, cold launch, compiler edit/rebuild, terminal-ready or universal 32% gain.** Bare launches without text arguments have no validator subprocesses to remove. Unicode and long-input performance was not measured. No Blaze default promotion follows from these results.

## Verification boundaries

- R1: pre-existing launcher suite plus two added regressions: **46/46**, after fixing a test fixture's early-validation PATH. The new subprocess-count regression fails against the frozen baseline for the expected reason.
- R1: **2,094** extracted-function exit-status comparisons under C and C.UTF-8; zero mismatches. This is not complete launcher argv/stderr equivalence.
- R2 final source: targeted admission tests **3/3**, including inherited Python optimization, exact argument forwarding, locale preservation and interpreter failure. R1's full suite/differential were not rerun on R2; R2 changes only fallback predicates to explicit exit-2 checks.
- A separate-context review found no new correctness defect in the bounded final change. Bash 3.2 syntax was considered, but only Bash 5.3 was executed here.
- Linux argv transport errors at >=131,072 bytes prevent testing the validator's nominal 256 KiB initial-text boundary end to end. This is inherited; no large-input support guarantee is made.

## New Blaze PR candidate

In clean pinned `crates/ra-jit/src/answers.rs`, `answer()` computed a compiler/cache key before checking whether a cache directory existed. With `RA_JIT_ANSWERS=0`, `dir()` returns `None`, but the old ordering still resolves compiler metadata and hashes input. The retained one-line candidate checks `dir()` first; it still runs the selected command when answers are disabled.

Exact `answer()` and `dir()` control-flow reductions, using explicit seams for the key, lock/store and own-answer machinery, show zero unnecessary key calls instead of one for disabled/no-home cases. Six behavior scenarios per arm covered real local shell child output/failure, no-home fallback, enabled caching, own answer and own-only miss; a separate focused count regression fails old and passes candidate. **These are not full-module tests, actual rustc cache-key tests or elapsed-time measurements.** The patch is apply-ready, not installed; real key-None, locking and full upstream integration remain next gates.

## Evidence and current policy

Full worker report (`reports/research-82abbb708cda5ae57589ee8a17aeaaaa-37.md`) · timing summary (`reports/performance/launcher-text-iteration37/timing/summary.json`) · final tests (`reports/performance/launcher-text-iteration37/final-launch-text-tests.log`) · Blaze patch (`reports/performance/blaze-disabled-key-iteration37/candidate.patch`). These files are kept with the measurement and are not published.

Keep Cargo as the qualified normal path and Blaze explicitly experimental until its existing edit/restore freshness and reload blockers are resolved. The existing protected-file seal mismatch remains owner-controlled and untouched. Earlier report/source identities were checked; see the worker audit for scope, setup failures and budget deviations. No whole-goal acceptance command is bound.
