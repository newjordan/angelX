# Blaze compiler-query environment identity — 2026-10-28, iteration 29

## Status / purpose

**Confirmed reduced-module correctness defect; unapplied explicit-context API prototype. Not deployable and not a speed measurement.** This closes an explicitly unqualified edge from the earlier target/file-input experiments: command-wide cleared inheritance. It does not replace those experiments or qualify installed Blaze.

Goal: eventually make Blaze safe enough to accelerate angelX startup and edit/reload loops. Incorrect compiler-query answers can select wrong cfgs or hide compiler rejection. Fast wrong answers are not a useful speedup. Keep the qualified Cargo launcher path; do not switch `auto` to Blaze on this result.

## Root cause

Pinned Blaze checkout: `b383b0bcf5da33816809b00a37e1c36a0e761efb`.

`crates/ra-jit/src/answers.rs::var_of` examines `Command::get_envs()`, then falls back to `std::env::var_os(name)` when there is no explicit entry. Rust's public `get_envs()` does **not** expose whether `env_clear()` was called. After clear+remove, even a removed variable may have no explicit entry. Consequently parent values can be used in:

1. `own_answer`: synthetic output and nightly/bootstrap decisions;
2. `key`: the persistent-answer cache identity;
3. toolchain resolution driven by the same resolver.

The child process itself does not inherit these variables. This mismatch is observable, not just a theoretical cache concern.

Examples in the frozen 34-row matrix:

- Parent `RUSTC_OVERRIDE_VERSION_STRING=parent29`, inherited `rustc -V`: correct overridden version primes the baseline answer cache. Clear the command environment and issue the same query: baseline still returns `rustc parent29`, not the real cleared compiler version. The problem repeats for `--version` and `-vV`, and clear+remove.
- Parent `RUSTC_BOOTSTRAP=-1`, cleared cfg query: synthetic cfg answers differ from the compiler's actual cleared environment.
- Cleared command explicitly sets `RUSTC_BOOTSTRAP=-1`, then asks `-Z unstable-options --print=target-spec-json`: actual rustc exits **1**, baseline returns **0** plus JSON. The prototype preserves the compiler's rejection and stderr.

Do not label all nine mismatches as exit-status false successes. Eight are wrong successful-output observations; one is a true nonzero-to-zero false success.

## Prototype contract

`reports/performance/blaze-command-env-iteration29/candidate-api-prototype.patch` adds:

- `Environment::{Inherit, Clear}` supplied by the code that constructed the command;
- `answer_with_env`, `own_answer_with_env`, and resolver/key variants;
- explicit child overrides/removals first, parent fallback only for `Inherit`;
- persistent key namespace `ra-jit answer 2` instead of `1`, refusing potentially contaminated old-namespace entries;
- three deterministic environment-context tests.

The reduced driver constructs the command and matching context together. Existing command-only compatibility functions still select `Inherit`. **Applying this patch alone is not sufficient**: those functions can still cache incorrect answers under the *new* namespace when passed a cleared command. Namespacing is migration protection, not context detection.

No guessing via `Debug` formatting, injected sentinel variable, incidental environment counts or hidden process-global state. Effective inheritance must come from construction or the command must bypass interception/cache.

## Remaining API integration door

Actual production boundary:

```
ra-jit/src/main.rs:8127
  project_model::answer_toolchain_questions_with(answers::ask)
project-model/src/lib.rs:218–240
  type Answerer = fn(&mut Command) -> Option<io::Result<Output>>
answers.rs::ask
  -> answer(command, &[])   // still Inherit in this prototype
```

The callback carries no inheritance metadata. `ready_question` and `utf8_stdout` use it. The selected module reduction did not exercise the full rust-analyzer graph or Cargo dispatch.

Potential upstream approaches:

1. Carry an explicit command/environment context from construction through toolchain callbacks, with no forgotten compatibility path; or
2. Conservatively bypass synthetic/persisted answers on command-taking boundaries whose policy cannot be known. This must also avoid helpers that infer a toolchain from a wrongly inherited PATH/RUSTUP environment. Do not merely skip `own_answer` and leave the wrong persistent key active.

Audit `ready_cargo`, Cargo environment handling and other command-taking entry points as separate integration work. Source inspection here proves the API cannot carry the information; it does **not** prove the current rust-analyzer command constructors call `env_clear()`. Current production reachability through that specific callback remains unmeasured.

## Evidence and scope

| Evidence | Observed result |
| --- | --- |
| Unchanged/test-only reduced logic | Three new regression assertions fail |
| Explicit-context reduced candidate suite | 13 passed, 0 failed |
| Real nightly compiler differential | Baseline 25/34 exact; prototype 34/34 exact on exit, stdout, stderr |
| True exit false success | One baseline success against rustc rejection; prototype agrees with rustc |
| Candidate-only persistent-cache controls | Inherited and clear+explicit-override identities each created a file, returned a private sentinel from that cache file, then matched rustc after restoring original bytes |
| Same-author integrity checks | 178/178 hold, including exact patch reconstruction |
| Separate-context review | Confirms local defect and flags missing API propagation; returned response was truncated, no complete full-engine approval |

Reference processes execute the exact same `std::process::Command` setup in a bypass branch, spawning real absolute-path rustc; output is not fabricated or normalized. No stderr exclusions. 110 explicitly launched query-driver processes; each timeout 5s. Engine/atomic writer, Cargo dispatcher, wrapper discovery and native CPU handling are not qualified by this reduction.

Two initial compiler-fixture failures and a receipt-parser failure are retained, not counted as regressions. Setting `RUSTC_OVERRIDE_VERSION_STRING` for Cargo itself made Cargo reject a non-semver identity; its cached identity persisted into the first repair. Final measurement builds used `CARGO_CACHE_RUSTC_INFO=0` with override absent, then supplied parent values only to direct test/query executables. See fixture amendments and raw receipts.

## Acceptance gates before adoption

- Propagate policy through real command construction and callback APIs, or decline unknown commands conservatively.
- Turn all command-only paths into explicit audited contracts; demonstrate cleared input cannot enter an inherited-only cache path.
- Compose and regression-test with earlier target/file-input fixes; this patch is against unchanged pinned answers, not their composition.
- Exercise real crate and rust-analyzer callback tests, Cargo readiness/dispatch, proxy/PATH selection, platform-specific environment behavior, and migration from old caches.
- Keep cache-read controls for both identities; add actual unknown-context fallback tests.
- Build/install only after the operator resolves the pre-existing protected seal gate. Do not re-seal automatically.
- Repeat end-to-end edit/change/restore image freshness and bounded startup/rebuild timing. No performance-adoption decision from this correctness matrix.

## Artifacts

- Main report: `reports/research-82abbb708cda5ae57589ee8a17aeaaaa-29.md`
- Frozen plan: `reports/performance/blaze-command-env-preregister-iteration29.md`
- Exact source, patch, raw outputs, reviews: `reports/performance/blaze-command-env-iteration29/`
- Preparation/check/differential/integrity scripts: `reports/performance/*blaze-command-env*iteration29.py`

Installed Blaze binaries, launcher, host images, host source/lock/build files and protected files were unchanged by this iteration. No claim that angelX starts or rebuilds faster yet.
