# Invocation-local compiler probe reuse (iteration 28, 2026-10-28)

## Change and invariant

`bin/angelX` now reuses the successful `rustc -vV` output used for installed-toolchain host discovery when the compiler path **exactly equals** the compiler eventually selected alongside Cargo. It uses shell state for this invocation only, not a file cache. The selected Cargo still runs synchronously on every source launch.

This is a small startup-overhead improvement, not a new build backend. Normal defaults remain the incremental `fast` profile and Cargo. Explicit Blaze behavior, features, source checking, build failures and no-build contracts are unchanged. Blaze automatic promotion remains blocked on its separately documented freshness and full-host gates.

### Selection/failure rules

1. Clear inherited internal probe/host variables and their export attributes.
2. Probe installed compilers as before. Record complete metadata only when the probe succeeded. A failed discovery probe may still supply a host as in the baseline, but is not trusted as metadata.
3. Select Cargo in the original order, rejecting the existing rustup shim case.
4. Invalidate captured metadata **before** any rustup provisioning attempt; installation can replace rustc at the same path.
5. Pin the selected Cargo's sibling compiler as before. If its exact path matches the successful discovery probe, pass the captured complete metadata as `ANGEL_BUILD_RUSTC`. Otherwise run the final selected compiler's `-vV`.
6. Always invoke Cargo. No source/image timestamp cache or delayed/background build substitution is added.

The PATH compiler fallback deliberately remains uncached. Its failure is handled explicitly: moving discovery out of nested command substitutions changes Bash `errexit` behavior, so a failing shim must not prevent a later working installed Cargo fallback.

No disk cache invalidation policy, wrapper-environment key, or machine-specific retained version string is required. This removes one redundant compiler process in the common installed pinned-toolchain case, not all compiler startup overhead.

## Measured evidence

Existing test entry point: `node --test tests/scripts/native-launch.test.mjs`.

- New per-launch query-count regression fails unchanged logic: two probes instead of one. Different-selected-compiler control already passes unchanged logic.
- First candidate suite: **38/38** tests passed.
- Separate-context review found failed-PATH-probe and same-path provisioning gaps that those initial tests missed. They were fixed.
- Final targeted checks: **6/6**, covering pinned lookup, invocation-local refresh, different selected compiler, failed PATH fallback (empty and partial output), provisioning invalidation and first provisioning. The final file contains 40 tests; there is **no claim of a final 40-test suite run**.

A separate private tiny Rust crate uses actual installed Rust/Cargo 1.95.0, offline, the launcher's fast profile/glass argument selection, a shared HOME/target across arms, unchanged source, and 12 alternating-order warm pairs. Full raw timing/output receipts and exact fixture sources are retained. All 24 warm launches exited zero with identical sentinel output; tracked source/lock/image hashes were unchanged.

| Warm tiny-crate launcher wall time | Baseline | Candidate |
|---|---:|---:|
| Median, 12 samples per arm | 30.907688 ms | 26.9796505 ms |
| Range | 26.356133–33.423516 ms | 24.516707–30.062799 ms |

Paired median reduction: **4.492934 ms**; this is **14.5366% of the baseline median**. Difference of the two arm medians is a different statistic: 3.9280375 ms (12.7089%). Candidate was faster in 11/12 pairs. The preregistered descriptive pilot threshold (>=1 ms and >=2%) was met; this is not a significance test or a production guarantee.

Prime observations are separate: baseline 109.434389 ms (initial crate compile), candidate 29.354358 ms (already built). These unequal-cache primes are **not a cold-build comparison**. Temporary target after priming: 979,021 bytes; the driver removed its temporary directory. The first driver failed before Cargo because the fixture omitted the required provider-policy helper; its source/log/raw failure are retained, not counted as timing data. The corrected driver supplied the real helper and an initial lockfile.

## Limits and promotion gate

No full cockpit build, provider call, Glass/TUI readiness measurement, host semantic-edit/restore measurement, cold setup qualification, or Blaze performance experiment was performed here. The tiny crate does not exercise the host build script, native dependency graph, browser assets or real application startup. Do not extrapolate the 14.5% pilot result to angelX as a whole.

The protected `cockpit/src/drive/loop_ctl.rs` does not match its operator seal. It was already mismatched at iteration entry and has no tracked working-tree diff; its bytes and the seal file are unchanged. Full-host build/promotion remains pending operator resolution of that gate. Do not reseal around it.

Concurrent external replacement of a toolchain between probing and execution is not made transactional. This is no persistent cache, but path equality does not guarantee immutable executable contents. The ordinary baseline also had a probe-to-execution window. macOS/Bash 3.2 was not exercised. Direct injection of exported internal probe variables was reviewed but not separately tested.

Two read-only, separate-context same-model reviews are retained, including a false-positive CARGO export allegation. The baseline's outer `CARGO=$(...)` assignment already updated parent-shell CARGO and retained an inherited export attribute. A minimal Bash control found identical export behavior; no speculative environment change was made.

## Reproduction and next action

- Canonical findings: `reports/research-82abbb708cda5ae57589ee8a17aeaaaa-28.md`.
- Preregistration: `reports/performance/launcher-probe-preregister-iteration28.md`.
- Exact iteration-local patch: `reports/performance/launcher-probe-iteration28/candidate-from-iteration27.patch` (not a patch against clean HEAD, which lacks earlier loop work).
- Raw timing/result and fixture inputs: `reports/performance/launcher-probe-iteration28/timing-r2/`.
- Preserved benchmark sources: `reports/performance/measure-launcher-probe-iteration28.py` (failed fixture) and `reports/performance/measure-launcher-probe-r2-iteration28.py` (corrected fixture).

After the operator resolves the protected-file seal, measure full-feature unchanged launcher-to-draft and semantic edit/restore with the existing bounded perf tools. Keep auto=Cargo until Blaze separately proves fresh output and beats qualified end-to-end latency. No claim of record-time startup or loop completion is warranted yet.
