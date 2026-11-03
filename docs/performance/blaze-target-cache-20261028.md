# Blaze upstream PR opportunity: invalidate answers for mutable target search paths

## Proposed change — not installed

Investigated pin: `b383b0bcf5da33816809b00a37e1c36a0e761efb`.

Patch: `reports/performance/blaze-target-cache-iteration26/candidate-upstream.patch` (not published).

`crates/ra-jit/src/answers.rs::is_question` excludes explicit `.json` target arguments, but accepts extensionless custom target names. `answers::key` includes the `RUST_TARGET_PATH` string, compiler identity, arguments and stdin; it does not bind the JSON file selected by that path. An edit at the same lookup path therefore need not invalidate a cached compiler answer. The own-answer implementation already refuses custom-target search paths, but that refusal falls through to the vulnerable persistent compiler-answer cache.

The conservative candidate adds a guard at the beginning of `key`: when the existing `var_of` helper returns `Some` for `RUST_TARGET_PATH`, return `None`. This suppresses **both reads and publication**, including existing stale entries, without implementing a competing target resolver. It does not flush unrelated caches, hash a directory on every startup, add dependencies, or alter the normal absent-variable key. Empty and non-Unicode values are treated conservatively as present. Explicit child-variable removal restores ordinary caching.

Scope is ordinary inherited environments and explicit per-variable overrides/removals. **It does not qualify arbitrary `Command::env_clear()` usage.** The inherited helper cannot observe that command-wide state: if a parent has the variable set and a command clears it, the candidate may bypass caching unnecessarily. That is a performance limitation, and a separate command-environment design question; this patch does not fix the earlier helper or promise effective-environment identity for all `Command`s.

## Concrete reproduction

Use a private target-spec directory and cache, not a developer's real cache. Export the host target with pinned nightly rustc (`RUSTC_BOOTSTRAP=1`, `-Zunstable-options --print=target-spec-json --target=x86_64-unknown-linux-gnu`). Write it as `angel-cache-target26.json`, changing only `vendor` to `cache_a`. Set `RUST_TARGET_PATH` to its absolute directory and query:

```sh
"$RUSTC" -Zunstable-options --print=cfg --target angel-cache-target26
"$RA_JIT" rustc-wrapper "$RUSTC" -Zunstable-options --print=cfg --target angel-cache-target26
```

Both initially print `target_vendor="cache_a"`. Change the file's vendor to `cache_b` without changing the lookup path or arguments. In the measured installed wrapper, rustc prints `cache_b` and Blaze prints **`cache_a` with exit 0**. Restoring A makes the stale answer look correct again. A no-touch control also matches.

The two measured workarounds are `RA_JIT_ANSWERS=0` and an explicit absolute `--target /path/to/angel-cache-target26.json`; both match the reference at all four phases. These are controls for selected compiler queries, **not qualification of a full angelX custom-target build or a startup recommendation**.

## Local acceptance completed

- Installed binary calibration: 12 requested comparisons, zero exclusions. Exactly the normal-name edit mismatches; eight disabled-cache/explicit-JSON controls match. Toolchain: nightly-2026-10-27, Linux x86-64.
- Regression with unchanged production logic: two key-policy tests fail (including raw non-Unicode path bytes). Candidate passes all **12/12** reduced-module tests, including pre-existing answer and unstable-option tests.
- Real compiler fallback differential using frozen complete answer modules: **14/14 candidate output triples** match rustc across separated/joined target spelling and prime/no-touch/edit/restore/delete/invalid/repair. Unchanged baseline matches 6/14, with four stale-successful-output differences and four suppressed reference errors. No stderr or error exclusions.
- A deliberately populated baseline cache is shared with the candidate at each step; its bytes and mtime remain unchanged. The candidate ignores it rather than requiring users to delete caches.
- Ordinary `--print=target-list` questions without `RUST_TARGET_PATH` still create an answer entry cold and retain the same entry warm; all four baseline/candidate control comparisons match rustc.
- **139/139 same-author integrity assertions** hold, including exact source delta, raw triple derivations, unchanged live identities and a patch dry-run/application that reconstructs the tested candidate. These are not 139 independent behavioral cases.
- One separate-context read-only review supports the demonstrated correction, but identified the `env_clear` contract limitation above. It is advisory, not independent dynamic acceptance.

The frozen reduction includes exact `answers.rs`, `own.rs`, embedded Rust data and unstable parser. Support substitutions remain: Cargo own dispatcher declines, wrapper discovery returns an error, native CPU panics if selected, the stub module includes only the exact valued-option constant, and atomic publication uses plain `std::fs::write`. Production atomic publication, wrapper integration, Cargo construction, native CPU, daemon and concurrent locks are **not** qualified. The actual query fallback/key/read code is unmodified except for this guard.

Raw evidence: iteration26 tree (`reports/performance/blaze-target-cache-iteration26`). Full research report (`reports/research-82abbb708cda5ae57589ee8a17aeaaaa-26.md`). These files are kept with the measurement and are not published.

## Before upstream merge or automatic host adoption

1. Build the complete patched engine and run upstream crate/wrapper/Cargo/daemon integration tests. This patch is not applied to the checkout or installed engine; no PR was opened.
2. Test supported Rust versions, platforms, alternate print types, relative paths/cwd changes, search-path precedence and concurrent target edits. File edits concurrent with a single compiler invocation remain the compiler's own behavior.
3. Resolve command-wide environment clearing if upstream requires the stronger effective-environment contract; do not fix it by guessing from inherited process values.
4. Consider a later precise resolver/content fingerprint if users need caching with target paths set. This deliberately conservative guard also disables fallback persistent caching for built-in questions whenever that variable is present, including empty. Repeated compiler spawns can therefore be slower.
5. Check custom specs located in other rustc lookup locations; this experiment only qualifies the absolute `RUST_TARGET_PATH` case, not all mutable compiler inputs.
6. Validate interaction with earlier candidate answer patches. This candidate is based on the unchanged pin, not a combined upstream branch.
7. Obtain operator resolution of the existing protected-file seal mismatch before any angelX host build/promotion. Current `loop_ctl.rs` equals HEAD but differs from its sealed hash. No resealing or protected-source edits were done.
8. Measure startup and edit/reload end to end only after correctness gates. **No speedup or record-time claim is established by this work.**
