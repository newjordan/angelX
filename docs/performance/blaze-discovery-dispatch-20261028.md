# Blaze upstream PR opportunity: avoid self-reentry during native discovery

## Status and boundary

**Unapplied candidate; not installed, submitted upstream, or qualified for angelX.**
Pinned upstream: `b383b0bcf5da33816809b00a37e1c36a0e761efb`.

Iteration 21 proposed an idempotence guard inside Unix `script::stand_in_front`. Iteration 22 instead changes compiler selection in `cargo/wrapper.rs` so native discovery does not invoke a second recognized Blaze frontend. The interception method remains byte-for-byte upstream. The guards and dispatch correction could be complementary, but their combination is not tested here.

No startup or rebuild latency improvement was measured. Cargo remains the normal angelX backend. Existing source/seal authorization and full-host semantic edit/restore qualification remain unresolved; neither is bypassed by this reduction.

## Mechanism

Production `wrapper::command` consumes the first program supplied by Cargo. If `switch::is_wrapper` recognizes it by filename stem (`ra-jit-rustc` or `ra-jit-rustc-first`), it stores that program in `twice` and consumes real rustc next. `answers::reply` can return before compilation. Compiler selection then resolves `RA_JIT_CHAINED_WRAPPER`, including its explicit-empty override and enabled-configuration fallback.

The pinned `ONLY_ASKS` arm selects `twice` when present and removes `ONLY_ASKS` from the child. That child enters ordinary Blaze compilation; the parent subsequently enters `for_load`. On the native rustc-backed path both can call `stand_in_front`. The second call renames the generated shell wrapper over the actual executable at `.real`.

The installed pinned executable reproduced this compile-only result. With a single frontend, `.real` was ELF; with nested frontends, `.real` was the shell wrapper and had the same SHA-256 as the linked wrapper. No recursive wrapper was executed.

### A rejected suspicion

Compiler selection ignores `chained` in the outer `ONLY_ASKS` arm, but this **does not imply the user wrapper is lost end-to-end**. The installed nested control called it once in the inner normal invocation. Its log showed `ONLY_ASKS` already removed. The candidate preserves this measured one-call behavior while eliminating the inner Blaze dispatch. Do not advertise general chain restoration as the benefit.

## Candidate

Retained patch:
`reports/performance/blaze-dispatch-reduction-r3-iteration22/candidate-upstream.patch`.

Only the `ONLY_ASKS` match arm changes:

- With a recognized second frontend and a resolved user chain, run that chain with real rustc first.
- With a recognized second frontend and no chain, run real rustc directly.
- Otherwise preserve Cargo's first program and remaining argv. Do not prepend a workspace chain to dependency units or an arbitrary user wrapper.
- Remove `ONLY_ASKS` from the selected child as before. Leave ordinary selection, `for_load`, answer handling, interception, sandbox and target classification unchanged.

A broader rule applying CHAINED whenever EVERY_CRATE is absent was rejected before execution: `answers::ready_cargo` may install a global wrapper without that marker, so it is not a reliable workspace discriminator.

The patch inherits filename-based recognition. Its comment saying recognition "proves" replacement is too strong; soften that before upstream submission. No stronger binary provenance is established here.

## Evidence

Command: `python3 reports/performance/blaze-dispatch-reduction-r3-iteration22.py`.
This was a single-use artifact-producing harness: its output directory is create-only, so do not rerun in place.

Result: exit 0; 20 controls. These cover command selection, the two inherited wrapper spellings, unset/empty chain and a controlled enabled-state fallback, dependency routing with/without EVERY_CRATE, arbitrary-wrapper argv inspection, non-UTF8 argv, unchanged ordinary selection, installed native baseline controls, original-method nested corruption, candidate native preservation, compiler failure, missing rustc, and real Cargo workspace/path-dependency topology through the reduction.

Candidate `.real` SHA-256 equaled the installed single-wrapper ELF SHA-256:
`4bdcf5f7ea76e1410e53d2adc66ed3f64bcb6ad30287a1072d4fe98dea55840c`.
Running that confirmed native file directly returned exit 0 and `native-script-22`. This is **not** execution through the sandbox frontend.

The Cargo topology control built and ran a workspace program and an external path dependency offline. The workspace compilation used the user chain; the external dependency did not. The reduction also forwarded seven Cargo question probes through the chain; production's earlier answer fast path is absent from this extraction, so those probe calls are not a production contract.

First failed extraction and second failed fixture are retained separately. The first omitted a local Path import. The second omitted `--out-dir` required by production Unit parsing, so it correctly compiled ordinary ELF without interception. The third added this flag alongside `-o`; rustc emitted its expected ignored-out-dir warning. No candidate logic was changed between these fixture repairs.

## Scope and acceptance before applying

The reduction compiles exact parsing/selection/interception excerpts with cached anyhow/shlex. It substitutes Enabled::read, does not include `answers::reply`, and simulates successful native interception rather than implementing production Unit/for_load/confinement. The real-Cargo control uses this reduction executable, not a patched Blaze binary. Timing is descriptive fixture cost only.

Required production tests:

1. Build a patched upstream binary in a separately authorized environment; retain source/compiler identity.
2. Real Cargo discovery with workspace and external build scripts: one intended user-wrapper invocation and one interception; no generated shell saved as `.real`.
3. Exercise the generated frontend and actual confinement, not just direct native execution.
4. Cover question/cache modes, enabled configuration, empty overrides, user/global wrappers, arbitrary/renamed wrappers and same-stem identity collisions. The answer fast path predates the patch and needs an explicit chain contract.
5. Cover own-target descriptions/scripts/proc macros, WASI scripts and Windows separately. These paths are untested here and may not use the selected compiler at all.
6. Cover compile failure, restart, stale outputs and concurrent Cargo builds with product-level state checks. Keep iteration 21's method-idempotence candidate separate until their composition is measured.
7. Reconcile operator-protected source authorization and repeat full-feature angelX build, semantic edit and restore freshness before changing launcher defaults.

Read-only separate-seat review is retained at `reports/performance/blaze-dispatch-review-iteration22.md`. The same-author evidence audit (`reports/performance/blaze-dispatch-evidence-audit-r2-iteration22.json`) exited 0 with 153 assertions, including exact extraction reconstruction, patch applicability, artifact hashes and unchanged live source identities. It is separate from compiler correctness and independent runtime qualification. Its initial failed reconstruction audit is also retained: the auditor used wrong template markers and omitted the documented module-path transformation; the second auditor corrects those bookkeeping defects without rerunning the experiment.

Compiler paths in raw receipts include mise/rustup shims and Cargo's resolved stable toolchain. This was not a frozen compiler-version comparison; no inference about compiler performance or cross-version equivalence is justified. No pre-existing upstream integration suite was run against a patched binary in this iteration.
