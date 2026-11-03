# Blaze PR lead: respect removed child wrapper entries during probe setup

Date: 2026-10-28. Scope: upstream research, **unapplied**. Not an angelX deployment.

Suggested title: **Respect child environment removals before installing the question-only rustc wrapper**.

## Concrete issue

Pinned Blaze `b383b0bcf5da33816809b00a37e1c36a0e761efb`, `crates/ra-jit/src/answers.rs:226–235`, decides whether to install its compiler-question wrapper with:

```rust
command.get_envs().any(|(it, value)| it == *name && value.is_some())
    || std::env::var_os(name).is_some_and(|it| !it.is_empty())
```

For either `RUSTC_WRAPPER` or `CARGO_BUILD_RUSTC_WRAPPER`, a nonempty parent variable is considered named even after `Command::env_remove` masks it in the child. With no other surviving named wrapper, the inherited value suppresses installing `RUSTC_WRAPPER=<blaze frontend>` plus `RA_JIT_ONLY_ASKS=1`. Cargo then runs its own rustc questions rather than taking that answer path.

A conditional production route exists: `plan.rs:127–136` passes `Asked.env` through `toolchain::command`, whose vendored implementation (`vendor/rust-analyzer/crates/toolchain/src/lib.rs:77–94`) translates `None` values into `env_remove`. `main.rs:3110` extends the reload configuration with `Inputs::variables`; `inputs.rs:203–207` can produce `None` for absent tracked variables. The default `FLAGS` set at `inputs.rs:16` contains rustflags, **not wrapper variables**. A wrapper removal is therefore not shown to be routine: tracked build-script variables or another caller/configuration would need to supply the entry. Frequency and daemon lifecycle benefit remain unmeasured.

## Minimal candidate

Retained exact unified diff:

`reports/performance/blaze-child-env-reduction-iteration23/candidate-upstream.patch`

```rust
match command.get_envs().find(|(it, _)| *it == *name) {
    Some((_, value)) => value.is_some(),
    None => std::env::var_os(name).is_some_and(|it| !it.is_empty()),
}
```

Only the `named` predicate changes. Explicit `Some("")` remains a way to suppress automatic insertion. An inherited empty string alone retains existing behavior and does not suppress insertion. Either other named variable still suppresses insertion. A missing frontend still leaves the child untouched.

Important policy boundary: `env_remove(RUSTC_WRAPPER)` means removing that parent's wrapper, **not** promising the child will have no wrapper after `ready_cargo`. The candidate may replace the removal with Blaze's question-only wrapper. Maintainer agreement on this policy is required before adoption. No changes to answer cache keys, `var_of`, workspace wrappers, isolation, dispatch, launcher or defaults are proposed.

## Evidence collected

- Original and candidate `ready_cargo` methods extracted from the exact source; standalone stdlib Rust driver substitutes only `wrapper_path()` with a local installed-frontend lookup. No full candidate crate build.
- 21 paired environment-selection controls / 42 helper invocations: expected maps matched in both variants. Only the three inherited-nonempty/last-wrapper-removal cases differ.
- Real installed Cargo/Rustc, tiny local stdlib fixture: `cargo rustc --offline --lib -- --print cfg`; identical cfg output, removed tripwires not invoked. Installed pinned Blaze handles the candidate's selected question path.
- Six alternating-order warm pairs after one prime per arm; baseline median **30.225 ms**, candidate **21.365 ms**; descriptive reduction **29.313%**. Every measured baseline ran the instrumented rustc proxy four times; every candidate ran it zero times.
- Existing `harness/answers.py::check`, first seven native question cases, temporary `src/lib.rs`: **7/7 compared**, zero skipped. Installed answer engine only; not candidate-crate qualification.
- `git apply --check` against the unchanged pinned checkout exited 0. No patch applied.

Raw evidence: `reports/performance/blaze-child-env-reduction-iteration23/`. Review: `reports/performance/blaze-child-env-review-iteration23.md`.

## Limits: do not broaden the claim

This is a deliberately exposed question workload. Both arms set `CARGO_CACHE_RUSTC_INFO=0`; normal Cargo may already suppress questions with its own cache. Each arm has independent temporary target and Blaze-home roots. The logging rustc shell proxy adds overhead and counts only its own executions; it is **not an exhaustive execve trace**. Installed Blaze may reach the compiler through a different path. The reported timing includes the proxy/log cost and is not a general pure compiler cost measurement. Six same-host warm pairs are a pilot, not statistical or cross-machine qualification. There is no angelX startup/edit/reload speed result here.

`Command::get_envs` does not expose whether inheritance was cleared; the change does not solve `env_clear`. Cargo-config-only wrappers, wrapper chaining/side effects, non-question delegation and daemon lifecycle were not tested. The read-only separate-context review was tool-truncated after its third finding; only available text is retained. An evidence audit is same-author bookkeeping, not a product test or independent approval.

## Acceptance before an upstream PR or install

1. Agree on removal vs explicit-empty policy and document it adjacent to `ready_cargo`.
2. Add in-crate regressions using isolated processes for each environment case (avoid process-global env mutation races). Build and test the complete candidate crate against the pinned toolchain.
3. Test config-only wrappers, workspace wrapper composition, named user wrapper preservation, question/non-question delegation and `RA_JIT_ONLY_ASKS` behavior; verify no skipped compilation or hidden side effects.
4. Qualify a real caller/daemon/reload path that supplies a removed inherited wrapper, with ordinary Cargo rustc-info caching enabled. Count actual compiler processes, not just the fixture proxy.
5. Extend the existing upstream question harness; require nonzero expected comparison counts. Its library question cases need a real `src/lib.rs`: without it, the current harness silently skips those two cases after rustc fails. Iteration23 retained both that diagnostic and a corrected temporary fixture.
6. Run normal freshness and restore controls and relevant platform variants. Keep the installed compiler, launcher and build defaults unchanged until review/qualification.

## angelX integration gate

The operator seal expects `cd017dfde2178e21ba3f05855698b703a67f40101f442e99142ad4651dcce733` for `cockpit/src/drive/loop_ctl.rs`; current bytes hash to `c5fb1562ccad4ff15fd49d6aa399d5bebcb0b5739e951865a6733bce087d7148`. They match the current HEAD blob, not an iteration23 edit. Host build/deployment qualification remains blocked; only the operator can reconcile the approved source with the seal. Do not reseal, revert or bypass it automatically.
